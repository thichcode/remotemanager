//! WebSocket relay for embedded SSH terminals.
//!
//! Spawns an `ssh` process attached to a real pseudo-terminal (ConPTY on
//! Windows via portable-pty) and relays it over a localhost WebSocket, the
//! same transport the RDP relay uses.
//!
//! Wire protocol (binary, little-endian):
//!   Server → Client:
//!     0x01 = Data        (raw PTY output bytes)
//!     0x02 = Closed      (session ended)
//!   Client → Server:
//!     0x10 = Input       (raw bytes to feed to the PTY)
//!     0x11 = Resize      (u16 rows, u16 cols)

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::Duration;

use log::info;
use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use tokio::sync::oneshot;

use crate::backend::security::input::validate_host;

pub const MSG_DATA: u8 = 0x01;
pub const MSG_CLOSED: u8 = 0x02;
pub const MSG_INPUT: u8 = 0x10;
pub const MSG_RESIZE: u8 = 0x11;

pub struct TerminalSessionParams {
    pub host: String,
    pub port: i32,
    pub username: String,
    pub ssh_key_path: Option<String>,
}

pub struct TerminalSessionResult {
    pub ws_port: u16,
    pub shutdown: oneshot::Sender<()>,
}

type SharedWs = Arc<Mutex<tungstenite::WebSocket<std::net::TcpStream>>>;

pub fn start_session(params: TerminalSessionParams) -> Result<TerminalSessionResult, String> {
    validate_host(&params.host)?;

    let listener = TcpListener::bind("127.0.0.1:0")
        .map_err(|e| format!("Failed to bind WebSocket listener: {}", e))?;
    let ws_port = listener
        .local_addr()
        .map_err(|e| format!("Failed to get listener address: {}", e))?
        .port();
    info!("Terminal WebSocket relay listening on 127.0.0.1:{}", ws_port);

    let (shutdown_tx, mut shutdown_rx) = oneshot::channel::<()>();

    let host = params.host.clone();
    let port = params.port;
    let username = params.username.clone();
    let ssh_key_path = params.ssh_key_path.clone();

    thread::Builder::new()
        .name("terminal-relay".into())
        .spawn(move || {
            listener.set_nonblocking(true).ok();

            loop {
                if shutdown_rx.try_recv().is_ok() {
                    info!("Terminal session shutdown requested");
                    break;
                }

                match listener.accept() {
                    Ok((tcp_stream, addr)) => {
                        info!("WebSocket connection from {}", addr);
                        tcp_stream.set_nonblocking(false).ok();

                        match tungstenite::accept(tcp_stream) {
                            Ok(ws_stream) => {
                                if let Err(e) = run_terminal_session(
                                    ws_stream,
                                    &host,
                                    port,
                                    &username,
                                    ssh_key_path.as_deref(),
                                ) {
                                    info!("Terminal session error: {}", e);
                                }
                                // After one client session ends, stop accepting
                                // new connections for this session id.
                                break;
                            }
                            Err(e) => {
                                info!("WebSocket accept error: {}", e);
                            }
                        }
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(50));
                    }
                    Err(e) => {
                        info!("Accept error: {}", e);
                        thread::sleep(Duration::from_millis(100));
                    }
                }
            }

            Ok::<(), String>(())
        })
        .map_err(|e| format!("Failed to spawn terminal relay thread: {}", e))?;

    Ok(TerminalSessionResult {
        ws_port,
        shutdown: shutdown_tx,
    })
}

/// Commands the WS reader thread forwards to the PTY command thread.
struct PtyCmd {
    data: Option<Vec<u8>>,
    resize: Option<(u16, u16)>,
}

fn run_terminal_session(
    ws_stream: tungstenite::WebSocket<std::net::TcpStream>,
    host: &str,
    port: i32,
    username: &str,
    ssh_key_path: Option<&str>,
) -> Result<(), String> {
    let ws: SharedWs = Arc::new(Mutex::new(ws_stream));

    // Open a real PTY for the ssh process
    let pty_system = native_pty_system();
    let pty = pty_system
        .openpty(PtySize {
            rows: 24,
            cols: 80,
            ..PtySize::default()
        })
        .map_err(|e| format!("Failed to open PTY: {}", e))?;

    // Build the ssh command
    let mut cmd = CommandBuilder::new("ssh");
    if let Some(key) = ssh_key_path {
        cmd.arg("-i");
        cmd.arg(key);
    }
    cmd.arg("-o");
    cmd.arg("IdentitiesOnly=yes");
    cmd.arg("-o");
    cmd.arg("StrictHostKeyChecking=accept-new");
    cmd.arg("-p");
    cmd.arg(port.to_string());
    cmd.arg(format!("{}@{}", username, host));

    let mut child = pty
        .slave
        .spawn_command(cmd)
        .map_err(|e| format!("Failed to spawn ssh: {}", e))?;
    drop(pty.slave);

    let mut writer = pty
        .master
        .take_writer()
        .map_err(|e| format!("Failed to take PTY writer: {}", e))?;
    let mut reader = pty
        .master
        .try_clone_reader()
        .map_err(|e| format!("Failed to clone PTY reader: {}", e))?;

    let (cmd_tx, cmd_rx) = mpsc::channel::<PtyCmd>();

    // Command thread: applies writes and resizes to the PTY master.
    let master = pty.master;
    let cmd_thread = thread::Builder::new()
        .name("terminal-cmd".into())
        .spawn(move || {
            while let Ok(msg) = cmd_rx.recv() {
                if let Some(rc) = msg.resize {
                    let _ = master.resize(PtySize {
                        rows: rc.0,
                        cols: rc.1,
                        ..PtySize::default()
                    });
                }
                if let Some(data) = msg.data {
                    let _ = writer.write_all(&data);
                    let _ = writer.flush();
                }
            }
        })
        .map_err(|e| format!("Failed to spawn terminal cmd thread: {}", e))?;

    // WebSocket reader thread: client input + resize → cmd channel
    let ws_reader = ws.clone();
    let cmd_tx_reader = cmd_tx.clone();
    let read_thread = thread::Builder::new()
        .name("terminal-ws-reader".into())
        .spawn(move || {
            loop {
                let msg = {
                    let mut guard = ws_reader.lock().unwrap();
                    guard.read()
                };
                match msg {
                    Ok(tungstenite::Message::Binary(data)) => {
                        if data.is_empty() {
                            continue;
                        }
                        match data[0] {
                            MSG_INPUT if data.len() >= 2 => {
                                let _ = cmd_tx_reader.send(PtyCmd {
                                    data: Some(data[1..].to_vec()),
                                    resize: None,
                                });
                            }
                            MSG_RESIZE if data.len() >= 5 => {
                                let rows = u16::from_le_bytes([data[1], data[2]]);
                                let cols = u16::from_le_bytes([data[3], data[4]]);
                                let _ = cmd_tx_reader.send(PtyCmd {
                                    data: None,
                                    resize: Some((rows, cols)),
                                });
                            }
                            _ => {}
                        }
                    }
                    Ok(tungstenite::Message::Close(_)) => break,
                    Err(_) => break,
                    _ => {}
                }
            }
        })
        .map_err(|e| format!("Failed to spawn WS reader thread: {}", e))?;

    // Read PTY output and stream to WebSocket
    let mut buf = [0u8; 8192];
    loop {
        let n = match reader.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(_) => break,
        };
        let mut frame = Vec::with_capacity(1 + n);
        frame.push(MSG_DATA);
        frame.extend_from_slice(&buf[..n]);
        let mut guard = ws.lock().unwrap();
        if guard.write(tungstenite::Message::Binary(frame)).is_err() {
            break;
        }
    }

    // Send closed notification
    {
        let mut guard = ws.lock().unwrap();
        let _ = guard.write(tungstenite::Message::Binary(vec![MSG_CLOSED]));
    }

    let _ = child.kill();
    let _ = child.wait();

    drop(cmd_tx);
    let _ = read_thread.join();
    let _ = cmd_thread.join();

    Ok(())
}

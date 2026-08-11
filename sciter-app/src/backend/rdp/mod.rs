pub mod frame;
pub mod input;

use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;

use log::info;
use rdp::core::client::Connector;
use rdp::core::event::RdpEvent;
use tokio::sync::oneshot;

use crate::backend::security::input::validate_host;

pub struct RdpSessionParams {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub width: u16,
    pub height: u16,
}

pub struct RdpSessionResult {
    pub ws_port: u16,
    pub shutdown: oneshot::Sender<()>,
}

type SharedWs = Arc<Mutex<tungstenite::WebSocket<TcpStream>>>;

pub fn start_session(params: RdpSessionParams) -> Result<RdpSessionResult, String> {
    validate_host(&params.host)?;

    let listener = TcpListener::bind("127.0.0.1:0")
        .map_err(|e| format!("Failed to bind WebSocket listener: {}", e))?;
    let ws_port = listener.local_addr()
        .map_err(|e| format!("Failed to get listener address: {}", e))?
        .port();
    info!("RDP WebSocket relay listening on 127.0.0.1:{}", ws_port);

    let (shutdown_tx, mut shutdown_rx) = oneshot::channel::<()>();

    let host = params.host.clone();
    let port = params.port;
    let username = params.username.clone();
    let password = params.password.clone();
    let width = params.width;
    let height = params.height;

    thread::Builder::new()
        .name("rdp-relay".into())
        .spawn(move || {
            listener.set_nonblocking(true).ok();

            loop {
                if shutdown_rx.try_recv().is_ok() {
                    info!("RDP session shutdown requested");
                    break;
                }

                match listener.accept() {
                    Ok((tcp_stream, addr)) => {
                        info!("WebSocket connection from {}", addr);
                        tcp_stream.set_nonblocking(false).ok();

                        match tungstenite::accept(tcp_stream) {
                            Ok(ws_stream) => {
                                let ws: SharedWs = Arc::new(Mutex::new(ws_stream));

                                // Connect to RDP server
                                let rdp_addr = match format!("{}:{}", host, port).parse::<SocketAddr>() {
                                    Ok(a) => a,
                                    Err(e) => return Err(format!("Invalid RDP address: {}", e)),
                                };

                                let tcp = match TcpStream::connect(&rdp_addr) {
                                    Ok(t) => t,
                                    Err(e) => {
                                        info!("RDP connection failed: {}", e);
                                        return Err(format!("RDP connection failed: {}", e));
                                    }
                                };

                                let mut connector = Connector::new()
                                    .screen(width, height)
                                    .credentials(String::new(), username.clone(), password.clone());

                                let mut client = match connector.connect(tcp) {
                                    Ok(c) => c,
                                    Err(e) => {
                                        info!("RDP handshake failed: {:?}", e);
                                        return Err(format!("RDP handshake failed: {:?}", e));
                                    }
                                };

                                info!("RDP session established for {}", host);

                                let (input_tx, input_rx) = mpsc::channel::<input::ClientEvent>();

                                // Spawn WebSocket reader thread
                                let ws_reader = ws.clone();
                                let ws_read_thread = thread::Builder::new()
                                    .name("rdp-ws-reader".into())
                                    .spawn(move || {
                                        loop {
                                            let msg = {
                                                let mut guard = ws_reader.lock().unwrap();
                                                guard.read()
                                            };
                                            match msg {
                                                Ok(tungstenite::Message::Binary(data)) => {
                                                    if let Some(event) = input::ClientEvent::parse(&data) {
                                                        let _ = input_tx.send(event);
                                                    }
                                                }
                                                Ok(tungstenite::Message::Close(_)) => break,
                                                Err(_) => break,
                                                _ => {}
                                            }
                                        }
                                    })
                                    .ok();

                                // Main loop: read RDP events, write to WebSocket
                                let mut running = true;
                                while running {
                                    // Drain input from channel
                                    while let Ok(event) = input_rx.try_recv() {
                                        match event {
                                            input::ClientEvent::Resize { .. } => {}
                                            other => {
                                                if let Some(rdp_event) = other.to_rdp_event() {
                                                    let _ = client.write(rdp_event);
                                                }
                                            }
                                        }
                                    }

                                    // Read RDP events (blocking)
                                    let read_result = client.read(|event| {
                                        match event {
                                            RdpEvent::Bitmap(bitmap) => {
                                                let dest_left = bitmap.dest_left;
                                                let dest_top = bitmap.dest_top;
                                                let w = bitmap.width;
                                                let h = bitmap.height;
                                                let bgra = if bitmap.is_compress {
                                                    bitmap.decompress().unwrap_or_default()
                                                } else {
                                                    bitmap.data
                                                };
                                                if !bgra.is_empty() {
                                                    let frame = frame::encode_frame(
                                                        dest_left as u32,
                                                        dest_top as u32,
                                                        w,
                                                        h,
                                                        &bgra,
                                                    );
                                                    let mut guard = ws.lock().unwrap();
                                                    let _ = guard.write(tungstenite::Message::Binary(frame));
                                                }
                                            }
                                            _ => {}
                                        }
                                    });

                                    if let Err(e) = read_result {
                                        info!("RDP read error: {:?}", e);
                                        running = false;
                                    }
                                }

                                // Send closed notification
                                {
                                    let mut guard = ws.lock().unwrap();
                                    let _ = guard.write(tungstenite::Message::Binary(
                                        frame::encode_closed(),
                                    ));
                                }

                                if let Some(t) = ws_read_thread {
                                    let _ = t.join();
                                }
                            }
                            Err(e) => {
                                info!("WebSocket accept error: {}", e);
                            }
                        }
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(std::time::Duration::from_millis(50));
                    }
                    Err(e) => {
                        info!("Accept error: {}", e);
                        thread::sleep(std::time::Duration::from_millis(100));
                    }
                }
            }

            Ok::<(), String>(())
        })
        .map_err(|e| format!("Failed to spawn RDP relay thread: {}", e))?;

    Ok(RdpSessionResult { ws_port, shutdown: shutdown_tx })
}

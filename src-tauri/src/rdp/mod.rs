pub mod frame;
pub mod input;

use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;

use log::info;
use rdp::core::client::Connector;
use rdp::core::event::RdpEvent;
use tokio::sync::oneshot;

use crate::security::input::validate_host;

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
                                let mut framebuffer = vec![0u8; width as usize * height as usize * 4];
                                for px in (0..framebuffer.len()).step_by(4) {
                                    framebuffer[px + 3] = 0xFF;
                                }

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
                                                let dest_left = bitmap.dest_left as usize;
                                                let dest_top = bitmap.dest_top as usize;
                                                let dest_right = bitmap.dest_right as usize;
                                                let dest_bottom = bitmap.dest_bottom as usize;
                                                let buf_width = bitmap.width as usize;

                                                let data = if bitmap.is_compress {
                                                    bitmap.decompress().unwrap_or_default()
                                                } else {
                                                    bitmap.data
                                                };

                                                let region_w = dest_right.saturating_sub(dest_left) + 1;
                                                let region_h = dest_bottom.saturating_sub(dest_top) + 1;
                                                if !data.is_empty() && region_w > 0 && region_h > 0
                                                    && dest_right < width as usize
                                                    && dest_bottom < height as usize
                                                {
                                                    let screen_w = width as usize;
                                                    for i in 0..region_h {
                                                        let src_start = i * buf_width * 4;
                                                        let dst_start = ((dest_top + i) * screen_w + dest_left) * 4;
                                                        if let (Some(src), Some(dst)) = (
                                                            data.get(src_start..src_start + region_w * 4),
                                                            framebuffer.get_mut(dst_start..dst_start + region_w * 4),
                                                        ) {
                                                            dst.copy_from_slice(src);
                                                        }
                                                    }
                                                    let frame = frame::encode_frame(
                                                        0,
                                                        0,
                                                        width,
                                                        height,
                                                        &framebuffer,
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

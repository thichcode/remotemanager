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

/// Copy a bitmap region into the full-screen framebuffer handling stride.
///
/// `data` is `buf_width`-wide (may be larger than the dest region width).
/// Returns the region rectangle `(left, top, right, bottom)` clipped to the
/// screen, or `None` when there is nothing to draw.
fn blit_bitmap_region(
    framebuffer: &mut [u8],
    screen_w: usize,
    screen_h: usize,
    data: &[u8],
    buf_width: usize,
    dest_left: u16,
    dest_top: u16,
    dest_right: u16,
    dest_bottom: u16,
) -> Option<(usize, usize, usize, usize)> {
    let left = dest_left as usize;
    let top = dest_top as usize;
    let right = dest_right as usize;
    let bottom = dest_bottom as usize;
    if data.is_empty() || right < left || bottom < top {
        return None;
    }
    let region_w = right - left + 1;
    let region_h = bottom - top + 1;
    if region_w == 0 || region_h == 0 || right >= screen_w || bottom >= screen_h {
        return None;
    }
    for i in 0..region_h {
        let src_start = i * buf_width * 4;
        let dst_start = ((top + i) * screen_w + left) * 4;
        if let (Some(src), Some(dst)) = (
            data.get(src_start..src_start + region_w * 4),
            framebuffer.get_mut(dst_start..dst_start + region_w * 4),
        ) {
            dst.copy_from_slice(src);
        }
    }
    Some((left, top, right, bottom))
}

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

                                // Tell the client the screen size once.
                                {
                                    let mut guard = ws.lock().unwrap();
                                    let _ = guard.write(tungstenite::Message::Binary(
                                        frame::encode_init(width, height),
                                    ));
                                }

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
                                                let dest_left = bitmap.dest_left;
                                                let dest_top = bitmap.dest_top;
                                                let dest_right = bitmap.dest_right;
                                                let dest_bottom = bitmap.dest_bottom;
                                                let buf_width = bitmap.width as usize;
                                                let data = bitmap.decompress().unwrap_or_default();
                                                if let Some((left, top, right, bottom)) = blit_bitmap_region(
                                                    &mut framebuffer,
                                                    width as usize,
                                                    height as usize,
                                                    &data,
                                                    buf_width,
                                                    dest_left,
                                                    dest_top,
                                                    dest_right,
                                                    dest_bottom,
                                                ) {
                                                    let region_w = right - left + 1;
                                                    let region_h = bottom - top + 1;
                                                    let mut region = Vec::with_capacity(region_w * region_h * 4);
                                                    for i in 0..region_h {
                                                        let start = ((top + i) * width as usize + left) * 4;
                                                        region.extend_from_slice(
                                                            &framebuffer[start..start + region_w * 4],
                                                        );
                                                    }
                                                    let frame = frame::encode_frame(
                                                        left as u32,
                                                        top as u32,
                                                        region_w as u16,
                                                        region_h as u16,
                                                        &region,
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

#[cfg(test)]
mod tests {
    use super::blit_bitmap_region;

    fn bgra(r: u8, g: u8, b: u8) -> [u8; 4] {
        [b, g, r, 0xFF]
    }

    #[test]
    fn blit_region_top_left() {
        let screen_w = 4;
        let screen_h = 4;
        let mut fb = vec![0u8; screen_w * screen_h * 4];
        // 2x2 bitmap buffer, region at dest (0,0)-(1,1)
        let mut data = Vec::new();
        data.extend_from_slice(&bgra(255, 0, 0));
        data.extend_from_slice(&bgra(0, 255, 0));
        data.extend_from_slice(&bgra(0, 0, 255));
        data.extend_from_slice(&bgra(255, 255, 0));

        let rect = blit_bitmap_region(&mut fb, screen_w, screen_h, &data, 2, 0, 0, 1, 1);
        assert_eq!(rect, Some((0, 0, 1, 1)));
        assert_eq!(&fb[0..4], &bgra(255, 0, 0));
        assert_eq!(&fb[4..8], &bgra(0, 255, 0));
        assert_eq!(&fb[16..20], &bgra(0, 0, 255));
        assert_eq!(&fb[20..24], &bgra(255, 255, 0));
    }

    #[test]
    fn blit_region_handles_stride() {
        // bitmap buffer is 3 wide but dest region is only 2 wide
        let screen_w = 4;
        let screen_h = 4;
        let mut fb = vec![0u8; screen_w * screen_h * 4];
        let mut data = Vec::new();
        data.extend_from_slice(&bgra(1, 1, 1));
        data.extend_from_slice(&bgra(2, 2, 2));
        data.extend_from_slice(&bgra(9, 9, 9)); // padding / ignored
        data.extend_from_slice(&bgra(3, 3, 3));
        data.extend_from_slice(&bgra(4, 4, 4));
        data.extend_from_slice(&bgra(9, 9, 9));

        let rect = blit_bitmap_region(&mut fb, screen_w, screen_h, &data, 3, 0, 0, 1, 1);
        assert_eq!(rect, Some((0, 0, 1, 1)));
        assert_eq!(&fb[0..4], &bgra(1, 1, 1));
        assert_eq!(&fb[4..8], &bgra(2, 2, 2));
        assert_eq!(&fb[16..20], &bgra(3, 3, 3));
        assert_eq!(&fb[20..24], &bgra(4, 4, 4));
        // padding column must not have leaked into screen column 2
        assert_eq!(&fb[8..12], &[0, 0, 0, 0]);
        assert_eq!(&fb[24..28], &[0, 0, 0, 0]);
    }

    #[test]
    fn blit_region_offset_destination() {
        let screen_w = 5;
        let screen_h = 5;
        let mut fb = vec![0u8; screen_w * screen_h * 4];
        let mut data = Vec::new();
        data.extend_from_slice(&bgra(1, 0, 0));
        data.extend_from_slice(&bgra(2, 0, 0));
        data.extend_from_slice(&bgra(3, 0, 0));
        data.extend_from_slice(&bgra(4, 0, 0));

        let rect = blit_bitmap_region(&mut fb, screen_w, screen_h, &data, 2, 2, 1, 3, 2);
        assert_eq!(rect, Some((2, 1, 3, 2)));
        let expected_row1 = [bgra(1, 0, 0), bgra(2, 0, 0)].concat();
        let expected_row2 = [bgra(3, 0, 0), bgra(4, 0, 0)].concat();
        assert_eq!(&fb[(1 * screen_w + 2) * 4..(1 * screen_w + 4) * 4], &expected_row1[..]);
        assert_eq!(&fb[(2 * screen_w + 2) * 4..(2 * screen_w + 4) * 4], &expected_row2[..]);
    }

    #[test]
    fn blit_region_out_of_bounds_is_rejected() {
        let mut fb = vec![0u8; 4 * 4 * 4];
        let data = vec![0u8; 16];
        assert!(blit_bitmap_region(&mut fb, 4, 4, &data, 4, 0, 0, 5, 1).is_none());
        assert!(blit_bitmap_region(&mut fb, 4, 4, &data, 4, 0, 0, 1, 9).is_none());
        assert!(blit_bitmap_region(&mut fb, 4, 4, &[], 4, 0, 0, 1, 1).is_none());
    }
}

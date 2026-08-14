/// Wire protocol frame encoding for the embedded RDP relay.
///
/// Server → Client:
///   0x01 = Frame (bitmap update, may be a partial region)
///   0x02 = Session closed
///   0x03 = Session init (screen size, sent once after connect)
///
/// Client → Server:
///   0x10 = Mouse event
///   0x11 = Keyboard event
///   0x12 = Resize

pub const MSG_FRAME: u8 = 0x01;
pub const MSG_CLOSED: u8 = 0x02;
pub const MSG_INIT: u8 = 0x03;
pub const MSG_MOUSE: u8 = 0x10;
pub const MSG_KEYBOARD: u8 = 0x11;
pub const MSG_RESIZE: u8 = 0x12;

/// Encode a session-init message carrying the screen size.
///
/// Layout (all little-endian):
///   [0]      u8   message type (0x03)
///   [1..3]   u16  screen width
///   [3..5]   u16  screen height
pub fn encode_init(width: u16, height: u16) -> Vec<u8> {
    let mut buf = Vec::with_capacity(5);
    buf.push(MSG_INIT);
    buf.extend_from_slice(&width.to_le_bytes());
    buf.extend_from_slice(&height.to_le_bytes());
    buf
}

/// Encode a bitmap frame update.
///
/// Layout (all little-endian):
///   [0]      u8   message type (0x01)
///   [1..3]   u16  width
///   [3..5]   u16  height
///   [5..9]   u32  x offset
///   [9..13]  u32  y offset
///   [13..]   BGRA pixel data (width * height * 4 bytes)
pub fn encode_frame(x: u32, y: u32, width: u16, height: u16, bgra: &[u8]) -> Vec<u8> {
    let mut buf = Vec::with_capacity(13 + bgra.len());
    buf.push(MSG_FRAME);
    buf.extend_from_slice(&width.to_le_bytes());
    buf.extend_from_slice(&height.to_le_bytes());
    buf.extend_from_slice(&x.to_le_bytes());
    buf.extend_from_slice(&y.to_le_bytes());
    buf.extend_from_slice(bgra);
    buf
}

/// Encode a session-closed notification.
pub fn encode_closed() -> Vec<u8> {
    vec![MSG_CLOSED]
}

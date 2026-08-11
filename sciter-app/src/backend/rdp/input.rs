/// Input event parsing from the WebSocket wire protocol.
///
/// Client → Server messages:
///   0x10 = Mouse event  (7 bytes total)
///   0x11 = Keyboard event (4 bytes total)
///   0x12 = Resize event (5 bytes total)

use rdp::core::event::{KeyboardEvent, PointerButton, PointerEvent, RdpEvent};

#[derive(Debug)]
pub enum ClientEvent {
    Mouse {
        x: u16,
        y: u16,
        button_mask: u8,
        event_type: u8,
    },
    Keyboard {
        scan_code: u16,
        down: bool,
    },
    Resize {
        width: u16,
        height: u16,
    },
}

impl ClientEvent {
    /// Parse a client message from raw bytes.
    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.is_empty() {
            return None;
        }
        match data[0] {
            0x10 if data.len() >= 7 => {
                let x = u16::from_le_bytes([data[1], data[2]]);
                let y = u16::from_le_bytes([data[3], data[4]]);
                let button_mask = data[5];
                let event_type = data[6];
                Some(ClientEvent::Mouse { x, y, button_mask, event_type })
            }
            0x11 if data.len() >= 4 => {
                let scan_code = u16::from_le_bytes([data[1], data[2]]);
                let down = data[3] != 0;
                Some(ClientEvent::Keyboard { scan_code, down })
            }
            0x12 if data.len() >= 5 => {
                let width = u16::from_le_bytes([data[1], data[2]]);
                let height = u16::from_le_bytes([data[3], data[4]]);
                Some(ClientEvent::Resize { width, height })
            }
            _ => None,
        }
    }

    /// Convert to an RDP event for `client.write()`.
    pub fn to_rdp_event(&self) -> Option<RdpEvent> {
        match self {
            ClientEvent::Mouse { x, y, button_mask, event_type } => {
                let button = match *button_mask {
                    0x01 => PointerButton::Left,
                    0x02 => PointerButton::Right,
                    0x04 => PointerButton::Middle,
                    _ => PointerButton::Left,
                };
                if *event_type == 0 {
                    Some(RdpEvent::Pointer(PointerEvent {
                        x: *x,
                        y: *y,
                        button: PointerButton::Left,
                        down: false,
                    }))
                } else {
                    Some(RdpEvent::Pointer(PointerEvent {
                        x: *x,
                        y: *y,
                        button,
                        down: *event_type == 1,
                    }))
                }
            }
            ClientEvent::Keyboard { scan_code, down } => {
                Some(RdpEvent::Key(KeyboardEvent {
                    code: *scan_code,
                    down: *down,
                }))
            }
            ClientEvent::Resize { .. } => None,
        }
    }
}

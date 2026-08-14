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
                    _ => PointerButton::None,
                };
                // event_type: 0 = move, 1 = button down, 2 = button up
                let down = *event_type == 1;
                Some(RdpEvent::Pointer(PointerEvent {
                    x: *x,
                    y: *y,
                    button,
                    down,
                }))
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

#[cfg(test)]
mod tests {
    use super::*;

    fn mouse(x: u16, y: u16, mask: u8, event_type: u8) -> PointerEvent {
        match (ClientEvent::Mouse { x, y, button_mask: mask, event_type })
            .to_rdp_event()
            .expect("mouse maps to event")
        {
            RdpEvent::Pointer(p) => p,
            _ => panic!("expected pointer event"),
        }
    }

    fn is_button(p: &PointerEvent, expected: PointerButton, down: bool) -> bool {
        p.button == expected && p.down == down
    }

    #[test]
    fn mouse_move_is_not_a_click() {
        // Moving the mouse must NOT tell the server button 1 is pressed,
        // otherwise the server thinks we are dragging and selects text.
        let p = mouse(100, 200, 0, 0);
        assert!(is_button(&p, PointerButton::None, false));
        assert_eq!((p.x, p.y), (100, 200));
    }

    #[test]
    fn mouse_move_preserves_coordinates() {
        let p = mouse(0, 768, 0, 0);
        assert_eq!((p.x, p.y), (0, 768));
    }

    #[test]
    fn mouse_left_down() {
        let p = mouse(10, 20, 0x01, 1);
        assert!(is_button(&p, PointerButton::Left, true));
    }

    #[test]
    fn mouse_left_up() {
        let p = mouse(10, 20, 0x01, 2);
        assert!(is_button(&p, PointerButton::Left, false));
    }

    #[test]
    fn mouse_right_down() {
        let p = mouse(10, 20, 0x02, 1);
        assert!(is_button(&p, PointerButton::Right, true));
    }

    #[test]
    fn mouse_middle_down() {
        let p = mouse(10, 20, 0x04, 1);
        assert!(is_button(&p, PointerButton::Middle, true));
    }

    #[test]
    fn keyboard_down_and_up() {
        let d = ClientEvent::Keyboard { scan_code: 0x1E, down: true }
            .to_rdp_event()
            .expect("keyboard down");
        match d {
            RdpEvent::Key(k) => { assert!(k.down); assert_eq!(k.code, 0x1E); }
            _ => panic!("expected key event"),
        }

        let u = ClientEvent::Keyboard { scan_code: 0x1E, down: false }
            .to_rdp_event()
            .expect("keyboard up");
        match u {
            RdpEvent::Key(k) => { assert!(!k.down); }
            _ => panic!("expected key event"),
        }
    }

    #[test]
    fn resize_is_ignored() {
        assert!(ClientEvent::Resize { width: 800, height: 600 }.to_rdp_event().is_none());
    }
}

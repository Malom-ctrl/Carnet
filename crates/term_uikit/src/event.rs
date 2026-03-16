use std::io::{self, Read};

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum Key {
    Char(char),
    Enter,
    Escape,
    Backspace,
    Up,
    Down,
    Left,
    Right,
    PageUp,
    PageDown,
    Unknown,
}

pub struct InputHandler;

impl InputHandler {
    pub fn read_key() -> Key {
        let mut buffer = [0u8; 4];
        let stdin = io::stdin();
        let mut handle = stdin.lock();

        if let Ok(size) = handle.read(&mut buffer) {
            if size == 0 {
                return Key::Unknown;
            }

            match buffer[0] {
                b'\x1b' => {
                    if size == 1 {
                        return Key::Escape;
                    }
                    if size >= 3 && buffer[1] == b'[' {
                        match buffer[2] {
                            b'A' => return Key::Up,
                            b'B' => return Key::Down,
                            b'C' => return Key::Right,
                            b'D' => return Key::Left,
                            b'5' if size >= 4 && buffer[3] == b'~' => return Key::PageUp,
                            b'6' if size >= 4 && buffer[3] == b'~' => return Key::PageDown,
                            _ => return Key::Unknown,
                        }
                    }
                    Key::Escape
                }
                b'\n' | b'\r' => Key::Enter,
                127 => Key::Backspace,
                c if c.is_ascii() => Key::Char(c as char),
                _ => Key::Unknown,
            }
        } else {
            Key::Unknown
        }
    }
}

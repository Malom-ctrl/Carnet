use std::io::{self, Read};

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum Key {
    Char(char),
    Enter,
    Escape,
    Backspace,
    Up,
    Down,
    Unknown,
}

pub struct InputHandler;

impl InputHandler {
    pub fn read_key() -> Key {
        let mut buffer = [0u8; 3];
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
                    if size == 3 && buffer[1] == b'[' {
                        match buffer[2] {
                            b'A' => return Key::Up,
                            b'B' => return Key::Down,
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

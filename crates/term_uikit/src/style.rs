#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Color {
    Reset,
    Black,
    Red,
    Green,
    Yellow,
    Blue,
    Magenta,
    Cyan,
    White,
    Rgb(u8, u8, u8),
    Indexed(u8),
}

impl std::fmt::Display for Color {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Color::Reset => write!(f, "0"),
            Color::Black => write!(f, "30"),
            Color::Red => write!(f, "31"),
            Color::Green => write!(f, "32"),
            Color::Yellow => write!(f, "33"),
            Color::Blue => write!(f, "34"),
            Color::Magenta => write!(f, "35"),
            Color::Cyan => write!(f, "36"),
            Color::White => write!(f, "37"),
            Color::Rgb(r, g, b) => write!(f, "38;2;{};{};{}", r, g, b),
            Color::Indexed(i) => write!(f, "38;5;{}", i),
        }
    }
}

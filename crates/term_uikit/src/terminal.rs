use std::io::{self, BufWriter, Write};
use std::mem;
use std::os::unix::io::AsRawFd;

#[derive(Clone, Debug, PartialEq)]
pub struct Cell {
    pub symbol: String,
    pub fg: String,
    pub bg: String,
    pub image_hash: Option<u64>,
    pub image_data: Option<String>, // Only on the anchor cell
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            symbol: " ".to_string(),
            fg: "0".to_string(),
            bg: "".to_string(),
            image_hash: None,
            image_data: None,
        }
    }
}

pub struct Buffer {
    pub width: u16,
    pub height: u16,
    pub cells: Vec<Cell>,
}

impl Buffer {
    pub fn new(width: u16, height: u16) -> Self {
        Self {
            width,
            height,
            cells: vec![Cell::default(); (width as usize) * (height as usize)],
        }
    }

    pub fn get_mut(&mut self, x: u16, y: u16) -> Option<&mut Cell> {
        if x < self.width && y < self.height {
            Some(&mut self.cells[(y as usize) * (self.width as usize) + (x as usize)])
        } else {
            None
        }
    }

    pub fn reset(&mut self) {
        for cell in &mut self.cells {
            *cell = Cell::default();
        }
    }
}

pub struct Terminal {
    stdout: BufWriter<io::Stdout>,
    original_termios: libc::termios,
    pub current_buffer: Buffer,
    pub next_buffer: Buffer,
    current_fg: String,
    current_bg: String,
    cursor_x: u16,
    cursor_y: u16,
}

impl Terminal {
    pub fn new() -> io::Result<Self> {
        let stdout = BufWriter::with_capacity(128 * 1024, io::stdout());
        let fd = io::stdout().as_raw_fd();

        let mut original_termios: libc::termios = unsafe { mem::zeroed() };
        if unsafe { libc::tcgetattr(fd, &mut original_termios) } != 0 {
            return Err(io::Error::last_os_error());
        }

        let mut term = Self {
            stdout,
            original_termios,
            current_buffer: Buffer::new(0, 0),
            next_buffer: Buffer::new(0, 0),
            current_fg: "0".to_string(),
            current_bg: "".to_string(),
            cursor_x: 0,
            cursor_y: 0,
        };

        let (rows, cols) = term.size();
        term.current_buffer = Buffer::new(cols, rows);
        term.next_buffer = Buffer::new(cols, rows);

        term.enter_raw_mode()?;
        term.hide_cursor()?;
        term.clear_screen()?;
        term.flush_raw()?;
        Ok(term)
    }

    fn enter_raw_mode(&mut self) -> io::Result<()> {
        let mut raw = self.original_termios;
        raw.c_lflag &= !(libc::ICANON | libc::ECHO);
        raw.c_cc[libc::VMIN] = 1;
        raw.c_cc[libc::VTIME] = 0;
        if unsafe { libc::tcsetattr(io::stdout().as_raw_fd(), libc::TCSAFLUSH, &raw) } != 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    fn exit_raw_mode(&mut self) -> io::Result<()> {
        if unsafe { libc::tcsetattr(io::stdout().as_raw_fd(), libc::TCSAFLUSH, &self.original_termios) } != 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    pub fn clear_images(&mut self) -> io::Result<()> {
        write!(self.stdout, "\x1b_Ga=d,d=A\x1b\\")?;
        Ok(())
    }

    pub fn clear_screen(&mut self) -> io::Result<()> {
        write!(self.stdout, "\x1b[2J\x1b[H")?;
        self.cursor_x = 0;
        self.cursor_y = 0;
        Ok(())
    }

    pub fn clear(&mut self) -> io::Result<()> {
        self.next_buffer.reset();
        Ok(())
    }

    pub fn resize(&mut self, width: u16, height: u16) {
        if self.next_buffer.width != width || self.next_buffer.height != height {
            self.next_buffer = Buffer::new(width, height);
            self.current_buffer = Buffer::new(width, height);
            let _ = self.clear_screen();
        }
    }

    pub fn flush(&mut self) -> io::Result<()> {
        let mut last_fg = String::new();
        let mut last_bg = String::new();
        let mut last_x = u16::MAX;
        let mut last_y = u16::MAX;

        for y in 0..self.next_buffer.height {
            for x in 0..self.next_buffer.width {
                let idx = (y as usize) * (self.next_buffer.width as usize) + (x as usize);
                let next_cell = &self.next_buffer.cells[idx];
                let current_cell = &self.current_buffer.cells[idx];

                if next_cell != current_cell {
                    // Check if only image changed or both
                    let symbol_changed = next_cell.symbol != current_cell.symbol 
                        || next_cell.fg != current_cell.fg 
                        || next_cell.bg != current_cell.bg;
                    
                    let image_changed = next_cell.image_hash != current_cell.image_hash;

                    if image_changed {
                        if let Some(data) = &next_cell.image_data {
                            // Print image directly. move_to_raw is used inside print_image logic usually,
                            // but here we are already at the right spot if we move cursor.
                            write!(self.stdout, "\x1b[{};{}H", y + 1, x + 1)?;
                            write!(self.stdout, "{}", data)?;
                            last_x = u16::MAX; // Invalidate cursor
                        }
                    }

                    if symbol_changed {
                        if x != last_x || y != last_y {
                            write!(self.stdout, "\x1b[{};{}H", y + 1, x + 1)?;
                        }

                        if next_cell.fg != last_fg || next_cell.bg != last_bg {
                            write!(self.stdout, "\x1b[0")?;
                            if !next_cell.fg.is_empty() && next_cell.fg != "0" {
                                write!(self.stdout, ";{}", next_cell.fg)?;
                            }
                            if !next_cell.bg.is_empty() {
                                write!(self.stdout, ";{}", next_cell.bg)?;
                            }
                            write!(self.stdout, "m")?;
                            last_fg = next_cell.fg.clone();
                            last_bg = next_cell.bg.clone();
                        }

                        write!(self.stdout, "{}", next_cell.symbol)?;
                        last_x = x + 1;
                        last_y = y;
                    }
                }
            }
        }

        write!(self.stdout, "\x1b[0m")?;
        self.stdout.flush()?;

        self.current_buffer.cells.clone_from_slice(&self.next_buffer.cells);
        self.next_buffer.reset();
        Ok(())
    }

    pub fn flush_raw(&mut self) -> io::Result<()> {
        self.stdout.flush()
    }

    pub fn hide_cursor(&mut self) -> io::Result<()> {
        write!(self.stdout, "\x1b[?25l")?;
        Ok(())
    }

    pub fn show_cursor(&mut self) -> io::Result<()> {
        write!(self.stdout, "\x1b[?25h")?;
        Ok(())
    }

    pub fn move_to(&mut self, row: u16, col: u16) -> io::Result<()> {
        self.cursor_x = col;
        self.cursor_y = row;
        Ok(())
    }

    pub fn move_to_raw(&mut self, row: u16, col: u16) -> io::Result<()> {
        write!(self.stdout, "\x1b[{};{}H", row + 1, col + 1)?;
        self.cursor_x = col;
        self.cursor_y = row;
        Ok(())
    }

    pub fn print(&mut self, text: &str) -> io::Result<()> {
        let mut x = self.cursor_x;
        for c in text.chars() {
            if let Some(cell) = self.next_buffer.get_mut(x, self.cursor_y) {
                cell.symbol = c.to_string();
                cell.fg = self.current_fg.clone();
                cell.bg = self.current_bg.clone();
            }
            x += 1;
        }
        self.cursor_x = x;
        Ok(())
    }

    pub fn print_raw(&mut self, text: &str) -> io::Result<()> {
        write!(self.stdout, "{}", text)?;
        Ok(())
    }

    pub fn set_image(&mut self, x: u16, y: u16, w: u16, h: u16, hash: u64, data: String) {
        // Mark all cells covered by the image with the hash
        for ry in y..(y + h) {
            for rx in x..(x + w) {
                if let Some(cell) = self.next_buffer.get_mut(rx, ry) {
                    cell.image_hash = Some(hash);
                    if rx == x && ry == y {
                        cell.image_data = Some(data.clone());
                    }
                }
            }
        }
    }

    pub fn set_color(&mut self, color_code: &str) -> io::Result<()> {
        self.current_fg = color_code.to_string();
        Ok(())
    }

    pub fn reset_color(&mut self) -> io::Result<()> {
        self.current_fg = "0".to_string();
        self.current_bg = "".to_string();
        Ok(())
    }

    pub fn size(&self) -> (u16, u16) {
        let mut winsize = libc::winsize { ws_row: 0, ws_col: 0, ws_xpixel: 0, ws_ypixel: 0 };
        unsafe {
            if libc::ioctl(io::stdout().as_raw_fd(), libc::TIOCGWINSZ, &mut winsize) == 0
                && winsize.ws_row > 0 && winsize.ws_col > 0 {
                return (winsize.ws_row, winsize.ws_col);
            }
        }
        let rows = std::env::var("LINES").ok().and_then(|s| s.parse().ok());
        let cols = std::env::var("COLUMNS").ok().and_then(|s| s.parse().ok());
        (rows.unwrap_or(24), cols.unwrap_or(80))
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        let _ = self.show_cursor();
        let _ = self.exit_raw_mode();
        let _ = self.stdout.write_all(b"\x1b[0m\x1b[2J\x1b[H");
        let _ = self.stdout.flush();
    }
}

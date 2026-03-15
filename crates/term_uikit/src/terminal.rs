use std::io::{self, BufWriter, Write};
use std::mem;
use std::os::unix::io::AsRawFd;

pub struct Terminal {
    stdout: BufWriter<io::Stdout>,
    original_termios: libc::termios,
}

impl Terminal {
    pub fn new() -> io::Result<Self> {
        let stdout = BufWriter::with_capacity(128 * 1024, io::stdout());
        let fd = io::stdout().as_raw_fd();

        // Capture original terminal state
        let mut original_termios: libc::termios = unsafe { mem::zeroed() };
        if unsafe { libc::tcgetattr(fd, &mut original_termios) } != 0 {
            return Err(io::Error::last_os_error());
        }

        let mut term = Self {
            stdout,
            original_termios,
        };

        // Enter raw mode
        term.enter_raw_mode()?;
        term.hide_cursor()?;
        term.clear()?;
        term.flush()?;
        Ok(term)
    }

    fn enter_raw_mode(&mut self) -> io::Result<()> {
        let mut raw = self.original_termios;

        // Modify flags for raw mode:
        // - c_lflag: Turn off ICANON (canonical mode) and ECHO (input echoing)
        // - c_cc: Set VMIN and VTIME for non-blocking-ish reads if needed
        raw.c_lflag &= !(libc::ICANON | libc::ECHO);

        // VMIN = 1, VTIME = 0 means read() will block until at least one byte is available
        raw.c_cc[libc::VMIN] = 1;
        raw.c_cc[libc::VTIME] = 0;

        if unsafe { libc::tcsetattr(io::stdout().as_raw_fd(), libc::TCSAFLUSH, &raw) } != 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    fn exit_raw_mode(&mut self) -> io::Result<()> {
        if unsafe {
            libc::tcsetattr(
                io::stdout().as_raw_fd(),
                libc::TCSAFLUSH,
                &self.original_termios,
            )
        } != 0
        {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    pub fn clear_images(&mut self) -> io::Result<()> {
        write!(self.stdout, "\x1b_Ga=d,d=A\x1b\\")?;
        Ok(())
    }

    pub fn clear(&mut self) -> io::Result<()> {
        write!(self.stdout, "\x1b[2J\x1b[H")?;
        Ok(())
    }

    pub fn flush(&mut self) -> io::Result<()> {
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
        write!(self.stdout, "\x1b[{};{}H", row + 1, col + 1)?;
        Ok(())
    }

    pub fn print(&mut self, text: &str) -> io::Result<()> {
        write!(self.stdout, "{}", text)?;
        Ok(())
    }

    pub fn set_color(&mut self, color_code: &str) -> io::Result<()> {
        write!(self.stdout, "\x1b[{}m", color_code)?;
        Ok(())
    }

    pub fn reset_color(&mut self) -> io::Result<()> {
        write!(self.stdout, "\x1b[0m")?;
        Ok(())
    }

    pub fn size(&self) -> (u16, u16) {
        let mut winsize = libc::winsize {
            ws_row: 0,
            ws_col: 0,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };

        unsafe {
            if libc::ioctl(io::stdout().as_raw_fd(), libc::TIOCGWINSZ, &mut winsize) == 0
                && winsize.ws_row > 0
                && winsize.ws_col > 0
            {
                return (winsize.ws_row, winsize.ws_col);
            }
        }

        // Fallback to environment variables
        let rows = std::env::var("LINES").ok().and_then(|s| s.parse().ok());
        let cols = std::env::var("COLUMNS").ok().and_then(|s| s.parse().ok());

        if let (Some(r), Some(c)) = (rows, cols) {
            return (r, c);
        }

        (24, 80)
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        self.clear().ok();
        self.show_cursor().ok();
        self.exit_raw_mode().ok();
        self.flush().ok();
    }
}

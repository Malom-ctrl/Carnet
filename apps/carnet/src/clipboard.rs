use crate::config::Config;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::process::{Command, Stdio};

/// Represent the content type captured from the clipboard.
#[derive(Debug, Clone)]
pub enum ClipboardContent {
    Text(String),
    Image(Vec<u8>), // Raw bytes (PNG, JPEG, etc.)
}

/// A wrapper for interaction with the `wl-clipboard` suite.
pub struct ClipboardManager;

impl ClipboardManager {
    pub fn capture(config: &Config) -> Option<ClipboardContent> {
        let (content, _) = Self::capture_with_conversion(config);
        content
    }

    pub fn capture_with_conversion(config: &Config) -> (Option<ClipboardContent>, bool) {
        if let Some(image_data) = Self::get_image() {
            return (Some(ClipboardContent::Image(image_data)), false);
        }

        if config.auto_convert_image_uri && let Some(image_data) = get_image_from_uri_list() {
            return (Some(ClipboardContent::Image(image_data)), true);
        }

        if let Some(text) = Self::get_text() {
            return (Some(ClipboardContent::Text(text)), false);
        }

        (None, false)
    }

    /// Sets the clipboard content using `wl-copy`.
    pub fn copy(content: &ClipboardContent, config: &Config) -> std::io::Result<()> {
        Self::copy_internal(content, config, false)
    }

    /// Sets the clipboard content and optionally stays in foreground.
    pub fn copy_and_wait(content: &ClipboardContent, config: &Config) -> std::io::Result<()> {
        Self::copy_internal(content, config, true)
    }

    /// Clears both regular and primary clipboards.
    pub fn clear() -> std::io::Result<()> {
        Command::new("wl-copy").arg("--clear").status()?;
        Command::new("wl-copy")
            .arg("--primary")
            .arg("--clear")
            .status()?;
        Ok(())
    }

    fn copy_internal(
        content: &ClipboardContent,
        config: &Config,
        foreground: bool,
    ) -> std::io::Result<()> {
        let (mime, data) = match content {
            ClipboardContent::Text(text) => ("text/plain", text.as_bytes().to_vec()),
            ClipboardContent::Image(data) => {
                let mime = if data.starts_with(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]) {
                    "image/png"
                } else if data.starts_with(&[0xFF, 0xD8, 0xFF]) {
                    "image/jpeg"
                } else {
                    "image/png"
                };
                (mime, data.clone())
            }
        };

        if data.is_empty() {
            return Ok(());
        }

        // Create a temporary file with secure permissions (0600)
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let tmp_path =
            std::env::temp_dir().join(format!("carnet-cb-{}-{}", std::process::id(), timestamp));

        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .mode(0o600)
            .open(&tmp_path)?;
        file.write_all(&data)?;
        drop(file); // Ensure data is flushed and file is closed

        // Direct execution of wl-copy without a shell helper
        let spawn_wl_copy = |primary: bool| -> std::io::Result<std::process::Child> {
            let mut cmd = Command::new("wl-copy");
            cmd.arg("--type").arg(mime);

            if foreground {
                cmd.arg("--foreground");
            }
            if primary {
                cmd.arg("--primary");
            }

            // Open the file for reading and use it as stdin
            let file = fs::File::open(&tmp_path)?;
            cmd.stdin(Stdio::from(file));
            cmd.stdout(Stdio::null());
            cmd.stderr(Stdio::null());

            cmd.spawn()
        };

        let mut child1 = spawn_wl_copy(false)?;
        let mut child2 = spawn_wl_copy(true)?;

        if foreground {
            // In foreground mode, we wait for wl-copy to finish (which happens when selection is lost)
            let _ = child1.wait();
            let _ = child2.wait();
        } else {
            // Wait a bit to ensure wl-copy has started reading from the file
            std::thread::sleep(std::time::Duration::from_millis(
                config.clipboard_sync_delay_ms,
            ));
        }

        // Cleanup the temporary file
        let _ = fs::remove_file(&tmp_path);

        Ok(())
    }

    fn get_text() -> Option<String> {
        let output = Command::new("wl-paste").arg("--no-newline").output().ok()?;

        if output.status.success() {
            let text = String::from_utf8_lossy(&output.stdout).to_string();
            if !text.is_empty() {
                return Some(text);
            }
        }
        None
    }

    fn get_image() -> Option<Vec<u8>> {
        let output = Command::new("wl-paste")
            .args(["--type", "image/png"])
            .output()
            .ok()?;

        if output.status.success() && !output.stdout.is_empty() {
            return Some(output.stdout);
        }

        let output = Command::new("wl-paste")
            .args(["--type", "image/jpeg"])
            .output()
            .ok()?;

        if output.status.success() && !output.stdout.is_empty() {
            return Some(output.stdout);
        }

        None
    }
}

fn get_image_from_uri_list() -> Option<Vec<u8>> {
    for path in get_image_paths_from_uri_list() {
        if let Ok(data) = fs::read(path) {
            return Some(data);
        }
    }
    None
}

pub fn get_raw_uri_list_output() -> Option<String> {
    if let Ok(output) = Command::new("wl-paste").arg("--list-types").output() {
        let types = String::from_utf8_lossy(&output.stdout);
        if !types.contains("text/uri-list") {
            return None;
        }
    } else {
        return None;
    }

    let output = Command::new("wl-paste")
        .args(["--type", "text/uri-list"])
        .output()
        .ok()?;

    if output.status.success() {
        Some(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        None
    }
}

pub fn parse_uri_list(content: &str) -> Vec<String> {
    let mut paths = Vec::new();
    for line in content.lines() {
        let line = line.trim();
        if let Some(path_str) = line.strip_prefix("file://") {
            let decoded_path = decode_uri_path(path_str);
            if !decoded_path.is_empty() {
                paths.push(decoded_path);
            }
        }
    }
    paths
}

pub fn filter_image_paths(paths: &[String]) -> Vec<String> {
    let mut valid_paths = Vec::new();
    for decoded_path in paths {
        let path = std::path::Path::new(&decoded_path);
        if let Ok(meta) = fs::metadata(path) {
            if !meta.is_file() {
                continue;
            }
            // Check magic numbers to verify it is an image
            if let Ok(mut file) = fs::File::open(path) {
                use std::io::Read;
                let mut magic = [0u8; 8];
                if file.read_exact(&mut magic).is_ok()
                    && (magic.starts_with(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]) || // PNG
                       magic.starts_with(&[0xFF, 0xD8, 0xFF]))
                    // JPEG
                {
                    valid_paths.push(decoded_path.clone());
                }
            }
        }
    }
    valid_paths
}

pub fn get_image_paths_from_uri_list() -> Vec<String> {
    if let Some(content) = get_raw_uri_list_output() {
        let paths = parse_uri_list(&content);
        filter_image_paths(&paths)
    } else {
        Vec::new()
    }
}

pub fn decode_uri_path(path: &str) -> String {
    let mut bytes = Vec::new();
    let mut chars = path.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '%' {
            let mut hex = String::new();
            if let Some(h1) = chars.next() {
                hex.push(h1);
            }
            if let Some(h2) = chars.next() {
                hex.push(h2);
            }
            if hex.len() == 2 && let Ok(byte) = u8::from_str_radix(&hex, 16) {
                // Reject null and newline characters to prevent path injection
                if byte == 0 || byte == b'\n' || byte == b'\r' {
                    return String::new();
                }
                bytes.push(byte);
                continue;
            }
            bytes.push(b'%');
            bytes.extend_from_slice(hex.as_bytes());
        } else {
            // Reject literal null and newline characters
            if c == '\0' || c == '\n' || c == '\r' {
                return String::new();
            }
            bytes.extend_from_slice(c.encode_utf8(&mut [0; 4]).as_bytes());
        }
    }
    String::from_utf8_lossy(&bytes).into_owned()
}

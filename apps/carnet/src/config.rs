use std::fs;
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct Tool {
    pub name: String,
    pub bin: String,
    pub content_type: String, // "text", "image", "both"
    pub preview: bool,
}

#[derive(Clone, Debug)]
pub struct Config {
    // History
    pub history_max_items: usize,
    pub history_max_item_size: usize,
    pub history_file_name: String,
    pub auto_convert_image_uri: bool,

    // UI
    pub ui_color_primary: String,
    pub ui_color_highlight: String,
    pub ui_color_dim: String,
    pub ui_icon_text: String,
    pub ui_icon_image: String,
    pub ui_icon_prompt: String,
    pub ui_icon_pin: String,
    pub ui_icon_sensitive: String,
    pub ui_border_chars: String, // 6 chars: top-left, top-right, bottom-left, bottom-right, horizontal, vertical
    pub ui_show_newlines: bool,

    // Performance
    pub refresh_rate_ms: u64,
    pub clipboard_sync_delay_ms: u64,

    // Tools
    pub tools: Vec<Tool>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            history_max_items: 100,
            history_max_item_size: 10 * 1024 * 1024,
            history_file_name: "history.bin".to_string(),
            auto_convert_image_uri: false,
            ui_color_primary: "93".to_string(),
            ui_color_highlight: "30;103".to_string(),
            ui_color_dim: "90".to_string(),
            ui_icon_text: "≡".to_string(),
            ui_icon_image: "▨".to_string(),
            ui_icon_prompt: "❯".to_string(),
            ui_icon_pin: "📌".to_string(),
            ui_icon_sensitive: "🔒".to_string(),
            ui_border_chars: "╭╮╰╯─│".to_string(),
            ui_show_newlines: true,
            refresh_rate_ms: 200,
            clipboard_sync_delay_ms: 100,
            tools: Vec::new(),
        }
    }
}

const DEFAULT_CONFIG: &str = r#"# Carnet Configuration File

# --- History Settings ---
# Maximum number of items to keep in history
HISTORY_MAX_ITEMS=100
# Maximum size in bytes for a single history item (default 10MB)
HISTORY_MAX_ITEM_SIZE=10485760
# Name of the history file stored in ~/.local/share/carnet/
HISTORY_FILE_NAME=history.bin
# Automatically detect if a copied path is an image and store/copy the image instead
AUTO_CONVERT_IMAGE_URI=false

# --- UI Theme Settings ---
# Primary color (ANSI color code, e.g., 93 is Bright Yellow)
UI_COLOR_PRIMARY=93
# Highlight color for selected item (ANSI code, e.g., 30;103 is Black on Bright Yellow)
UI_COLOR_HIGHLIGHT=30;103
# Color for dimmed/inactive elements (ANSI code, e.g., 90 is Grey)
UI_COLOR_DIM=90

# Icons for different content types
UI_ICON_TEXT=≡
UI_ICON_IMAGE=▨
UI_ICON_PROMPT=❯
UI_ICON_PIN=📌
UI_ICON_SENSITIVE=🔒

# Border characters (6 characters: top-left, top-right, bottom-left, bottom-right, horizontal, vertical)
UI_BORDER_CHARS=╭╮╰╯─│

# Show newlines (grey background) in preview
UI_SHOW_NEWLINES=true

# --- Performance Settings ---
# UI refresh rate in milliseconds
REFRESH_RATE_MS=200
# Delay in milliseconds to wait for clipboard sync after copy
CLIPBOARD_SYNC_DELAY_MS=100

# --- Tools ---
# Format: TOOL_NAME = Display Name | context | preview_flag | command
# context: text, image, both
# preview_flag: preview, no-preview
# command: The command to run (can contain pipes)

# Text Tools
TOOL_UPPER = Upper Case | text | preview | tr '[:lower:]' '[:upper:]'
TOOL_LOWER = Lower Case | text | preview | tr '[:upper:]' '[:lower:]'
TOOL_B64_ENC = Base64 Encode | text | preview | base64
TOOL_B64_DEC = Base64 Decode | text | preview | base64 -d
TOOL_STRIP = Remove Formatting | text | preview | tr -d '\n\r\t'
TOOL_TRIM = Trim Whitespace | text | preview | xargs
TOOL_JSON_PP = JSON Pretty Print | text | preview | jq .
TOOL_WC = Word Count | text | preview | wc -w
TOOL_SORT = Sort Lines | text | preview | sort
TOOL_UNIQ = Unique Lines | text | preview | sort | uniq
"#;

impl Config {
    pub fn load() -> Self {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let mut config_dir = PathBuf::from(home);
        config_dir.push(".config/carnet");

        // Ensure directory exists
        let _ = fs::create_dir_all(&config_dir);

        let mut path = config_dir;
        path.push("config");

        // Create default config if missing
        if !path.exists() {
            let _ = fs::write(&path, DEFAULT_CONFIG);
        }

        if let Ok(content) = fs::read_to_string(path) {
            Self::parse(&content)
        } else {
            Self::default()
        }
    }

    pub fn parse(content: &str) -> Self {
        let mut config = Self::default();

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            if let Some((key, value)) = line.split_once('=') {
                let key = key.trim();
                let value = value.trim();

                match key {
                    "HISTORY_MAX_ITEMS" => {
                        if let Ok(v) = value.parse() {
                            config.history_max_items = v;
                        }
                    }
                    "HISTORY_MAX_ITEM_SIZE" => {
                        if let Ok(v) = value.parse() {
                            config.history_max_item_size = v;
                        }
                    }
                    "HISTORY_FILE_NAME" => {
                        config.history_file_name = value.to_string();
                    }
                    "AUTO_CONVERT_IMAGE_URI" => {
                        config.auto_convert_image_uri = value.to_lowercase() == "true";
                    }
                    "UI_COLOR_PRIMARY" => {
                        config.ui_color_primary = value.to_string();
                    }
                    "UI_COLOR_HIGHLIGHT" => {
                        config.ui_color_highlight = value.to_string();
                    }
                    "UI_COLOR_DIM" => {
                        config.ui_color_dim = value.to_string();
                    }
                    "UI_ICON_TEXT" => {
                        config.ui_icon_text = value.to_string();
                    }
                    "UI_ICON_IMAGE" => {
                        config.ui_icon_image = value.to_string();
                    }
                    "UI_ICON_PROMPT" => {
                        config.ui_icon_prompt = value.to_string();
                    }
                    "UI_ICON_PIN" => {
                        config.ui_icon_pin = value.to_string();
                    }
                    "UI_ICON_SENSITIVE" => {
                        config.ui_icon_sensitive = value.to_string();
                    }
                    "UI_BORDER_CHARS" => {
                        if value.chars().count() >= 6 {
                            config.ui_border_chars = value.to_string();
                        }
                    }
                    "UI_SHOW_NEWLINES" => {
                        config.ui_show_newlines = value.to_lowercase() == "true";
                    }
                    "REFRESH_RATE_MS" => {
                        if let Ok(v) = value.parse() {
                            config.refresh_rate_ms = v;
                        }
                    }
                    "CLIPBOARD_SYNC_DELAY_MS" => {
                        if let Ok(v) = value.parse() {
                            config.clipboard_sync_delay_ms = v;
                        }
                    }
                    _ => {
                        if key.starts_with("TOOL_") {
                            let parts: Vec<&str> = value.splitn(4, '|').collect();
                            if parts.len() == 4 {
                                config.tools.push(Tool {
                                    name: parts[0].trim().to_string(),
                                    content_type: parts[1].trim().to_lowercase(),
                                    preview: parts[2].trim().to_lowercase() == "preview",
                                    bin: parts[3].trim().to_string(),
                                });
                            }
                        }
                    }
                }
            }
        }

        config
    }
}

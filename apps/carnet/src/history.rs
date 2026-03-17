use crate::clipboard::ClipboardContent;
use crate::config::Config;
use std::collections::{HashMap, VecDeque, hash_map::DefaultHasher};
use std::fs::{File, OpenOptions};
use std::hash::{Hash, Hasher};
use std::io::{BufReader, BufWriter, Read, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::PathBuf;

use std::time::SystemTime;

#[derive(Clone)]
pub struct HistoryItem {
    pub id: u64,
    pub content: ClipboardContent,
    pub timestamp: u64,
    pub is_pinned: bool,
    pub is_sensitive: bool,
}

pub struct HistoryManager {
    items: HashMap<u64, HistoryItem>,
    order: VecDeque<u64>,
    path: PathBuf,
    last_mtime: Option<SystemTime>,
    config: Config,
}

impl HistoryManager {
    pub fn new(config: Config) -> Self {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let mut path = PathBuf::from(home);
        path.push(".local/share/carnet");
        std::fs::create_dir_all(&path).ok();
        path.push(&config.history_file_name);

        let mut manager = Self {
            items: HashMap::new(),
            order: VecDeque::new(),
            path,
            last_mtime: None,
            config,
        };
        manager.refresh();
        manager
    }

    /// Generates a stable ID for clipboard content
    pub fn calculate_id(content: &ClipboardContent) -> u64 {
        let mut s = DefaultHasher::new();
        match content {
            ClipboardContent::Text(t) => {
                0u8.hash(&mut s);
                t.hash(&mut s);
            }
            ClipboardContent::Image(d) => {
                1u8.hash(&mut s);
                d.hash(&mut s);
            }
        }
        s.finish()
    }

    /// Reloads items from disk if the file has changed. Returns true if reloaded.
    pub fn refresh(&mut self) -> bool {
        let metadata = std::fs::metadata(&self.path).ok();
        let mtime = metadata.and_then(|m| m.modified().ok());

        if mtime != self.last_mtime {
            self.load_from_disk();
            self.last_mtime = mtime;
            return true;
        }
        false
    }

    /// Adds or moves an item to the top of the history
    pub fn add_with_sensitivity(&mut self, content: ClipboardContent, is_sensitive: bool) {
        // Size Limit Check
        let size = match &content {
            ClipboardContent::Text(t) => t.len(),
            ClipboardContent::Image(d) => d.len(),
        };
        if size > self.config.history_max_item_size {
            return;
        }

        let id = Self::calculate_id(&content);
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        // 1. Synchronized Update
        match self.items.entry(id) {
            std::collections::hash_map::Entry::Occupied(mut entry) => {
                // Move existing to front
                if let Some(pos) = self.order.iter().position(|&x| x == id) {
                    self.order.remove(pos);
                }
                let item = entry.get_mut();
                item.timestamp = timestamp;
                if is_sensitive {
                    item.is_sensitive = true;
                }
            }
            std::collections::hash_map::Entry::Vacant(entry) => {
                // Insert new
                entry.insert(HistoryItem {
                    id,
                    content,
                    timestamp,
                    is_pinned: false,
                    is_sensitive,
                });
            }
        }
        self.order.push_front(id);

        // 2. Prune
        self.prune();

        // 3. Persist
        self.save_to_disk();
        self.last_mtime = std::fs::metadata(&self.path)
            .ok()
            .and_then(|m| m.modified().ok());
    }

    /// Toggles the pinned state of an item
    pub fn toggle_pin(&mut self, id: u64) {
        if let Some(item) = self.items.get_mut(&id) {
            item.is_pinned = !item.is_pinned;
            self.save_to_disk();
        }
    }

    /// Deletes an item from history
    pub fn delete(&mut self, id: u64) {
        if self.items.remove(&id).is_some() {
            if let Some(pos) = self.order.iter().position(|&x| x == id) {
                self.order.remove(pos);
            }
            self.save_to_disk();
            self.last_mtime = std::fs::metadata(&self.path)
                .ok()
                .and_then(|m| m.modified().ok());
        }
    }

    /// Moves an existing item to the top of the history
    pub fn move_to_top(&mut self, id: u64) {
        if self.items.contains_key(&id) {
            if let Some(pos) = self.order.iter().position(|&x| x == id) {
                self.order.remove(pos);
            }
            let timestamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            if let Some(item) = self.items.get_mut(&id) {
                item.timestamp = timestamp;
            }
            self.order.push_front(id);
            self.save_to_disk();
            self.last_mtime = std::fs::metadata(&self.path)
                .ok()
                .and_then(|m| m.modified().ok());
        }
    }

    fn prune(&mut self) {
        // Don't prune pinned items
        let mut i = 0;
        let mut count = 0;
        while i < self.order.len() {
            let id = self.order[i];
            let is_pinned = self
                .items
                .get(&id)
                .map(|item| item.is_pinned)
                .unwrap_or(false);
            if !is_pinned {
                count += 1;
                if count > self.config.history_max_items {
                    self.items.remove(&id);
                    self.order.remove(i);
                    continue;
                }
            }
            i += 1;
        }
    }

    /// Returns a list of items filtered by a query, in chronological order, pinned items first
    pub fn get_filtered(&self, query: &str) -> Vec<&HistoryItem> {
        let mut matches: Vec<(&HistoryItem, i32)> = Vec::new();

        for id in &self.order {
            if let Some(item) = self.items.get(id) {
                let score = match &item.content {
                    ClipboardContent::Text(t) => crate::ui::score_fuzzy(query, t),
                    ClipboardContent::Image(_) => {
                        if query.is_empty() {
                            1
                        } else {
                            crate::ui::score_fuzzy(query, "[image]")
                        }
                    }
                };

                if score > 0 {
                    matches.push((item, score));
                }
            }
        }

        if query.is_empty() {
            // Default order: Pinned first, then chronological
            let mut pinned = Vec::new();
            let mut others = Vec::new();
            for (item, _) in matches {
                if item.is_pinned {
                    pinned.push(item);
                } else {
                    others.push(item);
                }
            }
            pinned.extend(others);
            pinned
        } else {
            // Sort by score (descending), then pinned, then chronological
            matches.sort_by(|a, b| {
                b.1.cmp(&a.1) // Higher score first
                    .then_with(|| b.0.is_pinned.cmp(&a.0.is_pinned))
                    .then_with(|| b.0.timestamp.cmp(&a.0.timestamp))
            });
            matches.into_iter().map(|(item, _)| item).collect()
        }
    }

    pub fn items(&self) -> HashMap<u64, HistoryItem> {
        self.items.clone()
    }

    fn save_to_disk(&self) {
        let file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&self.path);

        if let Ok(file) = file {
            let mut writer = BufWriter::new(file);
            for id in &self.order {
                if let Some(item) = self.items.get(id) {
                    let (type_byte, data) = match &item.content {
                        ClipboardContent::Text(t) => (0u8, t.as_bytes()),
                        ClipboardContent::Image(d) => (1u8, d.as_slice()),
                    };

                    writer.write_all(&[type_byte]).ok();
                    writer.write_all(&item.timestamp.to_le_bytes()).ok();
                    writer.write_all(&[if item.is_pinned { 1 } else { 0 }]).ok();
                    writer
                        .write_all(&[if item.is_sensitive { 1 } else { 0 }])
                        .ok();
                    writer.write_all(&(data.len() as u32).to_le_bytes()).ok();
                    writer.write_all(data).ok();
                }
            }
        }
    }

    fn load_from_disk(&mut self) {
        let file = File::open(&self.path);
        self.items.clear();
        self.order.clear();

        if let Ok(file) = file {
            let mut reader = BufReader::new(file);
            let mut buffer = Vec::new();
            if reader.read_to_end(&mut buffer).is_err() {
                return;
            }

            let mut cursor = 0;
            while cursor < buffer.len() {
                // Header size: 1 byte type + 8 bytes timestamp + 1 byte pin + 1 byte sensitive + 4 bytes length = 15 bytes
                if cursor + 15 > buffer.len() {
                    break;
                }

                let type_byte = buffer[cursor];
                cursor += 1;

                let mut time_bytes = [0u8; 8];
                time_bytes.copy_from_slice(&buffer[cursor..cursor + 8]);
                let timestamp = u64::from_le_bytes(time_bytes);
                cursor += 8;

                let is_pinned = buffer[cursor] == 1;
                cursor += 1;

                let is_sensitive = buffer[cursor] == 1;
                cursor += 1;

                let mut len_bytes = [0u8; 4];
                len_bytes.copy_from_slice(&buffer[cursor..cursor + 4]);
                let data_len = u32::from_le_bytes(len_bytes) as usize;
                cursor += 4;

                if cursor + data_len > buffer.len() {
                    break;
                }
                let data = &buffer[cursor..cursor + data_len];
                cursor += data_len;

                let content = match type_byte {
                    0 => ClipboardContent::Text(String::from_utf8_lossy(data).to_string()),
                    1 => ClipboardContent::Image(data.to_vec()),
                    _ => continue,
                };

                let id = Self::calculate_id(&content);
                self.items.insert(
                    id,
                    HistoryItem {
                        id,
                        content,
                        timestamp,
                        is_pinned,
                        is_sensitive,
                    },
                );
                self.order.push_back(id);
            }
        }
    }
}

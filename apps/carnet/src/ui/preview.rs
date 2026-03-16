use crate::clipboard::ClipboardContent;
use crate::config::Tool;
use std::collections::HashMap;
use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;

#[derive(Clone, Debug)]
pub enum PreviewResult {
    Loading,
    Success(ClipboardContent),
    Error(String),
}

pub struct PreviewManager {
    cache: HashMap<String, PreviewResult>,
    active_previews: HashMap<String, ()>, // Just to track what's running
    tx: mpsc::Sender<(String, PreviewResult)>,
    rx: mpsc::Receiver<(String, PreviewResult)>,
}

impl PreviewManager {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            cache: HashMap::new(),
            active_previews: HashMap::new(),
            tx,
            rx,
        }
    }

    pub fn poll(&mut self) -> bool {
        let mut changed = false;
        while let Ok((key, result)) = self.rx.try_recv() {
            self.cache.insert(key.clone(), result);
            self.active_previews.remove(&key);
            changed = true;
        }
        changed
    }

    pub fn get_preview(&mut self, tool: &Tool, input: &ClipboardContent) -> PreviewResult {
        let key = format!("{}:{}", tool.name, self.calculate_input_hash(input));

        if let Some(result) = self.cache.get(&key) {
            return result.clone();
        }

        // Start background execution if not already running
        if !self.active_previews.contains_key(&key) {
            let tool = tool.clone();
            let input = input.clone();
            let tx = self.tx.clone();
            let key_clone = key.clone();

            thread::spawn(move || {
                let content_type = match &input {
                    ClipboardContent::Text(_) => "text",
                    ClipboardContent::Image(_) => "image",
                };

                let child = Command::new("sh")
                    .arg("-c")
                    .arg(&tool.bin)
                    .stdin(Stdio::piped())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .spawn();

                match child {
                    Ok(mut child) => {
                        if let Some(mut stdin) = child.stdin.take() {
                            match &input {
                                ClipboardContent::Text(t) => {
                                    let _ = stdin.write_all(t.as_bytes());
                                }
                                ClipboardContent::Image(d) => {
                                    let _ = stdin.write_all(d);
                                }
                            }
                        }

                        let output = child.wait_with_output();
                        let result = match output {
                            Ok(out) if out.status.success() => {
                                let content = if content_type == "text" {
                                    ClipboardContent::Text(
                                        String::from_utf8_lossy(&out.stdout).to_string(),
                                    )
                                } else {
                                    ClipboardContent::Image(out.stdout)
                                };
                                PreviewResult::Success(content)
                            }
                            Ok(out) => PreviewResult::Error(
                                String::from_utf8_lossy(&out.stderr).to_string(),
                            ),
                            Err(e) => PreviewResult::Error(e.to_string()),
                        };

                        let _ = tx.send((key_clone, result));
                    }
                    Err(e) => {
                        let _ = tx.send((key_clone, PreviewResult::Error(e.to_string())));
                    }
                }
            });
            self.active_previews.insert(key.clone(), ());
        }

        PreviewResult::Loading
    }

    fn calculate_input_hash(&self, input: &ClipboardContent) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        match input {
            ClipboardContent::Text(t) => t.hash(&mut hasher),
            ClipboardContent::Image(d) => d.hash(&mut hasher),
        }
        hasher.finish()
    }

    pub fn clear(&mut self) {
        self.cache.clear();
        self.active_previews.clear();
        // Clear pending messages in receiver
        while self.rx.try_recv().is_ok() {}
    }
}

use carnet::clipboard::{ClipboardContent, ClipboardManager};
use carnet::config::{Config, Tool};
use carnet::history::HistoryManager;
use carnet::ui::Terminal;
use carnet::ui::preview::{PreviewManager, PreviewResult};
use carnet::ui::renderer::Renderer;
use carnet::ui::{InputHandler, Key, Mode, fuzzy_match};
use std::io::{self, Write};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use term_uikit::widgets::{Input, ListState, ParagraphState};

fn main() -> std::io::Result<()> {
    let args: Vec<String> = std::env::args().collect();

    if args.len() > 1 && args[1] == "--version" {
        println!("carnet {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    if std::env::var("CARNET_SANDBOXED").is_err() {
        eprintln!("Error: carnet must be run through carnet-sandbox");
        std::process::exit(1);
    }
    let config = Config::load();
    let command = args.get(1).map(|s| s.as_str()).unwrap_or("show");

    match command {
        "store" => {
            let convert = args.iter().any(|arg| arg == "--convert");
            store_command(config, convert)
        }
        "show" => {
            let keep_open = args.iter().any(|arg| arg == "--keep-open" || arg == "-k");
            show_command(config, keep_open)
        }
        _ => {
            eprintln!("Usage: carnet [store|show] [--keep-open|-k] [--convert]");
            std::process::exit(1);
        }
    }
}

fn store_command(config: Config, convert: bool) -> io::Result<()> {
    let (content, is_converted) = if convert && config.auto_convert_image_uri {
        ClipboardManager::capture_with_uri_conversion()
    } else {
        (ClipboardManager::capture(), false)
    };

    if let Some(content) = content {
        let mut history = HistoryManager::new(config.clone());
        let is_sensitive = std::env::var("CLIPBOARD_STATE")
            .map(|s| s == "sensitive")
            .unwrap_or(false);
        history.add_with_sensitivity(content.clone(), is_sensitive);

        if convert && is_converted {
            let _ = ClipboardManager::copy_and_wait(&content, &config);
        }
    }
    Ok(())
}

struct App {
    config: Config,
    history: Arc<Mutex<HistoryManager>>,
    mode: Mode,
    search_query: String,
    selected_id: Option<u64>,
    selected_tool_index: usize,
    history_state: ListState,
    tool_state: ListState,
    preview_manager: PreviewManager,
    preview_state: ParagraphState,
    preview_animation_progress: Option<f32>,
    preview_focused: bool,
    last_selected_id: Option<u64>,
    last_selected_tool_index: usize,
    should_render: bool,
    should_quit: bool,
    keep_open: bool,
}

impl App {
    fn new(config: Config, keep_open: bool) -> Self {
        let history = Arc::new(Mutex::new(HistoryManager::new(config.clone())));
        let selected_id = ClipboardManager::capture().map(|c| HistoryManager::calculate_id(&c));

        // Ensure selected_id exists in history
        let mut initial_selected_id = selected_id;
        if let Some(id) = selected_id {
            let h = history.lock().unwrap();
            if !h.get_filtered("").iter().any(|item| item.id == id) {
                initial_selected_id = None;
            }
        }

        Self {
            config,
            history,
            mode: Mode::Normal,
            search_query: String::new(),
            selected_id: initial_selected_id,
            selected_tool_index: 0,
            history_state: ListState::new(),
            tool_state: ListState::new(),
            preview_manager: PreviewManager::new(),
            preview_state: ParagraphState::new(),
            preview_animation_progress: None,
            preview_focused: false,
            last_selected_id: initial_selected_id,
            last_selected_tool_index: 0,
            should_render: true,
            should_quit: false,
            keep_open,
        }
    }

    fn update(&mut self) {
        if self.preview_manager.poll() {
            self.should_render = true;
        }

        if let Some(progress) = self.preview_animation_progress {
            self.preview_animation_progress = Some(progress + 0.04);
            self.should_render = true;
        }

        {
            let mut h = self.history.lock().unwrap();
            if h.refresh() {
                self.should_render = true;
            }
        }

        if self.selected_id != self.last_selected_id {
            self.preview_state.reset();
            self.last_selected_id = self.selected_id;
            self.should_render = true;
        }

        if self.selected_tool_index != self.last_selected_tool_index {
            self.preview_state.reset();
            self.last_selected_tool_index = self.selected_tool_index;
            self.should_render = true;
        }
    }

    fn get_history_context(&self) -> (Vec<carnet::history::HistoryItem>, usize) {
        let history_lock = self.history.lock().unwrap();
        if matches!(self.mode, Mode::Tools) {
            // In tools mode, the history list is just the selected item
            let item = if let Some(id) = self.selected_id {
                history_lock.items().get(&id).cloned()
            } else {
                history_lock.get_filtered("").first().cloned().cloned()
            };
            let items = item.map(|i| vec![i]).unwrap_or_default();
            (items, 0)
        } else {
            let filtered_items = history_lock.get_filtered(&self.search_query);
            let items: Vec<carnet::history::HistoryItem> =
                filtered_items.iter().map(|&i| i.clone()).collect();
            let idx = self
                .selected_id
                .and_then(|id| items.iter().position(|i| i.id == id))
                .unwrap_or(0);
            (items, idx)
        }
    }

    fn get_tools_context(&self) -> (Vec<Tool>, usize) {
        let selected_item = self.get_selected_item();
        let content_type = match selected_item.as_ref().map(|i| &i.content) {
            Some(ClipboardContent::Text(_)) => "text",
            Some(ClipboardContent::Image(_)) => "image",
            None => "none",
        };

        let filtered_tools: Vec<Tool> = self
            .config
            .tools
            .iter()
            .filter(|tool| {
                let tool_ctx = tool.content_type.to_lowercase();
                let ctx_match = if content_type == "none" {
                    true
                } else {
                    tool_ctx == "both" || tool_ctx == content_type
                };
                ctx_match && fuzzy_match(&self.search_query, &tool.name)
            })
            .cloned()
            .collect();

        let idx = if filtered_tools.is_empty() {
            0
        } else {
            self.selected_tool_index
                .min(filtered_tools.len().saturating_sub(1))
        };

        (filtered_tools, idx)
    }

    fn get_selected_item(&self) -> Option<carnet::history::HistoryItem> {
        let history_lock = self.history.lock().unwrap();
        if let Some(id) = self.selected_id {
            history_lock.items().get(&id).cloned()
        } else {
            history_lock.get_filtered("").first().cloned().cloned()
        }
    }

    fn handle_key(&mut self, key: Key) {
        let (filtered_items, current_index) = self.get_history_context();

        match self.mode {
            Mode::Normal => match key {
                Key::Char('q') | Key::Escape => self.should_quit = true,
                Key::Char('/') => {
                    self.mode = Mode::Search;
                    self.should_render = true;
                }
                Key::Char('t') => {
                    self.mode = Mode::Tools;
                    self.search_query.clear();
                    self.selected_tool_index = 0;
                    self.should_render = true;
                }
                Key::Left => {
                    self.preview_focused = false;
                    self.should_render = true;
                }
                Key::Right => {
                    self.preview_focused = true;
                    self.should_render = true;
                }
                Key::Up | Key::Char('k') => {
                    if self.preview_focused {
                        self.preview_state.scroll_up(1);
                    } else {
                        self.history_state.scroll_up();
                        if let Some(item) = filtered_items.get(self.history_state.selected()) {
                            self.selected_id = Some(item.id);
                        }
                    }
                    self.should_render = true;
                }
                Key::Down | Key::Char('j') => {
                    if self.preview_focused {
                        self.preview_state.scroll_down(1);
                    } else {
                        self.history_state.scroll_down();
                        if let Some(item) = filtered_items.get(self.history_state.selected()) {
                            self.selected_id = Some(item.id);
                        }
                    }
                    self.should_render = true;
                }
                Key::PageUp => {
                    if self.preview_focused {
                        self.preview_state.scroll_page_up();
                    } else {
                        self.history_state.scroll_page_up();
                        if let Some(item) = filtered_items.get(self.history_state.selected()) {
                            self.selected_id = Some(item.id);
                        }
                    }
                    self.should_render = true;
                }
                Key::PageDown => {
                    if self.preview_focused {
                        self.preview_state.scroll_page_down();
                    } else {
                        self.history_state.scroll_page_down();
                        if let Some(item) = filtered_items.get(self.history_state.selected()) {
                            self.selected_id = Some(item.id);
                        }
                    }
                    self.should_render = true;
                }
                Key::Char('p') => {
                    if let Some(id) = self.selected_id {
                        let mut h_write = self.history.lock().unwrap();
                        h_write.toggle_pin(id);
                        self.should_render = true;
                    }
                }
                Key::Char('c') => {
                    ClipboardManager::clear().ok();
                    self.should_render = true;
                }
                Key::Backspace => {
                    if let Some(id) = self.selected_id {
                        if let Some(current_clipboard) = ClipboardManager::capture()
                            && HistoryManager::calculate_id(&current_clipboard) == id
                        {
                            let _ = ClipboardManager::clear();
                        }

                        let mut h_write = self.history.lock().unwrap();
                        h_write.delete(id);

                        let next_filtered = h_write.get_filtered(&self.search_query);
                        if !next_filtered.is_empty() {
                            // Sophisticated selection following: next available, or previous if none
                            let next_idx = if current_index < next_filtered.len() {
                                current_index
                            } else {
                                next_filtered.len().saturating_sub(1)
                            };
                            self.selected_id = Some(next_filtered[next_idx].id);
                        } else {
                            self.selected_id = None;
                        }
                        self.should_render = true;
                    }
                }
                Key::Enter => {
                    if let Some(item) = filtered_items.get(current_index) {
                        ClipboardManager::copy(&item.content, &self.config).ok();
                        let id_to_move = item.id;
                        let mut h_write = self.history.lock().unwrap();
                        h_write.move_to_top(id_to_move);

                        if !self.keep_open {
                            self.should_quit = true;
                        }
                        self.should_render = true;
                    }
                }
                _ => {}
            },
            Mode::Search => match key {
                Key::Escape => {
                    self.mode = Mode::Normal;
                    self.search_query.clear();
                    let h = self.history.lock().unwrap();
                    self.selected_id = h.get_filtered(&self.search_query).first().map(|i| i.id);
                    self.should_render = true;
                }
                Key::Up | Key::Char('k') => {
                    self.history_state.scroll_up();
                    if let Some(item) = filtered_items.get(self.history_state.selected()) {
                        self.selected_id = Some(item.id);
                    }
                    self.should_render = true;
                }
                Key::Down | Key::Char('j') => {
                    self.history_state.scroll_down();
                    if let Some(item) = filtered_items.get(self.history_state.selected()) {
                        self.selected_id = Some(item.id);
                    }
                    self.should_render = true;
                }
                Key::PageUp => {
                    self.history_state.scroll_page_up();
                    if let Some(item) = filtered_items.get(self.history_state.selected()) {
                        self.selected_id = Some(item.id);
                    }
                    self.should_render = true;
                }
                Key::PageDown => {
                    self.history_state.scroll_page_down();
                    if let Some(item) = filtered_items.get(self.history_state.selected()) {
                        self.selected_id = Some(item.id);
                    }
                    self.should_render = true;
                }
                Key::Enter => {
                    self.mode = Mode::Normal;
                    self.should_render = true;
                }
                _ => {
                    if Input::handle_event(&mut self.search_query, key) {
                        self.should_render = true;
                        let h = self.history.lock().unwrap();
                        self.selected_id = h.get_filtered(&self.search_query).first().map(|i| i.id);
                    }
                }
            },
            Mode::Tools => match key {
                Key::Escape => {
                    self.mode = Mode::Normal;
                    self.search_query.clear();
                    self.selected_id = self.last_selected_id;
                    self.preview_manager.clear();
                    self.preview_focused = false;
                    self.preview_state.reset();
                    self.should_render = true;
                }
                Key::Left => {
                    self.preview_focused = false;
                    self.should_render = true;
                }
                Key::Right => {
                    self.preview_focused = true;
                    self.should_render = true;
                }
                Key::PageUp => {
                    if self.preview_focused {
                        self.preview_state.scroll_page_up();
                    } else {
                        self.tool_state.scroll_page_up();
                        self.selected_tool_index = self.tool_state.selected();
                    }
                    self.should_render = true;
                }
                Key::PageDown => {
                    if self.preview_focused {
                        self.preview_state.scroll_page_down();
                    } else {
                        self.tool_state.scroll_page_down();
                        self.selected_tool_index = self.tool_state.selected();
                    }
                    self.should_render = true;
                }
                Key::Up | Key::Char('k') => {
                    if self.preview_focused {
                        self.preview_state.scroll_up(1);
                    } else {
                        self.tool_state.scroll_up();
                        self.selected_tool_index = self.tool_state.selected();
                    }
                    self.should_render = true;
                }
                Key::Down | Key::Char('j') => {
                    if self.preview_focused {
                        self.preview_state.scroll_down(1);
                    } else {
                        self.tool_state.scroll_down();
                        self.selected_tool_index = self.tool_state.selected();
                    }
                    self.should_render = true;
                }
                Key::Enter => {
                    let (filtered_tools, tool_idx) = self.get_tools_context();
                    if let Some(item) = self.get_selected_item()
                        && !filtered_tools.is_empty()
                    {
                        let tool = &filtered_tools[tool_idx];
                        self.execute_tool(tool, &item);
                    }
                    self.mode = Mode::Normal;
                    self.search_query.clear();
                    self.selected_id = None;
                    self.preview_manager.clear();
                    self.should_render = true;
                }
                _ => {
                    if Input::handle_event(&mut self.search_query, key) {
                        self.selected_tool_index = 0;
                        self.should_render = true;
                    }
                }
            },
        }
    }

    fn execute_tool(&mut self, tool: &Tool, item: &carnet::history::HistoryItem) {
        let content_type = match item.content {
            ClipboardContent::Text(_) => "text",
            ClipboardContent::Image(_) => "image",
        };

        let mut cached_result = None;
        if tool.preview
            && let PreviewResult::Success(content) =
                self.preview_manager.get_preview(tool, &item.content)
        {
            cached_result = Some(content);
        }

        if let Some(content) = cached_result {
            ClipboardManager::copy(&content, &self.config).ok();
        } else {
            let child = Command::new("sh")
                .arg("-c")
                .arg(&tool.bin)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .spawn()
                .ok();

            if let Some(mut child) = child {
                if let Some(mut stdin) = child.stdin.take() {
                    match &item.content {
                        ClipboardContent::Text(t) => {
                            stdin.write_all(t.as_bytes()).ok();
                        }
                        ClipboardContent::Image(d) => {
                            stdin.write_all(d).ok();
                        }
                    }
                }
                let output = child.wait_with_output().ok();
                if let Some(output) = output
                    && output.status.success()
                    && !output.stdout.is_empty()
                {
                    let new_content = if content_type == "text" {
                        ClipboardContent::Text(String::from_utf8_lossy(&output.stdout).to_string())
                    } else {
                        ClipboardContent::Image(output.stdout)
                    };
                    ClipboardManager::copy(&new_content, &self.config).ok();
                }
            }
        }
    }

    fn get_preview_result(&mut self) -> Option<PreviewResult> {
        if !matches!(self.mode, Mode::Tools) {
            self.preview_animation_progress = None;
            return None;
        }

        let (filtered_tools, tool_idx) = self.get_tools_context();
        if filtered_tools.is_empty() {
            self.preview_animation_progress = None;
            return None;
        }

        let tool = &filtered_tools[tool_idx];
        if !tool.preview {
            self.preview_animation_progress = None;
            return None;
        }

        let selected_item = self.get_selected_item()?;
        let res = self
            .preview_manager
            .get_preview(tool, &selected_item.content);

        // Handle animation progress
        if matches!(res, PreviewResult::Loading) && self.preview_animation_progress.is_none() {
            self.preview_animation_progress = Some(0.0);
        }
        if let Some(progress) = self.preview_animation_progress
            && progress >= 1.0
            && !matches!(res, PreviewResult::Loading)
        {
            self.preview_animation_progress = None;
        }

        Some(res)
    }

    fn render(&mut self, terminal: &mut Terminal) -> io::Result<()> {
        if !self.should_render {
            return Ok(());
        }

        let (filtered_items, history_idx) = self.get_history_context();
        let (filtered_tools, tool_idx) = self.get_tools_context();

        let preview_result = self.get_preview_result();
        let active_id = ClipboardManager::capture().map(|c| HistoryManager::calculate_id(&c));

        Renderer::render(
            terminal,
            &self.mode,
            &self.search_query,
            &filtered_items,
            &filtered_tools,
            history_idx,
            tool_idx,
            active_id,
            &mut self.history_state,
            &mut self.tool_state,
            &self.config,
            preview_result,
            self.preview_animation_progress,
            self.preview_focused,
            &mut self.preview_state,
        )?;

        self.should_render = false;
        Ok(())
    }
}

fn show_command(config: Config, keep_open: bool) -> io::Result<()> {
    let mut terminal = Terminal::new()?;
    let mut app = App::new(config, keep_open);

    let (tx, rx) = mpsc::channel::<Key>();
    thread::spawn(move || {
        loop {
            let key = InputHandler::read_key();
            if tx.send(key).is_err() {
                break;
            }
        }
    });

    let mut last_size = terminal.size();

    loop {
        app.update();

        let current_size = terminal.size();
        if current_size != last_size {
            last_size = current_size;
            app.should_render = true;
        }

        app.render(&mut terminal)?;

        if app.should_quit {
            break;
        }

        while let Ok(key) = rx.try_recv() {
            app.handle_key(key);
        }

        thread::sleep(std::time::Duration::from_millis(10));
    }
    Ok(())
}

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
use std::time::Duration;
use term_uikit::widgets::{Input, ListState, ParagraphState};

fn main() -> std::io::Result<()> {
    if std::env::var("CARNET_SANDBOXED").is_err() {
        eprintln!("Error: carnet must be run through carnet-sandbox");
        std::process::exit(1);
    }
    let config = Config::load();
    let args: Vec<String> = std::env::args().collect();
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

fn show_command(config: Config, keep_open: bool) -> io::Result<()> {
    let history = Arc::new(Mutex::new(HistoryManager::new(config.clone())));
    let mut terminal = Terminal::new()?;

    // Initial selected_id: current clipboard content
    let mut selected_id: Option<u64> =
        ClipboardManager::capture().map(|c| HistoryManager::calculate_id(&c));

    // Ensure selected_id exists in history, otherwise default to None (will be first item)
    if let Some(id) = selected_id {
        let h = history.lock().unwrap();
        if !h.get_filtered("").iter().any(|item| item.id == id) {
            selected_id = None;
        }
    }

    let mut mode = Mode::Normal;
    let mut search_query = String::new();
    let mut selected_tool_index: usize = 0;

    let mut history_state = ListState::new();
    let mut tool_state = ListState::new();

    let mut last_item_id = selected_id;

    // Channel for non-blocking-ish input
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
    let mut should_render = true;
    let mut preview_manager = PreviewManager::new();
    let mut preview_animation_progress: Option<f32> = None;
    let mut preview_focused = false;
    let mut preview_state = ParagraphState::new();
    let mut last_selected_id = selected_id;
    let mut last_selected_tool_index = selected_tool_index;

    loop {
        if preview_manager.poll() {
            should_render = true;
        }

        if let Some(progress) = preview_animation_progress {
            preview_animation_progress = Some(progress + 0.04);
            should_render = true;
        }

        {
            let mut h = history.lock().unwrap();
            if h.refresh() {
                should_render = true;
            }
        }

        let current_size = terminal.size();
        if current_size != last_size {
            last_size = current_size;
            should_render = true;
        }

        if selected_id != last_selected_id {
            preview_state.reset();
            last_selected_id = selected_id;
            should_render = true;
        }

        if selected_tool_index != last_selected_tool_index {
            preview_state.reset();
            last_selected_tool_index = selected_tool_index;
            should_render = true;
        }

        // Render UI ONLY if something changed
        if should_render {
            // Sync ListState before rendering
            let filtered_ids: Vec<u64> = {
                let h = history.lock().unwrap();
                let query = if matches!(mode, Mode::Tools) {
                    ""
                } else {
                    &search_query
                };
                h.get_filtered(query).iter().map(|i| i.id).collect()
            };

            let current_idx = selected_id
                .and_then(|id| filtered_ids.iter().position(|&x| x == id))
                .unwrap_or(0);
            history_state.select(current_idx);
            tool_state.select(selected_tool_index);

            let preview_result = if matches!(mode, Mode::Tools) {
                let selected_item = get_selected_item(&history, last_item_id);

                if let Some(item) = selected_item {
                    let content_type = match item.content {
                        ClipboardContent::Text(_) => "text",
                        ClipboardContent::Image(_) => "image",
                    };

                    let filtered_tools = get_filtered_tools(&config, content_type, &search_query);

                    if !filtered_tools.is_empty() {
                        let tool_idx = selected_tool_index.min(filtered_tools.len() - 1);
                        let tool = filtered_tools[tool_idx];
                        if tool.preview {
                            let res = preview_manager.get_preview(tool, &item.content);

                            // Start animation if loading
                            if matches!(res, PreviewResult::Loading)
                                && preview_animation_progress.is_none()
                            {
                                preview_animation_progress = Some(0.0);
                            }

                            // Stop animation only if we reached the endof a tturn AND result is not loading
                            if let Some(progress) = preview_animation_progress {
                                if progress >= 1.0 && !matches!(res, PreviewResult::Loading) {
                                    preview_animation_progress = None;
                                }
                            }

                            Some(res)
                        } else {
                            preview_animation_progress = None;
                            None
                        }
                    } else {
                        preview_animation_progress = None;
                        None
                    }
                } else {
                    preview_animation_progress = None;
                    None
                }
            } else {
                preview_animation_progress = None;
                None
            };

            Renderer::render(
                &mut terminal,
                &history,
                &mode,
                &search_query,
                selected_id,
                ClipboardManager::capture().map(|c| HistoryManager::calculate_id(&c)),
                &mut history_state,
                &mut tool_state,
                &config,
                preview_result,
                preview_animation_progress.map(|p| p % 1.0),
                preview_focused,
                &mut preview_state,
            )?;
            should_render = false;
        }

        // Wait for input
        let timeout = if preview_animation_progress.is_some() {
            Duration::from_millis(10)
        } else {
            Duration::from_millis(config.refresh_rate_ms)
        };

        if let Ok(key) = rx.recv_timeout(timeout) {
            should_render = true;

            let history_query = if matches!(mode, Mode::Tools) {
                ""
            } else {
                &search_query
            };
            let filtered_ids_and_content: Vec<(u64, ClipboardContent)> = {
                let h = history.lock().unwrap();
                h.get_filtered(history_query)
                    .iter()
                    .map(|&i| (i.id, i.content.clone()))
                    .collect()
            };

            // Auto-select first item if current selection is invalid
            if selected_id.is_none()
                && !filtered_ids_and_content.is_empty()
                && !matches!(mode, Mode::Tools)
            {
                selected_id = Some(filtered_ids_and_content[0].0);
            } else if let Some(id) = selected_id {
                // If we have a selection but it's not in the filtered list anymore, jump to first match
                if !filtered_ids_and_content.iter().any(|(iid, _)| *iid == id)
                    && !filtered_ids_and_content.is_empty()
                {
                    selected_id = Some(filtered_ids_and_content[0].0);
                }
            }

            let current_index = selected_id
                .and_then(|id| {
                    filtered_ids_and_content
                        .iter()
                        .position(|(iid, _)| *iid == id)
                })
                .unwrap_or(0);

            match mode {
                Mode::Normal => match key {
                    Key::Char('q') | Key::Escape => break,
                    Key::Left => {
                        preview_focused = false;
                    }
                    Key::Right => {
                        preview_focused = true;
                    }
                    Key::Char('/') => {
                        mode = Mode::Search;
                    }
                    Key::Char('t') => {
                        mode = Mode::Tools;
                        search_query.clear();
                        selected_tool_index = 0;
                        last_item_id = selected_id;
                        preview_manager.clear();
                        preview_focused = false;
                    }
                    Key::Up | Key::Char('k') => {
                        if preview_focused {
                            preview_state.scroll_up(1);
                        } else if current_index > 0 {
                            let new_index = current_index - 1;
                            selected_id = Some(filtered_ids_and_content[new_index].0);
                        }
                    }
                    Key::Down | Key::Char('j') => {
                        if preview_focused {
                            let visible_height = terminal.size().0.saturating_sub(4) as usize;
                            preview_state.scroll_down(1, visible_height);
                        } else if current_index < filtered_ids_and_content.len().saturating_sub(1) {
                            let new_index = current_index + 1;
                            selected_id = Some(filtered_ids_and_content[new_index].0);
                        }
                    }
                    Key::PageUp => {
                        if preview_focused {
                            let page_size = terminal.size().0.saturating_sub(4) as usize;
                            preview_state.scroll_up(page_size);
                        }
                    }
                    Key::PageDown => {
                        if preview_focused {
                            let page_size = terminal.size().0.saturating_sub(4) as usize;
                            preview_state.scroll_down(page_size, page_size);
                        }
                    }
                    Key::Char('p') => {
                        if let Some(id) = selected_id {
                            let mut h_write = history.lock().unwrap();
                            h_write.toggle_pin(id);
                        }
                    }
                    Key::Char('c') => {
                        ClipboardManager::clear().ok();
                    }
                    Key::Backspace => {
                        if let Some(id) = selected_id {
                            // Check if the item being deleted is currently in the clipboard
                            if let Some(current_clipboard) = ClipboardManager::capture()
                                && HistoryManager::calculate_id(&current_clipboard) == id
                            {
                                let _ = ClipboardManager::clear();
                            }

                            let mut h_write = history.lock().unwrap();
                            h_write.delete(id);

                            if filtered_ids_and_content.len() > 1 {
                                if current_index < filtered_ids_and_content.len() - 1 {
                                    selected_id =
                                        Some(filtered_ids_and_content[current_index + 1].0);
                                } else {
                                    selected_id =
                                        Some(filtered_ids_and_content[current_index - 1].0);
                                }
                            } else {
                                selected_id = None;
                            }
                        }
                    }
                    Key::Enter => {
                        if let Some((id, content)) = filtered_ids_and_content.get(current_index) {
                            ClipboardManager::copy(content, &config).ok();
                            let id_to_move = *id;

                            let mut h_write = history.lock().unwrap();
                            h_write.move_to_top(id_to_move);

                            if !keep_open {
                                break;
                            }
                        }
                    }
                    _ => {}
                },
                Mode::Search => match key {
                    Key::Escape => {
                        mode = Mode::Normal;
                        search_query.clear();
                        let h = history.lock().unwrap();
                        selected_id = h.get_filtered(&search_query).first().map(|i| i.id);
                    }
                    Key::Up | Key::Char('k') => {
                        if current_index > 0 {
                            let new_index = current_index - 1;
                            selected_id = Some(filtered_ids_and_content[new_index].0);
                        }
                    }
                    Key::Down | Key::Char('j') => {
                        if current_index < filtered_ids_and_content.len().saturating_sub(1) {
                            let new_index = current_index + 1;
                            selected_id = Some(filtered_ids_and_content[new_index].0);
                        }
                    }
                    Key::Enter => {
                        mode = Mode::Normal;
                    }
                    _ => {
                        Input::handle_event(&mut search_query, key);
                    }
                },
                Mode::Tools => match key {
                    Key::Escape => {
                        mode = Mode::Normal;
                        search_query.clear();
                        selected_id = last_item_id;
                        preview_manager.clear();
                        preview_focused = false;
                        preview_state.reset();
                    }
                    Key::Left => {
                        preview_focused = false;
                    }
                    Key::Right => {
                        preview_focused = true;
                    }
                    Key::PageUp => {
                        if preview_focused {
                            let page_size = terminal.size().0.saturating_sub(4) as usize;
                            preview_state.scroll_up(page_size);
                        }
                    }
                    Key::PageDown => {
                        if preview_focused {
                            let page_size = terminal.size().0.saturating_sub(4) as usize;
                            preview_state.scroll_down(page_size, page_size);
                        }
                    }
                    Key::Up | Key::Char('k') => {
                        if preview_focused {
                            preview_state.scroll_up(1);
                        } else {
                            selected_tool_index = selected_tool_index.saturating_sub(1);
                        }
                    }
                    Key::Down | Key::Char('j') => {
                        if preview_focused {
                            let visible_height = terminal.size().0.saturating_sub(4) as usize;
                            preview_state.scroll_down(1, visible_height);
                        } else {
                            let selected_item = get_selected_item(&history, last_item_id);
                            if let Some(item) = selected_item {
                                let content_type = match item.content {
                                    ClipboardContent::Text(_) => "text",
                                    ClipboardContent::Image(_) => "image",
                                };
                                let filtered_tools =
                                    get_filtered_tools(&config, content_type, &search_query);
                                if selected_tool_index < filtered_tools.len().saturating_sub(1) {
                                    selected_tool_index += 1;
                                }
                            }
                        }
                    }
                    Key::Enter => {
                        // Execute tool
                        let selected_item = get_selected_item(&history, last_item_id);

                        if let Some(item) = selected_item {
                            let content_type = match item.content {
                                ClipboardContent::Text(_) => "text",
                                ClipboardContent::Image(_) => "image",
                            };

                            let filtered_tools =
                                get_filtered_tools(&config, content_type, &search_query);

                            if !filtered_tools.is_empty() {
                                let tool_idx = selected_tool_index.min(filtered_tools.len() - 1);
                                let tool = filtered_tools[tool_idx];

                                // Check if we have a cached result
                                let mut cached_result = None;
                                if tool.preview {
                                    if let PreviewResult::Success(content) =
                                        preview_manager.get_preview(tool, &item.content)
                                    {
                                        cached_result = Some(content);
                                    }
                                }

                                if let Some(content) = cached_result {
                                    ClipboardManager::copy(&content, &config).ok();
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
                                                ClipboardContent::Text(
                                                    String::from_utf8_lossy(&output.stdout)
                                                        .to_string(),
                                                )
                                            } else {
                                                ClipboardContent::Image(output.stdout)
                                            };

                                            ClipboardManager::copy(&new_content, &config).ok();
                                        }
                                    }
                                }
                            }
                        }
                        mode = Mode::Normal;
                        search_query.clear();
                        selected_id = None; // Reset to top
                        preview_manager.clear();
                    }
                    _ => {
                        if Input::handle_event(&mut search_query, key) {
                            selected_tool_index = 0;
                        }
                    }
                },
            }
        }
    }
    Ok(())
}

fn get_selected_item(
    history: &Arc<Mutex<HistoryManager>>,
    last_item_id: Option<u64>,
) -> Option<carnet::history::HistoryItem> {
    let h = history.lock().unwrap();
    let filtered = h.get_filtered("");
    if let Some(id) = last_item_id {
        filtered.iter().find(|i| i.id == id).map(|&i| i.clone())
    } else {
        filtered.first().map(|&i| i.clone())
    }
}

fn get_filtered_tools<'a>(
    config: &'a Config,
    content_type: &str,
    search_query: &str,
) -> Vec<&'a Tool> {
    config
        .tools
        .iter()
        .filter(|tool| {
            let tool_ctx = tool.content_type.to_lowercase();
            (tool_ctx == "both" || tool_ctx == content_type)
                && fuzzy_match(search_query, &tool.name)
        })
        .collect()
}

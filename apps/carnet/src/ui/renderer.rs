use crate::clipboard::ClipboardContent;
use crate::config::{Config, Tool};
use crate::history::HistoryItem;
use crate::ui::Mode;
use crate::ui::preview::PreviewResult;
use std::io;
use std::time::SystemTime;
use term_uikit::Terminal;
use term_uikit::image_proc::ImageProcessor;
use term_uikit::layout::Rect;
use term_uikit::widgets::{
    ActionBar, Card, EmptyView, Flex, ImageView, Input, List, ListItem, ListState, Paragraph,
    ParagraphState, Sizing, TextView, View,
};

pub struct Renderer;

impl Renderer {
    #[allow(clippy::too_many_arguments)]
    pub fn render(
        terminal: &mut Terminal,
        mode: &Mode,
        search_query: &str,
        filtered_items: &[HistoryItem],
        filtered_tools: &[Tool],
        selected_index: usize,
        tool_index: usize,
        active_id: Option<u64>,
        history_state: &mut ListState,
        tool_state: &mut ListState,
        config: &Config,
        preview_result: Option<PreviewResult>,
        loading_progress: Option<f32>,
        preview_focused: bool,
        preview_state: &mut ParagraphState,
    ) -> io::Result<()> {
        // --- DATA GATHERING ---
        let (rows, cols) = terminal.size();
        terminal.resize(cols, rows);
        let area = Rect::new(0, 0, cols, rows);

        // Palette for bottom bar
        let palette = match mode {
            Mode::Normal => vec![
                ("q/Esc", "Quit"),
                ("/", "Search"),
                ("t", "Tools"),
                ("k/↑", "Up"),
                ("j/↓", "Down"),
                ("p", "Pin"),
                ("c", "Clear"),
                ("Bksp", "Delete"),
                ("Enter", "Copy"),
            ],
            Mode::Search => vec![("Esc", "Cancel"), ("Enter", "Finish"), ("Backsp", "Delete")],
            Mode::Tools => vec![
                ("Esc", "Cancel"),
                ("Enter", "Run Tool"),
                ("Backsp", "Delete"),
            ],
        };

        // --- 2. VIEW TREE CONSTRUCTION ---
        let primary_color = format!("1;{}", config.ui_color_primary);

        // Search Input
        let search_active = matches!(mode, Mode::Search) || matches!(mode, Mode::Tools);
        let prompt_title = if matches!(mode, Mode::Tools) {
            " Tools "
        } else {
            " Search "
        };
        let prompt_prefix = if matches!(mode, Mode::Tools) {
            "Tool: "
        } else {
            &format!("{} ", config.ui_icon_prompt)
        };
        let placeholder = if matches!(mode, Mode::Normal) && search_query.is_empty() {
            "Type / to search, Enter to copy..."
        } else {
            ""
        };

        let search_input = Input::new()
            .with_value(search_query)
            .with_prefix(prompt_prefix)
            .with_placeholder(placeholder)
            .active(search_active)
            .with_colors(&primary_color, &config.ui_color_dim, "1;37");

        let search_card = Card::new()
            .with_title(prompt_title)
            .active(search_active)
            .with_colors(&primary_color, &config.ui_color_dim, "1;37")
            .with_border_chars(&config.ui_border_chars)
            .with_padding(1, 0)
            .content(search_input);

        // List Content
        let mut list_items = Vec::new();
        if matches!(mode, Mode::Tools) {
            for tool in filtered_tools.iter() {
                list_items.push(ListItem::new(format!("❯ {}", tool.name)));
            }
        } else {
            for item in filtered_items.iter() {
                let time_or_pin = if item.is_pinned {
                    format!("{} ", config.ui_icon_pin)
                } else {
                    let diff = SystemTime::now()
                        .duration_since(SystemTime::UNIX_EPOCH)
                        .unwrap()
                        .as_secs()
                        .saturating_sub(item.timestamp);
                    if diff < 60 {
                        format!("{:>4}s ", diff)
                    } else if diff < 3600 {
                        format!("{:>4}m ", diff / 60)
                    } else {
                        format!("{:>4}h ", diff / 3600)
                    }
                };

                let mut li;
                match &item.content {
                    ClipboardContent::Text(text) => {
                        let icon = if item.is_sensitive {
                            &config.ui_icon_sensitive
                        } else {
                            &config.ui_icon_text
                        };
                        let preview: String = if item.is_sensitive {
                            "*".repeat(10)
                        } else {
                            text.chars().filter(|c| !c.is_control()).collect()
                        };
                        li = ListItem::new(preview).with_icon(icon);
                    }
                    ClipboardContent::Image(data) => {
                        let icon = if item.is_sensitive {
                            &config.ui_icon_sensitive
                        } else {
                            &config.ui_icon_image
                        };
                        let info = if item.is_sensitive {
                            " [SENSITIVE IMAGE] ".to_string()
                        } else {
                            let mime = term_uikit::image_format::detect_mime(data);
                            if let Some((w, h)) = ImageProcessor::get_image_info(data, mime) {
                                format!("{} {}x{}", mime.trim_start_matches("image/"), w, h)
                            } else {
                                "image [Binary]".to_string()
                            }
                        };
                        li = ListItem::new(info).with_icon(icon);
                    }
                }
                li = li.with_right_text(time_or_pin);
                if item.is_pinned {
                    li = li.highlight(true);
                } else {
                    li = li.dimmed(true);
                }
                if let Some(aid) = active_id
                    && item.id == aid
                {
                    li = li.active(true);
                }
                list_items.push(li);
            }
        }

        let list_state = if matches!(mode, Mode::Tools) {
            tool_state.select(tool_index);
            tool_state
        } else {
            history_state.select(selected_index);
            history_state
        };
        let history_list = List::new(list_items, list_state)
            .with_colors(
                &config.ui_color_highlight,
                &config.ui_color_dim,
                "1;37",
                &primary_color,
            )
            .with_active_icon(&config.ui_icon_prompt);

        let list_title = if matches!(mode, Mode::Tools) {
            " Select Tool "
        } else {
            &format!(" History ({} items) ", filtered_items.len())
        };
        let list_card = Card::new()
            .with_title(list_title)
            .active(
                !preview_focused && (matches!(mode, Mode::Normal) || matches!(mode, Mode::Tools)),
            )
            .with_colors(&primary_color, &config.ui_color_dim, "1;37")
            .with_border_chars(&config.ui_border_chars)
            .content(history_list);

        // Preview Content
        let preview_view: Box<dyn View> = if let Some(result) = &preview_result {
            match result {
                PreviewResult::Loading => Box::new(EmptyView),
                PreviewResult::Success(content) => match content {
                    ClipboardContent::Text(text) => Box::new(
                        Paragraph::new(text.clone())
                            .show_newlines(config.ui_show_newlines)
                            .with_state(preview_state),
                    ),
                    ClipboardContent::Image(data) => Box::new(ImageView::new(data)),
                },
                PreviewResult::Error(err) => Box::new(TextView::new(format!("Error: {}", err))),
            }
        } else if let Some(selected) = filtered_items.get(selected_index) {
            if selected.is_sensitive {
                Box::new(TextView::new(" [ CONTENT MASKED ] "))
            } else {
                match &selected.content {
                    ClipboardContent::Text(text) => Box::new(
                        Paragraph::new(text.clone())
                            .show_newlines(config.ui_show_newlines)
                            .with_state(preview_state),
                    ),
                    ClipboardContent::Image(data) => Box::new(ImageView::new(data)),
                }
            }
        } else {
            Box::new(EmptyView)
        };

        let preview_card = Card::new()
            .with_title(" Preview ")
            .active(preview_focused)
            .loading_progress(loading_progress)
            .with_colors(&primary_color, &config.ui_color_dim, "1;37")
            .with_border_chars(&config.ui_border_chars)
            .content(preview_view);

        // Action Bar
        let action_bar =
            ActionBar::new(palette).with_colors(&primary_color, &config.ui_color_dim, "1;37");

        let actions_card = Card::new()
            .with_title(" Actions ")
            .active(false)
            .with_colors(&primary_color, &config.ui_color_dim, "1;37")
            .with_border_chars(&config.ui_border_chars)
            .with_padding(1, 0)
            .content(action_bar);

        // Assemble Final Root Tree
        let mut root = Flex::vertical()
            .add(search_card, Sizing::Intrinsic)
            .add(
                Flex::horizontal()
                    .add(list_card, Sizing::Fill(1))
                    .add(preview_card, Sizing::Fill(1)),
                Sizing::Fill(1),
            )
            .add(actions_card, Sizing::Intrinsic);

        // --- RENDERING ---
        terminal.clear()?;
        terminal.clear_images()?;
        root.render(area, terminal)?;
        terminal.flush()
    }
}

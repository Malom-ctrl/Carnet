use crate::event::Key;
use crate::layout::{Constraint, Direction, Layout, Rect};
use crate::terminal::Terminal;
use std::borrow::Cow;
use std::io;

pub trait View {
    fn measure(&self, width: Option<u16>, height: Option<u16>) -> (u16, u16);
    fn render(&mut self, area: Rect, terminal: &mut Terminal) -> io::Result<()>;
}

impl<'a> View for Box<dyn View + 'a> {
    fn measure(&self, width: Option<u16>, height: Option<u16>) -> (u16, u16) {
        (**self).measure(width, height)
    }
    fn render(&mut self, area: Rect, terminal: &mut Terminal) -> io::Result<()> {
        (**self).render(area, terminal)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Sizing {
    Fixed(u16),
    Fill(u32),
    Intrinsic,
}

impl From<Sizing> for Constraint {
    fn from(sizing: Sizing) -> Self {
        match sizing {
            Sizing::Fixed(l) => Constraint::Length(l),
            Sizing::Fill(w) => Constraint::Fill(w),
            Sizing::Intrinsic => Constraint::Intrinsic,
        }
    }
}

pub struct Flex<'a> {
    direction: Direction,
    children: Vec<(Box<dyn View + 'a>, Sizing)>,
}

impl<'a> Flex<'a> {
    pub fn new(direction: Direction) -> Self {
        Self {
            direction,
            children: Vec::new(),
        }
    }

    pub fn vertical() -> Self {
        Self::new(Direction::Vertical)
    }

    pub fn horizontal() -> Self {
        Self::new(Direction::Horizontal)
    }

    pub fn add<V: View + 'a>(mut self, view: V, sizing: Sizing) -> Self {
        self.children.push((Box::new(view), sizing));
        self
    }
}

impl<'a> View for Flex<'a> {
    fn measure(&self, width: Option<u16>, height: Option<u16>) -> (u16, u16) {
        let constraints: Vec<Constraint> = self.children.iter().map(|(_, s)| (*s).into()).collect();
        let layout = Layout::new()
            .direction(self.direction)
            .constraints(constraints);

        let area = Rect::new(0, 0, width.unwrap_or(0), height.unwrap_or(0));
        let measures: Vec<(u16, u16)> = self
            .children
            .iter()
            .map(|(v, _)| v.measure(width, height))
            .collect();

        let rects = layout.split_with_measures(area, &measures);

        let mut total_w = 0;
        let mut total_h = 0;

        for r in rects {
            match self.direction {
                Direction::Horizontal => {
                    total_w += r.width;
                    total_h = total_h.max(r.height);
                }
                Direction::Vertical => {
                    total_w = total_w.max(r.width);
                    total_h += r.height;
                }
            }
        }
        (total_w, total_h)
    }

    fn render(&mut self, area: Rect, terminal: &mut Terminal) -> io::Result<()> {
        let constraints: Vec<Constraint> = self.children.iter().map(|(_, s)| (*s).into()).collect();
        let layout = Layout::new()
            .direction(self.direction)
            .constraints(constraints);

        let measures: Vec<(u16, u16)> = self
            .children
            .iter()
            .map(|(v, _)| v.measure(Some(area.width), Some(area.height)))
            .collect();

        let rects = layout.split_with_measures(area, &measures);

        for (i, (child, _)) in self.children.iter_mut().enumerate() {
            if let Some(rect) = rects.get(i) {
                child.render(*rect, terminal)?;
            }
        }
        Ok(())
    }
}

pub struct Card<'a> {
    pub title: Option<String>,
    pub active: bool,
    pub primary_color: String,
    pub dim_color: String,
    pub text_color: String,
    pub border_chars: Option<String>,
    pub horizontal_padding: u16,
    pub vertical_padding: u16,
    pub content: Option<Box<dyn View + 'a>>,
}

impl<'a> Card<'a> {
    pub fn new() -> Self {
        Self {
            title: None,
            active: false,
            primary_color: "1;34".into(),
            dim_color: "2".into(),
            text_color: "1;37".into(),
            border_chars: None,
            horizontal_padding: 0,
            vertical_padding: 0,
            content: None,
        }
    }

    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn active(mut self, active: bool) -> Self {
        self.active = active;
        self
    }

    pub fn with_colors(mut self, primary: &str, dim: &str, text: &str) -> Self {
        self.primary_color = primary.into();
        self.dim_color = dim.into();
        self.text_color = text.into();
        self
    }

    pub fn with_border_chars(mut self, chars: &str) -> Self {
        self.border_chars = Some(chars.into());
        self
    }

    pub fn with_padding(mut self, horizontal: u16, vertical: u16) -> Self {
        self.horizontal_padding = horizontal;
        self.vertical_padding = vertical;
        self
    }

    pub fn content<V: View + 'a>(mut self, view: V) -> Self {
        self.content = Some(Box::new(view));
        self
    }

    fn inner_area(&self, area: Rect) -> Rect {
        let h_offset = 1 + self.horizontal_padding;
        let v_offset = 1 + self.vertical_padding;

        if area.width < (h_offset * 2) || area.height < (v_offset * 2) {
            Rect::default()
        } else {
            Rect {
                x: area.x + h_offset,
                y: area.y + v_offset,
                width: area.width - (h_offset * 2),
                height: area.height - (v_offset * 2),
            }
        }
    }
}

impl<'a> View for Card<'a> {
    fn measure(&self, width: Option<u16>, height: Option<u16>) -> (u16, u16) {
        let h_extra = 2 + (self.horizontal_padding * 2);
        let v_extra = 2 + (self.vertical_padding * 2);

        if let Some(content) = &self.content {
            let inner_w = width.map(|w| w.saturating_sub(h_extra));
            let inner_h = height.map(|h| h.saturating_sub(v_extra));
            let (cw, ch) = content.measure(inner_w, inner_h);
            (cw + h_extra, ch + v_extra)
        } else {
            let w = width.map(|w| w.max(h_extra)).unwrap_or(h_extra);
            let h = height.map(|h| h.max(v_extra)).unwrap_or(v_extra);
            (w, h)
        }
    }

    fn render(&mut self, area: Rect, terminal: &mut Terminal) -> io::Result<()> {
        if area.width < 2 || area.height < 2 {
            return Ok(());
        }

        let b_chars: Vec<char> = self
            .border_chars
            .as_ref()
            .map(|s| s.chars().collect())
            .unwrap_or_else(|| "╭╮╰╯─│".chars().collect());

        let c_tl = b_chars.get(0).unwrap_or(&'╭');
        let c_tr = b_chars.get(1).unwrap_or(&'╮');
        let c_bl = b_chars.get(2).unwrap_or(&'╰');
        let c_br = b_chars.get(3).unwrap_or(&'╯');
        let c_h = b_chars.get(4).unwrap_or(&'─');
        let c_v = b_chars.get(5).unwrap_or(&'│');

        if self.active {
            terminal.set_color(&self.primary_color)?;
        } else {
            terminal.set_color(&self.dim_color)?;
        }

        terminal.move_to(area.y, area.x)?;
        terminal.print(&c_tl.to_string())?;
        terminal.move_to(area.y, area.x + area.width - 1)?;
        terminal.print(&c_tr.to_string())?;
        terminal.move_to(area.y + area.height - 1, area.x)?;
        terminal.print(&c_bl.to_string())?;
        terminal.move_to(area.y + area.height - 1, area.x + area.width - 1)?;
        terminal.print(&c_br.to_string())?;

        for i in 1..area.width - 1 {
            terminal.move_to(area.y, area.x + i)?;
            terminal.print(&c_h.to_string())?;
            terminal.move_to(area.y + area.height - 1, area.x + i)?;
            terminal.print(&c_h.to_string())?;
        }
        for i in 1..area.height - 1 {
            terminal.move_to(area.y + i, area.x)?;
            terminal.print(&c_v.to_string())?;
            terminal.move_to(area.y + i, area.x + area.width - 1)?;
            terminal.print(&c_v.to_string())?;
        }

        if let Some(txt) = &self.title {
            if area.width > (txt.len() as u16 + 4) {
                terminal.move_to(area.y, area.x + 2)?;
                terminal.print(" ")?;
                terminal.set_color(&self.text_color)?;
                terminal.print(txt)?;
                if self.active {
                    terminal.set_color(&self.primary_color)?;
                } else {
                    terminal.set_color(&self.dim_color)?;
                }
                terminal.print(" ")?;
            }
        }
        terminal.reset_color()?;

        let inner = self.inner_area(area);
        if let Some(content) = &mut self.content {
            content.render(inner, terminal)?;
        }

        Ok(())
    }
}

pub struct ActionBar<'a> {
    pub items: Vec<(&'a str, &'a str)>,
    pub primary_color: String,
    pub dim_color: String,
    pub text_color: String,
}

impl<'a> ActionBar<'a> {
    pub fn new(items: Vec<(&'a str, &'a str)>) -> Self {
        Self {
            items,
            primary_color: "1;34".into(),
            dim_color: "2".into(),
            text_color: "1;37".into(),
        }
    }

    pub fn with_colors(mut self, primary: &str, dim: &str, text: &str) -> Self {
        self.primary_color = primary.into();
        self.dim_color = dim.into();
        self.text_color = text.into();
        self
    }

    fn calculate_layout(&self, width: u16) -> Vec<(u16, u16)> {
        let mut positions = Vec::with_capacity(self.items.len());
        let mut current_x = 0;
        let mut current_y = 0;
        let gap = 2;

        for (_i, (key, desc)) in self.items.iter().enumerate() {
            let item_width = key.len() + 1 + desc.len(); // key + ":" + desc

            // Check if we need to wrap.
            if current_x > 0 && current_x + (item_width as u16) > width {
                current_x = 0;
                current_y += 1;
            }

            positions.push((current_x, current_y));
            current_x += (item_width as u16) + (gap as u16);
        }
        positions
    }
}

impl<'a> View for ActionBar<'a> {
    fn measure(&self, width: Option<u16>, _height: Option<u16>) -> (u16, u16) {
        let width = width.unwrap_or(80);
        if self.items.is_empty() {
            return (0, 0);
        }
        let positions = self.calculate_layout(width);
        let last_y = positions.last().map(|&(_, y)| y).unwrap_or(0);
        (width, last_y + 1)
    }

    fn render(&mut self, area: Rect, terminal: &mut Terminal) -> io::Result<()> {
        let positions = self.calculate_layout(area.width);

        for (i, (key, desc)) in self.items.iter().enumerate() {
            if let Some(&(px, py)) = positions.get(i) {
                terminal.move_to(area.y + py, area.x + px)?;
                terminal.set_color(&self.primary_color)?;
                terminal.print(key)?;
                terminal.set_color(&self.dim_color)?;
                terminal.print(":")?;
                terminal.set_color(&self.text_color)?;
                terminal.print(desc)?;
            }
        }
        terminal.reset_color()?;
        Ok(())
    }
}

pub struct ListItem<'a> {
    pub text: Cow<'a, str>,
    pub icon: Option<&'a str>,
    pub right_text: Option<String>,
    pub dimmed: bool,
    pub highlight_override: bool,
    pub active: bool,
}

impl<'a> ListItem<'a> {
    pub fn new(text: impl Into<Cow<'a, str>>) -> Self {
        Self {
            text: text.into(),
            icon: None,
            right_text: None,
            dimmed: false,
            highlight_override: false,
            active: false,
        }
    }

    pub fn active(mut self, active: bool) -> Self {
        self.active = active;
        self
    }

    pub fn with_icon(mut self, icon: &'a str) -> Self {
        self.icon = Some(icon);
        self
    }

    pub fn with_right_text(mut self, text: String) -> Self {
        self.right_text = Some(text);
        self
    }

    pub fn dimmed(mut self, dimmed: bool) -> Self {
        self.dimmed = dimmed;
        self
    }

    pub fn highlight(mut self, highlight: bool) -> Self {
        self.highlight_override = highlight;
        self
    }
}

#[derive(Default)]
pub struct ListState {
    pub selected: usize,
    pub offset: usize,
}

impl ListState {
    pub fn new() -> Self {
        Self {
            selected: 0,
            offset: 0,
        }
    }

    pub fn select(&mut self, index: usize) {
        self.selected = index;
    }

    pub fn selected(&self) -> usize {
        self.selected
    }

    pub fn scroll_up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
    }

    pub fn scroll_down(&mut self, total_items: usize) {
        if total_items > 0 && self.selected < total_items - 1 {
            self.selected += 1;
        }
    }

    pub fn scroll_to_top(&mut self) {
        self.selected = 0;
        self.offset = 0;
    }

    pub fn scroll_to_bottom(&mut self, total_items: usize) {
        if total_items > 0 {
            self.selected = total_items - 1;
        }
    }

    pub fn ensure_visible(&mut self, height: usize) {
        if height == 0 {
            return;
        }
        if self.selected < self.offset {
            self.offset = self.selected;
        } else if self.selected >= self.offset + height {
            self.offset = self.selected - height + 1;
        }
    }
}

pub struct List<'a, 's> {
    pub items: Vec<ListItem<'a>>,
    pub highlight_color: String,
    pub dim_color: String,
    pub text_color: String,
    pub primary_color: String,
    pub active_icon: Option<&'a str>,
    pub state: &'s mut ListState,
}

impl<'a, 's> List<'a, 's> {
    pub fn new(items: Vec<ListItem<'a>>, state: &'s mut ListState) -> Self {
        Self {
            items,
            highlight_color: "7".into(),
            dim_color: "2".into(),
            text_color: "1;37".into(),
            primary_color: "1;34".into(),
            active_icon: None,
            state,
        }
    }

    pub fn with_active_icon(mut self, icon: &'a str) -> Self {
        self.active_icon = Some(icon);
        self
    }

    pub fn with_colors(mut self, highlight: &str, dim: &str, text: &str, primary: &str) -> Self {
        self.highlight_color = highlight.into();
        self.dim_color = dim.into();
        self.text_color = text.into();
        self.primary_color = primary.into();
        self
    }
}

impl<'a, 's> View for List<'a, 's> {
    fn measure(&self, width: Option<u16>, height: Option<u16>) -> (u16, u16) {
        (width.unwrap_or(0), height.unwrap_or(0))
    }

    fn render(&mut self, area: Rect, terminal: &mut Terminal) -> io::Result<()> {
        let max_display = area.height as usize;
        if max_display == 0 {
            return Ok(());
        }

        self.state.ensure_visible(max_display);

        for (i, item) in self
            .items
            .iter()
            .skip(self.state.offset)
            .take(max_display)
            .enumerate()
        {
            let absolute_index = self.state.offset + i;
            terminal.move_to(area.y + i as u16, area.x)?;
            let is_selected = absolute_index == self.state.selected;

            if is_selected {
                terminal.set_color(&self.highlight_color)?;
            } else {
                terminal.reset_color()?;
            }

            let mut cur_len = 0;
            if item.active && self.active_icon.is_some() {
                if !is_selected {
                    terminal.set_color(&self.primary_color)?;
                }
                terminal.print(&format!("{} ", self.active_icon.unwrap()))?;
                if !is_selected {
                    terminal.reset_color()?;
                }
                cur_len += 2;
            } else {
                terminal.print("  ")?;
                cur_len += 2;
            }

            if let Some(icon) = item.icon {
                terminal.print(&format!("{} ", icon))?;
                cur_len += 2;
            }

            let max_text_len =
                area.width.saturating_sub(cur_len as u16).saturating_sub(10) as usize;
            let text_preview: String = item
                .text
                .chars()
                .filter(|c| !c.is_control())
                .take(max_text_len)
                .collect();

            terminal.print(&text_preview)?;
            cur_len += text_preview.len();

            if let Some(right) = &item.right_text {
                let fill = (area.width as usize)
                    .saturating_sub(cur_len)
                    .saturating_sub(right.len());
                for _ in 0..fill {
                    terminal.print(" ")?;
                }

                if !is_selected && item.dimmed {
                    terminal.set_color(&self.dim_color)?;
                } else if !is_selected && item.highlight_override {
                    terminal.set_color(&self.primary_color)?;
                }

                terminal.print(right)?;

                if is_selected {
                    terminal.set_color(&self.highlight_color)?;
                }
            }

            terminal.reset_color()?;
        }
        Ok(())
    }
}

pub struct Input<'a> {
    pub value: &'a str,
    pub placeholder: &'a str,
    pub prefix: &'a str,
    pub active: bool,
    pub primary_color: String,
    pub dim_color: String,
    pub text_color: String,
}

impl<'a> Input<'a> {
    pub fn new() -> Self {
        Self {
            value: "",
            placeholder: "",
            prefix: "",
            active: false,
            primary_color: "1;34".into(),
            dim_color: "2".into(),
            text_color: "1;37".into(),
        }
    }

    pub fn with_value(mut self, value: &'a str) -> Self {
        self.value = value;
        self
    }

    pub fn with_placeholder(mut self, placeholder: &'a str) -> Self {
        self.placeholder = placeholder;
        self
    }

    pub fn with_prefix(mut self, prefix: &'a str) -> Self {
        self.prefix = prefix;
        self
    }

    pub fn active(mut self, active: bool) -> Self {
        self.active = active;
        self
    }

    pub fn with_colors(mut self, primary: &str, dim: &str, text: &str) -> Self {
        self.primary_color = primary.into();
        self.dim_color = dim.into();
        self.text_color = text.into();
        self
    }

    pub fn handle_event(value: &mut String, key: Key) -> bool {
        match key {
            Key::Backspace => {
                if !value.is_empty() {
                    value.pop();
                    return true;
                }
            }
            Key::Char(c) => {
                value.push(c);
                return true;
            }
            _ => {}
        }
        false
    }
}

impl<'a> View for Input<'a> {
    fn measure(&self, width: Option<u16>, _height: Option<u16>) -> (u16, u16) {
        (width.unwrap_or(0), 1)
    }

    fn render(&mut self, area: Rect, terminal: &mut Terminal) -> io::Result<()> {
        terminal.move_to(area.y, area.x)?;
        terminal.set_color(&self.primary_color)?;
        terminal.print(self.prefix)?;

        if !self.value.is_empty() {
            terminal.set_color(&self.text_color)?;
            terminal.print(self.value)?;
        } else {
            terminal.set_color(&self.dim_color)?;
            terminal.print(self.placeholder)?;
        }

        if self.active {
            terminal.set_color("5;37")?; // Blink white
            terminal.print("█")?;
        }

        terminal.reset_color()
    }
}

pub struct TextView {
    pub text: String,
}

impl TextView {
    pub fn new(text: impl Into<String>) -> Self {
        Self { text: text.into() }
    }
}

impl View for TextView {
    fn measure(&self, width: Option<u16>, _height: Option<u16>) -> (u16, u16) {
        let lines = self.text.lines().count() as u16;
        let max_w = self.text.lines().map(|l| l.len()).max().unwrap_or(0) as u16;
        (width.unwrap_or(max_w), lines)
    }

    fn render(&mut self, area: Rect, terminal: &mut Terminal) -> io::Result<()> {
        for (i, line) in self.text.lines().take(area.height as usize).enumerate() {
            terminal.move_to(area.y + i as u16, area.x)?;
            // Filter out control characters except some basic ones to prevent escape injections
            let filtered_line: String = line
                .chars()
                .filter(|c| !c.is_control())
                .take(area.width as usize)
                .collect();
            terminal.print(&filtered_line)?;
        }
        Ok(())
    }
}

pub struct ImageView<'a> {
    pub data: &'a [u8],
}

impl<'a> ImageView<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data }
    }
}

impl<'a> View for ImageView<'a> {
    fn measure(&self, width: Option<u16>, height: Option<u16>) -> (u16, u16) {
        (width.unwrap_or(0), height.unwrap_or(0))
    }

    fn render(&mut self, area: Rect, terminal: &mut Terminal) -> io::Result<()> {
        crate::image_proc::ImageProcessor::render_image(
            terminal,
            self.data,
            area.x,
            area.y,
            area.width,
            area.height,
        )
    }
}

pub struct EmptyView;

impl View for EmptyView {
    fn measure(&self, _width: Option<u16>, _height: Option<u16>) -> (u16, u16) {
        (0, 0)
    }
    fn render(&mut self, _area: Rect, _terminal: &mut Terminal) -> io::Result<()> {
        Ok(())
    }
}

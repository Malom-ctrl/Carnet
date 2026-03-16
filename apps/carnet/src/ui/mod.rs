pub mod preview;
pub mod renderer;

pub use term_uikit::event::{InputHandler, Key};
pub use term_uikit::terminal::Terminal;

#[derive(Clone, Copy, PartialEq)]
pub enum Mode {
    Normal,
    Search,
    Tools,
}

pub fn fuzzy_match(query: &str, text: &str) -> bool {
    if query.is_empty() {
        return true;
    }
    let query = query.to_lowercase();
    let text = text.to_lowercase();
    let mut it = text.chars();
    for q_char in query.chars() {
        if it.find(|&t_char| t_char == q_char).is_none() {
            return false;
        }
    }
    true
}

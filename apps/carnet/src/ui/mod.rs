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
    score_fuzzy(query, text) > 0
}

pub fn score_fuzzy(query: &str, text: &str) -> i32 {
    if query.is_empty() {
        return 1;
    }

    let query_chars: Vec<char> = query.to_lowercase().chars().collect();
    let text_chars: Vec<char> = text.to_lowercase().chars().collect();

    let mut score = 0;
    let mut text_idx = 0;
    let mut last_match_idx = 0;
    let mut match_count = 0;

    for &q_char in &query_chars {
        let mut found = false;
        while text_idx < text_chars.len() {
            if text_chars[text_idx] == q_char {
                // Base score for a match
                score += 10;

                // Bonus for proximity to last match
                if match_count > 0 && text_idx == last_match_idx + 1 {
                    score += 20;
                }

                // Bonus for word boundaries (start of string, after space, underscore, dot, or slash)
                if text_idx == 0
                    || text_chars[text_idx - 1] == ' '
                    || text_chars[text_idx - 1] == '_'
                    || text_chars[text_idx - 1] == '.'
                    || text_chars[text_idx - 1] == '/'
                {
                    score += 30;
                }

                last_match_idx = text_idx;
                text_idx += 1;
                match_count += 1;
                found = true;
                break;
            }
            text_idx += 1;
        }

        if !found {
            return 0;
        }
    }

    // Length penalty to prefer shorter matches
    score -= (text_chars.len() as i32) / 2;
    score
}

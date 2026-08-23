use crate::app::{Action, AppState, InputMode};
use crate::keymap::KeyAction;

/// Typing always leaves the chip cursor: it selects an already-entered tag, and
/// the two cannot be active at once.
pub fn insert_char(state: &mut AppState, c: char) {
    if state.tag_cursor_index.is_some() {
        state.tag_cursor_index = None;
    }
    state.tag_input.push(c);
}

pub fn dispatch(state: &mut AppState, action: KeyAction) -> Action {
    match action {
        KeyAction::CursorLeft => {
            let tags = state.selected_command_tags();
            if !tags.is_empty() {
                if let Some(idx) = state.tag_cursor_index {
                    state.tag_cursor_index = Some(if idx == 0 { tags.len() - 1 } else { idx - 1 });
                } else {
                    state.tag_cursor_index = Some(tags.len() - 1);
                }
            }
        }
        KeyAction::CursorRight => {
            let tags = state.selected_command_tags();
            if let Some(idx) = state.tag_cursor_index {
                state.tag_cursor_index = Some(if idx + 1 >= tags.len() { 0 } else { idx + 1 });
            }
        }
        KeyAction::DeleteCharBackward => {
            if let Some(idx) = state.tag_cursor_index {
                let mut tags = state.selected_command_tags();
                if idx < tags.len() {
                    tags.remove(idx);
                    let new_len = tags.len();
                    state.set_tags(tags);
                    state.tag_cursor_index = if new_len == 0 {
                        None
                    } else if idx >= new_len {
                        Some(new_len - 1)
                    } else {
                        Some(idx)
                    };
                }
            } else if state.tag_input.is_empty() {
                let tags = state.selected_command_tags();
                if !tags.is_empty() {
                    state.tag_cursor_index = Some(tags.len() - 1);
                }
            } else {
                state.tag_input.pop();
                state.tag_selected_index = 0;
            }
        }
        KeyAction::KillLine => {
            state.tag_input.clear();
            state.tag_selected_index = 0;
            state.tag_cursor_index = None;
        }
        KeyAction::AcceptSuggestion => {
            let no_cursor = state.tag_cursor_index.is_none();
            let suggestions = state.filtered_tags();
            let has_valid = !suggestions.is_empty() && state.tag_selected_index < suggestions.len();

            if no_cursor && has_valid {
                let tag = suggestions[state.tag_selected_index].clone();
                state.apply_selected_tag(tag);
                state.tag_selected_index = 0;
            }
        }
        KeyAction::NavigateUp => {
            state.tag_selected_index = state.tag_selected_index.saturating_sub(1);
        }
        KeyAction::NavigateDown => {
            let suggestions = state.filtered_tags();
            let show_create = !state.tag_input.trim().is_empty()
                && !suggestions
                    .iter()
                    .any(|t| t.to_lowercase() == state.tag_input.trim().to_lowercase());

            let max_index = if show_create {
                suggestions.len()
            } else {
                suggestions.len().saturating_sub(1)
            };
            state.tag_selected_index = (state.tag_selected_index + 1).min(max_index);
        }
        KeyAction::Confirm => {
            let search_text = state.tag_input.trim().to_string();
            let selected_idx = state.tag_selected_index;

            let suggestions = state.filtered_tags();

            if search_text.is_empty() {
                state.tag_input.clear();
                state.tag_cursor_index = None;
                state.input_mode = InputMode::Normal;
                state.tag_selected_index = 0;
            } else {
                let search_lower = search_text.to_lowercase();
                let exact_match = suggestions.iter().any(|t| t.to_lowercase() == search_lower);

                let mut new_tag: Option<String> = None;

                if selected_idx < suggestions.len() {
                    new_tag = Some(suggestions[selected_idx].clone());
                } else if !exact_match {
                    new_tag = Some(search_text.clone());
                }

                if let Some(tag) = new_tag {
                    let mut tags = state.selected_command_tags();
                    if !tags.contains(&tag) {
                        tags.push(tag);
                        tags.sort();
                        state.set_tags(tags);
                    }
                }

                state.tag_input.clear();
                state.tag_cursor_index = None;
                state.input_mode = InputMode::Normal;
                state.tag_selected_index = 0;
            }
        }
        _ => {}
    }
    Action::None
}

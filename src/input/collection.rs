use crate::app::{Action, AppState, CollectionInputMode, InputMode};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

pub fn handle(state: &mut AppState, key: KeyEvent) -> Action {
    match (key.code, key.modifiers) {
        (KeyCode::Char(c), KeyModifiers::NONE) => {
            if state.collection_input_mode == CollectionInputMode::AddToCollection {
                state.collection_input_text.push(c);
                state.collection_popup_index = 0;
            } else {
                state.collection_input_text.push(c);
            }
        }
        (KeyCode::Backspace, _) => {
            if state.collection_input_mode == CollectionInputMode::AddToCollection {
                if !state.collection_input_text.is_empty() {
                    state.collection_input_text.pop();
                    state.collection_popup_index = state.collection_popup_index.saturating_sub(1);
                }
            } else {
                state.collection_input_text.pop();
            }
        }
        (KeyCode::Up, _) => {
            if state.collection_input_mode == CollectionInputMode::AddToCollection {
                state.collection_popup_index = state.collection_popup_index.saturating_sub(1);
            }
        }
        (KeyCode::Down, _) => {
            if state.collection_input_mode == CollectionInputMode::AddToCollection {
                let search_text = &state.collection_input_text;
                let show_create = !search_text.is_empty()
                    && !state
                        .filtered_collections(search_text)
                        .iter()
                        .any(|c| c.name.to_lowercase() == search_text.to_lowercase());

                let filtered_count = state.filtered_collections(search_text).len();
                let max_index = if show_create {
                    filtered_count
                } else {
                    filtered_count.saturating_sub(1)
                };
                state.collection_popup_index = (state.collection_popup_index + 1).min(max_index);
            }
        }
        (KeyCode::Enter, _) => {
            match state.collection_input_mode {
                CollectionInputMode::NewCollection => {
                    if !state.collection_input_text.is_empty() {
                        state.create_collection(state.collection_input_text.clone());
                    }
                }
                CollectionInputMode::EditCollection => {
                    let id = state.editing_collection_id.clone();
                    let text = state.collection_input_text.clone();
                    if let (Some(id), false) = (id, text.is_empty()) {
                        state.rename_collection(&id, text);
                    }
                }
                CollectionInputMode::AddToCollection => {
                    let search_text = state.collection_input_text.trim().to_string();
                    let selected_idx = state.collection_popup_index;
                    let cmd_text = state.active_command().map(|c| c.text.clone());

                    let filtered = state.filtered_collections(&search_text);

                    if selected_idx < filtered.len() {
                        let col_id = filtered[selected_idx].id.clone();
                        if let Some(text) = cmd_text {
                            state.add_command_to_collection(&text, &col_id);
                        }
                    } else if !search_text.is_empty() && selected_idx == filtered.len() {
                        state.create_collection(search_text.clone());
                        let new_col_id = state
                            .collections
                            .iter()
                            .find(|c| c.name.to_lowercase() == search_text.to_lowercase())
                            .map(|c| c.id.clone());
                        if let (Some(col_id), Some(text)) = (new_col_id, cmd_text) {
                            state.add_command_to_collection(&text, &col_id);
                        }
                    }
                }
                CollectionInputMode::None => {}
            }
            state.input_mode = InputMode::Normal;
            state.collection_input_mode = CollectionInputMode::None;
            state.collection_input_text.clear();
            state.editing_collection_id = None;
        }
        (KeyCode::Esc, _) => {
            state.input_mode = InputMode::Normal;
            state.collection_input_mode = CollectionInputMode::None;
            state.collection_input_text.clear();
            state.editing_collection_id = None;
        }
        _ => {}
    }
    Action::None
}

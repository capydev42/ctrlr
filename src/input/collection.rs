use crate::app::{Action, AppState, CollectionInputMode, InputMode};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

pub fn handle(state: &mut AppState, key: KeyEvent) -> Action {
    match (key.code, key.modifiers) {
        (KeyCode::Char(c), KeyModifiers::NONE) => match state.collection_input_mode {
            CollectionInputMode::AddToCollection => {
                state.collection_input_text.push(c);
                state.collection_popup_index = 0;
            }
            CollectionInputMode::AddToCollectionSearch => {
                state.collection_input_text.push(c);
                state.add_command_search_index = 0;
            }
            _ => {
                state.collection_input_text.push(c);
            }
        },
        (KeyCode::Backspace, _) => match state.collection_input_mode {
            CollectionInputMode::AddToCollection => {
                if !state.collection_input_text.is_empty() {
                    state.collection_input_text.pop();
                    state.collection_popup_index = state.collection_popup_index.saturating_sub(1);
                }
            }
            CollectionInputMode::AddToCollectionSearch => {
                if !state.collection_input_text.is_empty() {
                    state.collection_input_text.pop();
                    state.add_command_search_index = 0;
                }
            }
            _ => {
                state.collection_input_text.pop();
            }
        },
        (KeyCode::Up, _) => match state.collection_input_mode {
            CollectionInputMode::AddToCollection => {
                state.collection_popup_index = state.collection_popup_index.saturating_sub(1);
            }
            CollectionInputMode::AddToCollectionSearch => {
                state.add_command_search_index = state.add_command_search_index.saturating_sub(1);
            }
            _ => {}
        },
        (KeyCode::Down, _) => match state.collection_input_mode {
            CollectionInputMode::AddToCollection => {
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
            CollectionInputMode::AddToCollectionSearch => {
                let results = state.search_results_for_add_command();
                let search_text = state.collection_input_text.trim();
                let search_lower = search_text.to_lowercase();
                let exact_match = results
                    .iter()
                    .any(|c| c.text.to_lowercase() == search_lower);
                let show_create = !search_text.is_empty() && !exact_match;

                let results_count = results.len();
                let max_index = if show_create {
                    results_count
                } else {
                    results_count.saturating_sub(1)
                };
                state.add_command_search_index =
                    (state.add_command_search_index + 1).min(max_index);
            }
            _ => {}
        },
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
                CollectionInputMode::AddToCollectionSearch => {
                    let search_text = state.collection_input_text.trim().to_string();
                    let selected_idx = state.add_command_search_index;
                    let results = state.search_results_for_add_command();
                    let search_lower = search_text.to_lowercase();
                    let exact_match = results
                        .iter()
                        .any(|c| c.text.to_lowercase() == search_lower);

                    if selected_idx < results.len() {
                        let cmd = results[selected_idx].text.clone();
                        state.add_command_to_collection_by_text(&cmd);
                    } else if !search_text.is_empty() && !exact_match {
                        state.add_command_to_collection_by_text(&search_text);
                    }
                }
                CollectionInputMode::None => {}
            }
            state.input_mode = InputMode::Normal;
            state.collection_input_mode = CollectionInputMode::None;
            state.collection_input_text.clear();
            state.editing_collection_id = None;
            state.add_command_search_index = 0;
        }
        (KeyCode::Esc, _) => {
            state.input_mode = InputMode::Normal;
            state.collection_input_mode = CollectionInputMode::None;
            state.collection_input_text.clear();
            state.editing_collection_id = None;
            state.add_command_search_index = 0;
        }
        _ => {}
    }
    Action::None
}

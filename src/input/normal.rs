use crate::app::{Action, ActivePane, AppState, CollectionInputMode, InputMode, ViewMode};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

pub fn handle(state: &mut AppState, key: KeyEvent) -> Action {
    match (key.code, key.modifiers) {
        (KeyCode::Tab, _) => {
            state.switch_pane();
            return Action::None;
        }
        (KeyCode::Char('1'), KeyModifiers::NONE) => {
            state.view_mode = ViewMode::History;
            state.active_pane = ActivePane::History;
            state.filter_commands();
            return Action::None;
        }
        (KeyCode::Char('2'), KeyModifiers::NONE) => {
            state.view_mode = ViewMode::Favorites;
            state.active_pane = ActivePane::History;
            state.filter_commands();
            return Action::None;
        }
        (KeyCode::Char('3'), KeyModifiers::NONE) => {
            state.view_mode = ViewMode::Collections;
            state.active_pane = ActivePane::CollectionsList;
            state.load_collection_commands();
            state.filter_commands();
            return Action::None;
        }
        (KeyCode::Char('j'), KeyModifiers::CONTROL) => {
            state.pane_down();
            return Action::None;
        }
        (KeyCode::Char('k'), KeyModifiers::CONTROL) => {
            state.pane_up();
            return Action::None;
        }
        (KeyCode::Char('h'), KeyModifiers::CONTROL) => {
            state.pane_left();
            return Action::None;
        }
        (KeyCode::Char('l'), KeyModifiers::CONTROL) => {
            state.pane_right();
            return Action::None;
        }
        (KeyCode::Char('c'), KeyModifiers::NONE) => {
            if state.active_pane == ActivePane::Search {
                state.add_to_search('c');
            } else {
                let has_selection = match state.view_mode {
                    ViewMode::Collections => {
                        matches!(state.active_pane, ActivePane::CollectionItems)
                            && !state.collection_commands.is_empty()
                    }
                    _ => !state.filtered.is_empty(),
                };
                if has_selection {
                    state.collection_input_mode = CollectionInputMode::AddToCollection;
                    state.input_mode = InputMode::CollectionInput;
                }
            }
            return Action::None;
        }
        (KeyCode::Enter, _) => {
            if state.view_mode == ViewMode::Collections {
                match state.active_pane {
                    ActivePane::CollectionsList => {
                        state.load_collection_commands();
                        state.active_pane = ActivePane::CollectionItems;
                        state.selected_index = 0;
                        state.list_state.select(Some(0));
                        return Action::None;
                    }
                    ActivePane::CollectionItems => {
                        let cmd = state.filtered.get(state.selected_index).cloned();
                        if let Some(ref c) = cmd {
                            state.mark_executed_for_text(&c.text);
                        }
                        return cmd.map(|c| Action::Execute(c.text)).unwrap_or(Action::None);
                    }
                    _ => return Action::None,
                }
            }
            let cmd = state.selected_command();
            state.mark_executed();
            return cmd.map(Action::Execute).unwrap_or(Action::None);
        }
        _ => {}
    }

    match (key.code, key.modifiers) {
        (KeyCode::Up, _) => {
            state.clear_key_buffer();
            handle_navigation_up(state);
        }
        (KeyCode::Down, _) => {
            state.clear_key_buffer();
            handle_navigation_down(state);
        }
        (KeyCode::PageDown, _) | (KeyCode::Char('d'), KeyModifiers::CONTROL) => {
            state.clear_key_buffer();
            handle_page_down(state);
        }
        (KeyCode::PageUp, _) | (KeyCode::Char('u'), KeyModifiers::CONTROL) => {
            state.clear_key_buffer();
            handle_page_up(state);
        }
        (KeyCode::Char('g'), KeyModifiers::NONE) | (KeyCode::Char('g'), KeyModifiers::SHIFT) => {
            if !matches!(
                state.active_pane,
                ActivePane::History | ActivePane::CollectionsList | ActivePane::CollectionItems
            ) {
                state.clear_key_buffer();
            } else {
                state.check_key_buffer_timeout();
                if state.key_buffer == Some('g') {
                    state.clear_key_buffer();
                    handle_go_to_top(state);
                } else {
                    state.set_key_buffer('g');
                }
            }
        }
        (KeyCode::Char('G'), KeyModifiers::NONE) | (KeyCode::Char('G'), KeyModifiers::SHIFT) => {
            if !matches!(
                state.active_pane,
                ActivePane::History | ActivePane::CollectionsList | ActivePane::CollectionItems
            ) {
                state.clear_key_buffer();
            } else {
                state.clear_key_buffer();
                handle_go_to_bottom(state);
            }
        }
        (KeyCode::Esc, _) => {
            state.clear_key_buffer();
            state.handle_esc();
        }
        _ => {
            state.clear_key_buffer();
        }
    }

    match state.active_pane {
        ActivePane::Search => match (key.code, key.modifiers) {
            (KeyCode::Char(c), KeyModifiers::NONE) => {
                state.add_to_search(c);
            }
            (KeyCode::Backspace, _) => {
                state.remove_from_search();
            }
            _ => {}
        },
        ActivePane::History => match (key.code, key.modifiers) {
            (KeyCode::Char('/'), KeyModifiers::NONE) => {
                state.active_pane = ActivePane::Search;
            }
            (KeyCode::Char('t'), KeyModifiers::NONE) => {
                state.input_mode = InputMode::TagInput;
                state.tag_input = String::new();
                state.tag_selected_index = 0;
                state.tag_cursor_index = None;
            }
            (KeyCode::Char('j'), KeyModifiers::NONE) => {
                state.navigate_down();
                state.list_state.select(Some(state.selected_index));
            }
            (KeyCode::Char('k'), KeyModifiers::NONE) => {
                state.navigate_up();
                state.list_state.select(Some(state.selected_index));
            }
            (KeyCode::Char('f'), KeyModifiers::NONE) => {
                state.toggle_favorite();
            }
            (KeyCode::Char('d'), KeyModifiers::NONE) => {
                state.show_details = !state.show_details;
            }
            _ => {}
        },
        ActivePane::CollectionsList => match (key.code, key.modifiers) {
            (KeyCode::Char('/'), KeyModifiers::NONE) => {
                state.active_pane = ActivePane::Search;
            }
            (KeyCode::Char('n'), KeyModifiers::NONE) => {
                state.collection_input_mode = CollectionInputMode::NewCollection;
                state.collection_input_text.clear();
                state.input_mode = InputMode::CollectionInput;
            }
            (KeyCode::Char('e'), KeyModifiers::NONE) => {
                if let Some(col) = state.selected_collection() {
                    let col_id = col.id.clone();
                    let col_name = col.name.clone();
                    state.editing_collection_id = Some(col_id);
                    state.collection_input_text = col_name;
                    state.collection_input_mode = CollectionInputMode::EditCollection;
                    state.input_mode = InputMode::CollectionInput;
                }
            }
            (KeyCode::Char('d'), KeyModifiers::NONE) => {
                state.delete_collection();
            }
            (KeyCode::Char('j'), KeyModifiers::NONE) => {
                state.navigate_collection_down();
                state
                    .collection_list_state
                    .select(Some(state.selected_collection_index));
            }
            (KeyCode::Char('k'), KeyModifiers::NONE) => {
                state.navigate_collection_up();
                state
                    .collection_list_state
                    .select(Some(state.selected_collection_index));
            }
            _ => {}
        },
        ActivePane::CollectionItems => match (key.code, key.modifiers) {
            (KeyCode::Char('/'), KeyModifiers::NONE) => {
                state.active_pane = ActivePane::Search;
            }
            (KeyCode::Char('d'), KeyModifiers::NONE) => {
                state.show_details = !state.show_details;
            }
            (KeyCode::Char('a'), KeyModifiers::NONE) => {
                state.collection_input_mode = CollectionInputMode::AddToCollectionSearch;
                state.collection_input_text.clear();
                state.input_mode = InputMode::CollectionInput;
                state.add_command_search_index = 0;
            }
            (KeyCode::Char('r'), KeyModifiers::NONE) => {
                if let Some(cmd) = state.filtered.get(state.selected_index) {
                    let text = cmd.text.clone();
                    state.remove_command_from_collection(&text);
                }
            }
            (KeyCode::Char('j'), KeyModifiers::NONE) => {
                state.navigate_down();
                state
                    .collection_items_list_state
                    .select(Some(state.selected_index));
            }
            (KeyCode::Char('k'), KeyModifiers::NONE) => {
                state.navigate_up();
                state
                    .collection_items_list_state
                    .select(Some(state.selected_index));
            }
            _ => {}
        },
    }

    Action::None
}

fn handle_navigation_up(state: &mut AppState) {
    match state.view_mode {
        ViewMode::Collections => match state.active_pane {
            ActivePane::CollectionsList => state.navigate_collection_up(),
            ActivePane::CollectionItems => {
                state.navigate_up();
                state.list_state.select(Some(state.selected_index));
            }
            _ => {
                if state.active_pane == ActivePane::Search {
                    state.active_pane = ActivePane::History;
                }
                state.navigate_up();
                state.list_state.select(Some(state.selected_index));
            }
        },
        _ => {
            if state.active_pane == ActivePane::Search {
                state.active_pane = ActivePane::History;
            }
            state.navigate_up();
            state.list_state.select(Some(state.selected_index));
        }
    }
}

fn handle_navigation_down(state: &mut AppState) {
    match state.view_mode {
        ViewMode::Collections => match state.active_pane {
            ActivePane::CollectionsList => {
                state.navigate_collection_down();
                state
                    .collection_list_state
                    .select(Some(state.selected_collection_index));
            }
            ActivePane::CollectionItems => {
                state.navigate_down();
                state.list_state.select(Some(state.selected_index));
            }
            _ => {
                if state.active_pane == ActivePane::Search {
                    state.active_pane = ActivePane::History;
                }
                state.navigate_down();
                state.list_state.select(Some(state.selected_index));
            }
        },
        _ => {
            if state.active_pane == ActivePane::Search {
                state.active_pane = ActivePane::History;
            }
            state.navigate_down();
            state.list_state.select(Some(state.selected_index));
        }
    }
}

fn handle_page_down(state: &mut AppState) {
    match state.view_mode {
        ViewMode::Collections => match state.active_pane {
            ActivePane::CollectionsList => {
                state.navigate_collection_page_down();
                state
                    .collection_list_state
                    .select(Some(state.selected_collection_index));
            }
            ActivePane::CollectionItems => {
                state.navigate_page_down();
                state.list_state.select(Some(state.selected_index));
            }
            _ => {
                if state.active_pane == ActivePane::Search {
                    state.active_pane = ActivePane::History;
                }
                state.navigate_page_down();
                state.list_state.select(Some(state.selected_index));
            }
        },
        _ => {
            if state.active_pane == ActivePane::Search {
                state.active_pane = ActivePane::History;
            }
            state.navigate_page_down();
            state.list_state.select(Some(state.selected_index));
        }
    }
}

fn handle_page_up(state: &mut AppState) {
    match state.view_mode {
        ViewMode::Collections => match state.active_pane {
            ActivePane::CollectionsList => {
                state.navigate_collection_page_up();
                state
                    .collection_list_state
                    .select(Some(state.selected_collection_index));
            }
            ActivePane::CollectionItems => {
                state.navigate_page_up();
                state.list_state.select(Some(state.selected_index));
            }
            _ => {
                if state.active_pane == ActivePane::Search {
                    state.active_pane = ActivePane::History;
                }
                state.navigate_page_up();
                state.list_state.select(Some(state.selected_index));
            }
        },
        _ => {
            if state.active_pane == ActivePane::Search {
                state.active_pane = ActivePane::History;
            }
            state.navigate_page_up();
            state.list_state.select(Some(state.selected_index));
        }
    }
}

fn handle_go_to_top(state: &mut AppState) {
    match state.view_mode {
        ViewMode::Collections => match state.active_pane {
            ActivePane::CollectionsList => {
                state.go_to_collection_top();
            }
            ActivePane::CollectionItems => {
                state.go_to_top();
            }
            _ => {
                if state.active_pane == ActivePane::Search {
                    state.active_pane = ActivePane::History;
                }
                state.go_to_top();
            }
        },
        _ => {
            if state.active_pane == ActivePane::Search {
                state.active_pane = ActivePane::History;
            }
            state.go_to_top();
        }
    }
}

fn handle_go_to_bottom(state: &mut AppState) {
    match state.view_mode {
        ViewMode::Collections => match state.active_pane {
            ActivePane::CollectionsList => {
                state.go_to_collection_bottom();
            }
            ActivePane::CollectionItems => {
                state.go_to_bottom();
            }
            _ => {
                if state.active_pane == ActivePane::Search {
                    state.active_pane = ActivePane::History;
                }
                state.go_to_bottom();
            }
        },
        _ => {
            if state.active_pane == ActivePane::Search {
                state.active_pane = ActivePane::History;
            }
            state.go_to_bottom();
        }
    }
}

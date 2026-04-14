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
            handle_navigation_up(state);
        }
        (KeyCode::Down, _) => {
            handle_navigation_down(state);
        }
        (KeyCode::Esc, _) => {
            state.handle_esc();
        }
        _ => {}
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
                if state.selected_collection().is_some() {
                    state.editing_collection_id = state.selected_collection().map(|c| c.id.clone());
                    state.collection_input_text = state
                        .selected_collection()
                        .map(|c| c.name.clone())
                        .unwrap_or_default();
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
                if state.view_mode == ViewMode::Collections
                    && state.active_pane == ActivePane::CollectionItems
                {
                    state.collection_input_mode = CollectionInputMode::AddToCollectionSearch;
                    state.collection_input_text.clear();
                    state.input_mode = InputMode::CollectionInput;
                    state.add_command_search_index = 0;
                }
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

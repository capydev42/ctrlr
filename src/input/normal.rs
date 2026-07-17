use crate::app::{Action, ActivePane, AppState, CollectionInputMode, InputMode, ViewMode};
use crate::input::help;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::time::Instant;

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
        (KeyCode::Char('?'), KeyModifiers::NONE) => {
            state.help_open = true;
            state.help_search_query.clear();
            state.help_filtered_shortcuts = help::get_shortcuts_for_context(state);
            state.help_selected_index = 0;
            state.help_list_state.select(Some(0));
            return Action::None;
        }
        (KeyCode::Char('t'), KeyModifiers::CONTROL) => {
            state.open_theme_popup();
            return Action::None;
        }
        (KeyCode::Char('e'), KeyModifiers::CONTROL) => {
            state.open_export_popup();
            return Action::None;
        }
        (KeyCode::Char('o'), KeyModifiers::CONTROL) => {
            state.open_import_popup();
            return Action::None;
        }
        // Guarded so that outside the search bar this falls through to the
        // page-up arm below, which Ctrl+u shares with PageUp.
        (KeyCode::Char('u'), KeyModifiers::CONTROL) if state.active_pane == ActivePane::Search => {
            state.clear_key_buffer();
            state.clear_search();
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
            (KeyCode::Char(c), KeyModifiers::NONE | KeyModifiers::SHIFT) => {
                state.add_to_search(c);
            }
            (KeyCode::Backspace, _) => {
                state.remove_from_search();
            }
            _ => {}
        },
        ActivePane::History => match (key.code, key.modifiers) {
            (KeyCode::Backspace, _) => {
                state.active_pane = ActivePane::Search;
                state.remove_from_search();
            }
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
            (KeyCode::Char('y'), KeyModifiers::NONE) => {
                let text = state
                    .filtered
                    .get(state.selected_index)
                    .map(|c| c.text.clone());
                if let Some(text) = text {
                    let (success, msg) = crate::app::clipboard::copy_to_clipboard(&text);
                    if success {
                        state.status_message = Some("📋 Copied to clipboard".into());
                    } else if let Some(msg) = msg {
                        state.status_message = Some(msg);
                    }
                    state.status_timestamp = Some(Instant::now());
                }
            }
            (KeyCode::Char('d'), KeyModifiers::NONE) => {
                state.show_details = !state.show_details;
            }
            _ => {}
        },
        ActivePane::CollectionsList => match (key.code, key.modifiers) {
            (KeyCode::Backspace, _) => {
                state.active_pane = ActivePane::Search;
                state.remove_from_search();
            }
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
            (KeyCode::Backspace, _) => {
                state.active_pane = ActivePane::Search;
                state.remove_from_search();
            }
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
            (KeyCode::Char('y'), KeyModifiers::NONE) => {
                let text = state
                    .filtered
                    .get(state.selected_index)
                    .map(|c| c.text.clone());
                if let Some(text) = text {
                    let (success, msg) = crate::app::clipboard::copy_to_clipboard(&text);
                    if success {
                        state.status_message = Some("📋 Copied to clipboard".into());
                    } else if let Some(msg) = msg {
                        state.status_message = Some(msg);
                    }
                    state.status_timestamp = Some(Instant::now());
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::Command;

    fn cmd(text: &str) -> Command {
        Command {
            id: text.to_string(),
            text: text.to_string(),
            tags: vec![],
            collection_ids: vec![],
            favorite: false,
            _context: vec![],
            use_count: 0,
            last_used: None,
        }
    }

    fn state_with_query(pane: ActivePane, query: &str) -> AppState {
        let commands = (0..40).map(|i| cmd(&format!("command {}", i))).collect();
        let mut state = AppState::new(commands, None);
        state.active_pane = pane;
        state.search_query = query.to_string();
        state.filter_commands();
        state
    }

    fn ctrl(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)
    }

    #[test]
    fn test_ctrl_u_clears_search_and_keeps_focus() {
        let mut state = state_with_query(ActivePane::Search, "command 1");

        handle(&mut state, ctrl('u'));

        assert!(state.search_query.is_empty());
        // Must not fall through to page-up, which steals focus to History.
        assert_eq!(state.active_pane, ActivePane::Search);
        assert_eq!(state.selected_index, 0);
    }

    #[test]
    fn test_ctrl_u_still_pages_up_outside_search() {
        let mut state = state_with_query(ActivePane::History, "command");
        state.selected_index = 30;

        handle(&mut state, ctrl('u'));

        assert_eq!(state.search_query, "command", "query must be untouched");
        assert!(state.selected_index < 30, "should have paged up");
    }

    #[test]
    fn test_page_up_key_still_steals_focus_from_search() {
        let mut state = state_with_query(ActivePane::Search, "command");
        state.selected_index = 30;

        handle(
            &mut state,
            KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE),
        );

        assert_eq!(state.search_query, "command", "PageUp must not clear");
        assert_eq!(state.active_pane, ActivePane::History);
        assert!(state.selected_index < 30);
    }

    #[test]
    fn test_ctrl_u_on_empty_search_does_not_quit() {
        let mut state = state_with_query(ActivePane::Search, "");

        let action = handle(&mut state, ctrl('u'));

        assert!(matches!(action, Action::None), "must never signal exit");
        assert_eq!(state.active_pane, ActivePane::Search);
    }
}

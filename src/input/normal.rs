use crate::app::{Action, ActivePane, AppState, CollectionInputMode, InputMode, ViewMode};
use crate::keymap::{KeyAction, KeyContext};
use std::time::Instant;

/// Columns `<` and `>` move a divider by. Wide enough to be worth a keypress,
/// narrow enough to land where you meant.
pub const RESIZE_STEP: i16 = 4;

pub fn insert_char(state: &mut AppState, c: char) {
    state.add_to_search(c);
}

/// Keyed on the context as well as the action, because the panes genuinely
/// disagree about what "navigate down" means: the collections list moves a
/// different selection from the command list, and each pane keeps its own
/// `ListState`. The `Global` arm is what `Up`/`Down` do from anywhere, which
/// includes stealing focus away from the search bar.
pub fn dispatch(state: &mut AppState, context: KeyContext, action: KeyAction) -> Action {
    match (context, action) {
        (_, KeyAction::Execute) => return activate_selected(state),
        (_, KeyAction::EditCommand) => state.begin_edit_command(),

        // Movement from anywhere.
        (KeyContext::Global, KeyAction::NavigateUp) => handle_navigation_up(state),
        (KeyContext::Global, KeyAction::NavigateDown) => handle_navigation_down(state),
        (_, KeyAction::PageUp) => handle_page_up(state),
        (_, KeyAction::PageDown) => handle_page_down(state),
        (_, KeyAction::GoToTop) => handle_go_to_top(state),
        (_, KeyAction::GoToBottom) => handle_go_to_bottom(state),

        // Movement within a pane. Deliberately not routed through the two
        // helpers above: `j` in the collection items pane has always driven
        // `collection_items_list_state` while `Down` drives `list_state`.
        (KeyContext::History, KeyAction::NavigateDown) => {
            state.navigate_down();
            state.list_state.select(Some(state.selected_index));
        }
        (KeyContext::History, KeyAction::NavigateUp) => {
            state.navigate_up();
            state.list_state.select(Some(state.selected_index));
        }
        (KeyContext::CollectionsList, KeyAction::NavigateDown) => {
            state.navigate_collection_down();
            state
                .collection_list_state
                .select(Some(state.selected_collection_index));
        }
        (KeyContext::CollectionsList, KeyAction::NavigateUp) => {
            state.navigate_collection_up();
            state
                .collection_list_state
                .select(Some(state.selected_collection_index));
        }
        (KeyContext::CollectionItems, KeyAction::NavigateDown) => {
            state.navigate_down();
            state
                .collection_items_list_state
                .select(Some(state.selected_index));
        }
        (KeyContext::CollectionItems, KeyAction::NavigateUp) => {
            state.navigate_up();
            state
                .collection_items_list_state
                .select(Some(state.selected_index));
        }

        // Panes and views.
        (_, KeyAction::SwitchPane) => state.switch_pane(),
        (_, KeyAction::PaneDown) => state.pane_down(),
        (_, KeyAction::PaneUp) => state.pane_up(),
        (_, KeyAction::PaneLeft) => state.pane_left(),
        (_, KeyAction::PaneRight) => state.pane_right(),
        (_, KeyAction::ShrinkPane) => state.nudge_divider(-RESIZE_STEP),
        (_, KeyAction::GrowPane) => state.nudge_divider(RESIZE_STEP),
        (_, KeyAction::ViewHistory) => switch_view_history(state),
        (_, KeyAction::ViewFavorites) => switch_view_favorites(state),
        (_, KeyAction::ViewCollections) => switch_view_collections(state),
        (_, KeyAction::ScopeCwd) => toggle_cwd_scope(state),

        // Search.
        (_, KeyAction::FocusSearch) => state.active_pane = ActivePane::Search,
        (_, KeyAction::ClearSearch) => state.clear_search(),
        (KeyContext::Search, KeyAction::DeleteCharBackward) => state.remove_from_search(),
        // Backspace in a list pane both focuses the search bar and eats a
        // character, so holding it walks back out of a query.
        (_, KeyAction::DeleteCharBackward) => {
            state.active_pane = ActivePane::Search;
            state.remove_from_search();
        }

        // The selection.
        (_, KeyAction::ToggleFavorite) => state.toggle_favorite(),
        (_, KeyAction::ToggleDetails) => state.show_details = !state.show_details,
        (_, KeyAction::CopyToClipboard) => copy_selection(state),
        (_, KeyAction::EditTags) => {
            state.input_mode = InputMode::TagInput;
            state.tag_input = String::new();
            state.tag_selected_index = 0;
            state.tag_cursor_index = None;
        }
        (_, KeyAction::AddToCollection) if !state.filtered.is_empty() => {
            state.collection_input_mode = CollectionInputMode::AddToCollection;
            state.collection_input_text.clear();
            state.collection_popup_index = 0;
            state.input_mode = InputMode::CollectionInput;
        }

        // Collections.
        (_, KeyAction::NewCollection) => state.begin_new_collection(),
        (_, KeyAction::EditCollection) => state.begin_rename_collection(),
        (_, KeyAction::DeleteCollection) => state.delete_collection(),
        (_, KeyAction::SearchCollection) => {
            state.collection_input_mode = CollectionInputMode::AddToCollectionSearch;
            state.collection_input_text.clear();
            state.input_mode = InputMode::CollectionInput;
            state.add_command_search_index = 0;
        }
        (_, KeyAction::RemoveFromCollection) => {
            if let Some(cmd) = state.filtered.get(state.selected_index) {
                let text = cmd.text.clone();
                state.remove_command_from_collection(&text);
            }
        }

        // Overlays.
        (_, KeyAction::ShowHelp) => open_help(state),
        (_, KeyAction::ChangeTheme) => state.open_theme_popup(),
        (_, KeyAction::ExportData) => state.open_export_popup(),
        (_, KeyAction::ImportData) => state.open_import_popup(),

        _ => {}
    }
    Action::None
}

fn copy_selection(state: &mut AppState) {
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

/// What Enter does on the current selection: drill into a collection, or hand
/// the chosen command back to the shell. Shared with the mouse handler so a
/// double-click cannot drift from the keybinding.
pub fn activate_selected(state: &mut AppState) -> Action {
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
    cmd.map(Action::Execute).unwrap_or(Action::None)
}

pub fn switch_view_history(state: &mut AppState) {
    state.view_mode = ViewMode::History;
    state.active_pane = ActivePane::History;
    state.filter_commands();
}

pub fn switch_view_favorites(state: &mut AppState) {
    state.view_mode = ViewMode::Favorites;
    state.active_pane = ActivePane::History;
    state.filter_commands();
}

pub fn switch_view_collections(state: &mut AppState) {
    state.view_mode = ViewMode::Collections;
    state.active_pane = ActivePane::CollectionsList;
    state.load_collection_commands();
    state.filter_commands();
}

fn toggle_cwd_scope(state: &mut AppState) {
    let message = state.toggle_cwd_scope();
    state.set_status_message(message);
}

fn open_help(state: &mut AppState) {
    state.help_open = true;
    state.help_search_query.clear();
    state.help_filtered_shortcuts = super::help::get_shortcuts_for_context(state);
    state.help_selected_index = 0;
    state.help_list_state.select(Some(0));
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
    use crate::input;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

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
            runs_here: 0,
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

    fn alt(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::ALT)
    }

    fn plain(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE)
    }

    #[test]
    fn test_digit_types_into_search_when_focused() {
        let mut state = state_with_query(ActivePane::Search, "");

        input::handle(&mut state, plain('1'));

        assert_eq!(state.search_query, "1");
        assert_eq!(state.view_mode, ViewMode::History, "view must not change");
        assert_eq!(state.active_pane, ActivePane::Search);
    }

    #[test]
    fn test_question_mark_types_into_search() {
        let mut state = state_with_query(ActivePane::Search, "");

        input::handle(&mut state, plain('?'));

        assert_eq!(state.search_query, "?");
        assert!(!state.help_open, "help must not open from the search bar");
    }

    #[test]
    fn test_digit_switches_view_in_list_pane() {
        let mut state = state_with_query(ActivePane::History, "command");

        input::handle(&mut state, plain('2'));

        assert_eq!(state.view_mode, ViewMode::Favorites);
        assert_eq!(state.search_query, "command", "typing must not occur here");
    }

    #[test]
    fn test_alt_digit_switches_view_from_search() {
        let mut state = state_with_query(ActivePane::Search, "git");

        input::handle(&mut state, alt('3'));

        assert_eq!(state.view_mode, ViewMode::Collections);
        assert_eq!(state.search_query, "git", "query must be untouched");
    }

    #[test]
    fn test_f1_opens_help_from_search() {
        let mut state = state_with_query(ActivePane::Search, "");

        input::handle(&mut state, KeyEvent::new(KeyCode::F(1), KeyModifiers::NONE));

        assert!(state.help_open);
    }

    #[test]
    fn test_ctrl_u_clears_search_and_keeps_focus() {
        let mut state = state_with_query(ActivePane::Search, "command 1");

        input::handle(&mut state, ctrl('u'));

        assert!(state.search_query.is_empty());
        // Must not fall through to page-up, which steals focus to History.
        assert_eq!(state.active_pane, ActivePane::Search);
        assert_eq!(state.selected_index, 0);
    }

    #[test]
    fn test_ctrl_u_still_pages_up_outside_search() {
        let mut state = state_with_query(ActivePane::History, "command");
        state.selected_index = 30;

        input::handle(&mut state, ctrl('u'));

        assert_eq!(state.search_query, "command", "query must be untouched");
        assert!(state.selected_index < 30, "should have paged up");
    }

    #[test]
    fn test_page_up_key_still_steals_focus_from_search() {
        let mut state = state_with_query(ActivePane::Search, "command");
        state.selected_index = 30;

        input::handle(
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

        let action = input::handle(&mut state, ctrl('u'));

        assert!(matches!(action, Action::None), "must never signal exit");
        assert_eq!(state.active_pane, ActivePane::Search);
    }
}

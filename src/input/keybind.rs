//! The keybinding editor: rebind any key from inside ctrlr.
//!
//! Everything here reads and writes the same [`Keymap`] the rest of the app
//! resolves against, so a new key works the moment it is set rather than after
//! a restart. The file is written once on close — see
//! [`AppState::close_keybind_popup`].

use crate::app::{Action, AppState};
use crate::keymap::{Binding, KeyAction, KeyContext, format_binding};

/// What the next key press will do to the selected row.
///
/// All three are "press the key", which is why removing does not need a way to
/// point at one key among several: you name it by pressing it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CaptureMode {
    /// Drop whatever the action had and use this key instead.
    Replace,
    /// Keep the existing keys and add this one.
    Add,
    /// Take this key away, leaving the rest.
    Remove,
}

impl CaptureMode {
    pub fn prompt(self) -> &'static str {
        match self {
            CaptureMode::Replace => "Press the key to bind",
            CaptureMode::Add => "Press a key to add",
            CaptureMode::Remove => "Press the key to remove",
        }
    }
}

/// One line of the editor.
#[derive(Clone, Debug, PartialEq)]
pub struct KeybindRow {
    pub context: KeyContext,
    pub action: KeyAction,
    pub keys: Vec<String>,
    /// Two-key sequences cannot be recorded by pressing them — capture reads a
    /// single press — so those rows are shown but not editable.
    pub editable: bool,
}

/// Every action bound anywhere, plus the ones a context could have but does
/// not, so a user can add a key to something currently unbound.
pub fn rows(state: &AppState) -> Vec<KeybindRow> {
    let mut out = Vec::new();
    for &context in KeyContext::ALL {
        let entries = state.keymap.entries(context);
        // The defaults decide which actions belong to a context at all: an
        // action nobody ever bound there would be noise in a list this long.
        let mut actions: Vec<KeyAction> = Vec::new();
        for (_, action) in crate::keymap::defaults::keymap().entries(context) {
            if !actions.contains(action) {
                actions.push(*action);
            }
        }
        for (_, action) in entries {
            if !actions.contains(action) {
                actions.push(*action);
            }
        }
        for action in actions {
            let editable = !entries
                .iter()
                .any(|(b, a)| *a == action && matches!(b, Binding::Chord(..)));
            out.push(KeybindRow {
                context,
                action,
                keys: state.keymap.keys_for(context, action),
                editable,
            });
        }
    }
    out
}

/// Substring match over context and action names. Deliberately not fuzzy: the
/// names are machine-ish (`toggle_favorite`, `collections_list`) and a user
/// filtering this list is usually typing the exact word they saw.
pub fn filter(rows: &[KeybindRow], query: &str) -> Vec<KeybindRow> {
    if query.is_empty() {
        return rows.to_vec();
    }
    let needle = query.to_lowercase();
    rows.iter()
        .filter(|row| {
            row.action.as_str().contains(&needle)
                || row.context.as_str().contains(&needle)
                || row.keys.iter().any(|k| k.to_lowercase().contains(&needle))
        })
        .cloned()
        .collect()
}

pub fn insert_char(state: &mut AppState, c: char) {
    state.keybind_query.push(c);
    state.refresh_keybind_rows();
}

pub fn dispatch(state: &mut AppState, action: KeyAction) -> Action {
    match action {
        KeyAction::NavigateUp => {
            state.select_keybind_row(state.keybind_selected_index.saturating_sub(1));
        }
        KeyAction::NavigateDown => {
            state.select_keybind_row(state.keybind_selected_index + 1);
        }
        KeyAction::PageUp => {
            state.select_keybind_row(state.keybind_selected_index.saturating_sub(PAGE));
        }
        KeyAction::PageDown => {
            state.select_keybind_row(state.keybind_selected_index + PAGE);
        }
        KeyAction::DeleteCharBackward => {
            state.keybind_query.pop();
            state.refresh_keybind_rows();
        }
        KeyAction::KillLine => {
            state.keybind_query.clear();
            state.refresh_keybind_rows();
        }
        KeyAction::Confirm => state.begin_capture(CaptureMode::Replace),
        KeyAction::AddKeybinding => state.begin_capture(CaptureMode::Add),
        KeyAction::RemoveKeybinding => state.begin_capture(CaptureMode::Remove),
        KeyAction::ResetKeybindings => state.reset_keybindings(),
        _ => {}
    }
    Action::None
}

const PAGE: usize = 10;

/// The display text for a row's keys, or a placeholder when it has none —
/// an empty column reads as a rendering bug rather than as "unbound".
pub fn keys_display(row: &KeybindRow) -> String {
    if row.keys.is_empty() {
        "—".to_owned()
    } else {
        row.keys.join("  ")
    }
}

/// What a chord that is already taken belongs to, for the warning before it is
/// taken away.
pub fn current_owner(
    state: &AppState,
    context: KeyContext,
    binding: &Binding,
) -> Option<KeyAction> {
    state
        .keymap
        .entries(context)
        .iter()
        .find(|(b, _)| b == binding)
        .map(|(_, a)| *a)
}

pub fn describe(binding: &Binding) -> String {
    format_binding(binding)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    /// None of these touch the filesystem: the popup only writes on close, and
    /// nothing here closes it.
    fn editor() -> AppState {
        let mut state = AppState::new(Vec::new(), None);
        state.open_keybind_popup();
        state
    }

    fn press(state: &mut AppState, code: KeyCode, mods: KeyModifiers) -> Action {
        input::handle(state, KeyEvent::new(code, mods))
    }

    fn select(state: &mut AppState, context: KeyContext, action: KeyAction) {
        let index = state
            .keybind_rows
            .iter()
            .position(|r| r.context == context && r.action == action)
            .unwrap_or_else(|| panic!("{:?}/{:?} is not listed", context, action));
        state.select_keybind_row(index);
    }

    #[test]
    fn test_ctrl_g_opens_the_editor_from_a_list_pane() {
        let mut state = AppState::new(Vec::new(), None);
        state.active_pane = crate::app::ActivePane::History;
        press(&mut state, KeyCode::Char('g'), KeyModifiers::CONTROL);
        assert!(state.keybind_popup_open);
        assert!(!state.keybind_rows.is_empty());
    }

    #[test]
    fn test_rows_cover_every_context_that_has_bindings() {
        let state = editor();
        for &context in KeyContext::ALL {
            if crate::keymap::defaults::keymap()
                .entries(context)
                .is_empty()
            {
                continue;
            }
            assert!(
                state.keybind_rows.iter().any(|r| r.context == context),
                "{:?} is missing from the editor",
                context
            );
        }
    }

    #[test]
    fn test_typing_filters_the_list() {
        let mut state = editor();
        let all = state.keybind_rows.len();
        for c in "favorite".chars() {
            input::handle(
                &mut state,
                KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE),
            );
        }
        assert_eq!(state.keybind_query, "favorite");
        assert!(state.keybind_rows.len() < all);
        // Matches `toggle_favorite` and `view_favorites` alike — the filter is
        // a substring over the names, not a guess at which one was meant.
        assert!(
            state
                .keybind_rows
                .iter()
                .all(|r| r.action.as_str().contains("favorite"))
        );
        assert!(
            state
                .keybind_rows
                .iter()
                .any(|r| r.action == KeyAction::ToggleFavorite)
        );
    }

    /// Enter arms capture, and the very next press is taken raw — `v` must not
    /// be read as whatever `v` currently means.
    #[test]
    fn test_capture_binds_the_next_key_pressed() {
        let mut state = editor();
        select(&mut state, KeyContext::History, KeyAction::ToggleFavorite);
        press(&mut state, KeyCode::Enter, KeyModifiers::NONE);
        assert!(state.capturing.is_some());

        press(&mut state, KeyCode::Char('v'), KeyModifiers::NONE);
        assert!(state.capturing.is_none());
        assert_eq!(
            state
                .keymap
                .keys_for(KeyContext::History, KeyAction::ToggleFavorite),
            vec!["v"]
        );
    }

    /// A rebind replaces rather than adds, matching what the file does.
    #[test]
    fn test_capture_replaces_the_previous_keys() {
        let mut state = editor();
        select(&mut state, KeyContext::Global, KeyAction::ShowHelp);
        assert_eq!(
            state
                .keymap
                .keys_for(KeyContext::Global, KeyAction::ShowHelp),
            vec!["?", "F1"]
        );
        press(&mut state, KeyCode::Enter, KeyModifiers::NONE);
        press(&mut state, KeyCode::F(2), KeyModifiers::NONE);
        assert_eq!(
            state
                .keymap
                .keys_for(KeyContext::Global, KeyAction::ShowHelp),
            vec!["F2"]
        );
    }

    /// Taking an occupied key silently would lose whatever owned it, so the
    /// first press only warns.
    #[test]
    fn test_capturing_an_occupied_key_needs_a_second_press() {
        let mut state = editor();
        select(&mut state, KeyContext::History, KeyAction::ToggleFavorite);
        press(&mut state, KeyCode::Enter, KeyModifiers::NONE);

        press(&mut state, KeyCode::Char('d'), KeyModifiers::NONE);
        assert_eq!(
            state
                .keymap
                .keys_for(KeyContext::History, KeyAction::ToggleFavorite),
            vec!["f"],
            "the first press must change nothing"
        );
        assert!(state.capturing.is_some(), "still waiting");
        assert!(
            state
                .status_message
                .as_deref()
                .unwrap_or_default()
                .contains("toggle_details")
        );

        press(&mut state, KeyCode::Char('d'), KeyModifiers::NONE);
        assert_eq!(
            state
                .keymap
                .keys_for(KeyContext::History, KeyAction::ToggleFavorite),
            vec!["d"]
        );
        assert!(
            state
                .keymap
                .keys_for(KeyContext::History, KeyAction::ToggleDetails)
                .is_empty(),
            "the old owner is evicted, or the new binding would be shadowed"
        );
    }

    #[test]
    fn test_ctrl_a_adds_a_key_and_keeps_the_existing_ones() {
        let mut state = editor();
        select(&mut state, KeyContext::Global, KeyAction::ShowHelp);
        press(&mut state, KeyCode::Char('a'), KeyModifiers::CONTROL);
        press(&mut state, KeyCode::F(2), KeyModifiers::NONE);
        assert_eq!(
            state
                .keymap
                .keys_for(KeyContext::Global, KeyAction::ShowHelp),
            vec!["?", "F1", "F2"],
            "adding appends rather than replacing"
        );
    }

    /// The distinction that made this worth adding: Enter still replaces.
    #[test]
    fn test_enter_still_replaces_where_ctrl_a_appends() {
        let mut state = editor();
        select(&mut state, KeyContext::Global, KeyAction::ShowHelp);
        press(&mut state, KeyCode::Enter, KeyModifiers::NONE);
        press(&mut state, KeyCode::F(2), KeyModifiers::NONE);
        assert_eq!(
            state
                .keymap
                .keys_for(KeyContext::Global, KeyAction::ShowHelp),
            vec!["F2"]
        );
    }

    #[test]
    fn test_adding_a_key_the_action_already_has_changes_nothing() {
        let mut state = editor();
        select(&mut state, KeyContext::Global, KeyAction::ShowHelp);
        press(&mut state, KeyCode::Char('a'), KeyModifiers::CONTROL);
        press(&mut state, KeyCode::Char('?'), KeyModifiers::NONE);
        assert_eq!(
            state
                .keymap
                .keys_for(KeyContext::Global, KeyAction::ShowHelp),
            vec!["?", "F1"]
        );
        assert!(state.capturing.is_none(), "and it stops asking");
    }

    /// Removing names the key by pressing it, so there is no second selection
    /// to build for a row that holds several.
    #[test]
    fn test_ctrl_d_removes_only_the_key_pressed() {
        let mut state = editor();
        select(&mut state, KeyContext::Global, KeyAction::ShowHelp);
        press(&mut state, KeyCode::Char('d'), KeyModifiers::CONTROL);
        press(&mut state, KeyCode::F(1), KeyModifiers::NONE);
        assert_eq!(
            state
                .keymap
                .keys_for(KeyContext::Global, KeyAction::ShowHelp),
            vec!["?"]
        );
    }

    #[test]
    fn test_removing_the_last_key_leaves_the_action_unbound() {
        let mut state = editor();
        select(&mut state, KeyContext::History, KeyAction::ToggleFavorite);
        press(&mut state, KeyCode::Char('d'), KeyModifiers::CONTROL);
        press(&mut state, KeyCode::Char('f'), KeyModifiers::NONE);
        assert!(
            state
                .keymap
                .keys_for(KeyContext::History, KeyAction::ToggleFavorite)
                .is_empty()
        );
        // Still listed, so a key can be put back.
        assert!(
            state
                .keybind_rows
                .iter()
                .any(|r| r.context == KeyContext::History && r.action == KeyAction::ToggleFavorite)
        );
    }

    /// Pressing a key the row does not hold must not remove someone else's.
    #[test]
    fn test_removing_a_key_the_row_does_not_have_is_refused() {
        let mut state = editor();
        select(&mut state, KeyContext::History, KeyAction::ToggleFavorite);
        press(&mut state, KeyCode::Char('d'), KeyModifiers::CONTROL);
        press(&mut state, KeyCode::Char('y'), KeyModifiers::NONE);
        assert_eq!(
            state
                .keymap
                .keys_for(KeyContext::History, KeyAction::ToggleFavorite),
            vec!["f"]
        );
        assert_eq!(
            state
                .keymap
                .keys_for(KeyContext::History, KeyAction::CopyToClipboard),
            vec!["y"],
            "the key's real owner is untouched"
        );
    }

    #[test]
    fn test_remove_is_refused_on_a_row_with_no_keys() {
        let mut state = editor();
        select(&mut state, KeyContext::History, KeyAction::ToggleFavorite);
        press(&mut state, KeyCode::Char('d'), KeyModifiers::CONTROL);
        press(&mut state, KeyCode::Char('f'), KeyModifiers::NONE);
        press(&mut state, KeyCode::Char('d'), KeyModifiers::CONTROL);
        assert!(state.capturing.is_none(), "nothing left to remove");
    }

    /// Adding onto an occupied key goes through the same two-press
    /// confirmation as replacing does.
    #[test]
    fn test_adding_an_occupied_key_needs_a_second_press() {
        let mut state = editor();
        select(&mut state, KeyContext::History, KeyAction::ToggleFavorite);
        press(&mut state, KeyCode::Char('a'), KeyModifiers::CONTROL);
        press(&mut state, KeyCode::Char('y'), KeyModifiers::NONE);
        assert_eq!(
            state
                .keymap
                .keys_for(KeyContext::History, KeyAction::ToggleFavorite),
            vec!["f"]
        );
        press(&mut state, KeyCode::Char('y'), KeyModifiers::NONE);
        assert_eq!(
            state
                .keymap
                .keys_for(KeyContext::History, KeyAction::ToggleFavorite),
            vec!["f", "y"]
        );
        assert!(
            state
                .keymap
                .keys_for(KeyContext::History, KeyAction::CopyToClipboard)
                .is_empty()
        );
    }

    /// A different second key is a fresh attempt, not a confirmation.
    #[test]
    fn test_a_different_key_restarts_the_conflict_prompt() {
        let mut state = editor();
        select(&mut state, KeyContext::History, KeyAction::ToggleFavorite);
        press(&mut state, KeyCode::Enter, KeyModifiers::NONE);
        press(&mut state, KeyCode::Char('d'), KeyModifiers::NONE);
        press(&mut state, KeyCode::Char('t'), KeyModifiers::NONE);
        assert_eq!(
            state
                .keymap
                .keys_for(KeyContext::History, KeyAction::ToggleFavorite),
            vec!["f"],
            "neither key was taken"
        );
        assert!(state.capturing.is_some());
    }

    #[test]
    fn test_escape_cancels_capture_without_binding_anything() {
        let mut state = editor();
        select(&mut state, KeyContext::History, KeyAction::ToggleFavorite);
        press(&mut state, KeyCode::Enter, KeyModifiers::NONE);
        press(&mut state, KeyCode::Esc, KeyModifiers::NONE);
        assert!(state.capturing.is_none());
        assert!(
            state.keybind_popup_open,
            "cancelling capture is not closing"
        );
        assert_eq!(
            state
                .keymap
                .keys_for(KeyContext::History, KeyAction::ToggleFavorite),
            vec!["f"]
        );
    }

    /// Ctrl+C would otherwise resolve to Cancel and close the popup; while
    /// capturing it is just a key like any other.
    #[test]
    fn test_capture_takes_keys_that_normally_mean_something_else() {
        let mut state = editor();
        select(&mut state, KeyContext::History, KeyAction::ToggleFavorite);
        press(&mut state, KeyCode::Enter, KeyModifiers::NONE);
        press(&mut state, KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert!(state.keybind_popup_open, "Ctrl+C did not close the popup");
        assert_eq!(
            state
                .keymap
                .keys_for(KeyContext::History, KeyAction::ToggleFavorite),
            vec!["Ctrl+c"]
        );
    }

    /// A single press cannot express `g g`, so that row says so rather than
    /// letting Enter look broken.
    #[test]
    fn test_sequence_rows_are_not_editable() {
        let mut state = editor();
        select(&mut state, KeyContext::Global, KeyAction::GoToTop);
        assert!(!state.selected_keybind_row().unwrap().editable);
        press(&mut state, KeyCode::Enter, KeyModifiers::NONE);
        assert!(state.capturing.is_none(), "capture must not arm");
    }

    #[test]
    fn test_moving_off_a_row_abandons_its_capture() {
        let mut state = editor();
        select(&mut state, KeyContext::History, KeyAction::ToggleFavorite);
        press(&mut state, KeyCode::Enter, KeyModifiers::NONE);
        press(&mut state, KeyCode::Down, KeyModifiers::NONE);
        assert!(state.capturing.is_none());
    }

    #[test]
    fn test_reset_restores_the_defaults() {
        let mut state = editor();
        select(&mut state, KeyContext::History, KeyAction::ToggleFavorite);
        press(&mut state, KeyCode::Enter, KeyModifiers::NONE);
        press(&mut state, KeyCode::Char('v'), KeyModifiers::NONE);
        press(&mut state, KeyCode::Char('r'), KeyModifiers::CONTROL);
        assert_eq!(
            state
                .keymap
                .keys_for(KeyContext::History, KeyAction::ToggleFavorite),
            vec!["f"]
        );
    }

    /// The row list is what the popup renders, so it has to follow a rebind
    /// without waiting for a reopen.
    #[test]
    fn test_the_visible_row_updates_after_a_rebind() {
        let mut state = editor();
        select(&mut state, KeyContext::History, KeyAction::ToggleFavorite);
        press(&mut state, KeyCode::Enter, KeyModifiers::NONE);
        press(&mut state, KeyCode::Char('v'), KeyModifiers::NONE);
        let row = state
            .keybind_rows
            .iter()
            .find(|r| r.context == KeyContext::History && r.action == KeyAction::ToggleFavorite)
            .unwrap();
        assert_eq!(row.keys, vec!["v"]);
    }

    #[test]
    fn test_keys_display_marks_an_unbound_action() {
        let row = KeybindRow {
            context: KeyContext::History,
            action: KeyAction::ToggleFavorite,
            keys: vec![],
            editable: true,
        };
        assert_eq!(keys_display(&row), "—");
    }

    #[test]
    fn test_filter_matches_keys_as_well_as_names() {
        let rows = rows(&editor());
        let by_key = filter(&rows, "ctrl+t");
        assert!(by_key.iter().any(|r| r.action == KeyAction::ChangeTheme));
        assert!(filter(&rows, "no-such-thing").is_empty());
    }
}

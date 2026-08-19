pub mod collection;
pub mod edit;
pub mod help;
pub mod import_export;
pub mod mouse;
pub mod normal;
pub mod tag;

use crate::app::{Action, ActivePane, AppState, InputMode};
use crate::keymap::{KeyAction, KeyContext, Resolved};
use crossterm::event::KeyEvent;

/// The pane a key would land in with no overlay open. Also what the help popup
/// describes: while it is open `active_context` reports `Help`, but the user is
/// asking about the view behind it.
pub fn pane_context(state: &AppState) -> KeyContext {
    match state.active_pane {
        ActivePane::Search => KeyContext::Search,
        ActivePane::History => KeyContext::History,
        ActivePane::CollectionsList => KeyContext::CollectionsList,
        ActivePane::CollectionItems => KeyContext::CollectionItems,
    }
}

/// Overlay priority, in one place. This used to be three hand-maintained
/// copies — the guard chain here, the Esc condition in `main.rs`, and
/// `mouse::popup_open` — which is exactly the kind of list that drifts.
pub fn active_context(state: &AppState) -> KeyContext {
    // Checked first: it is offered on startup, before the user has done
    // anything else.
    if state.integration_popup_open {
        return KeyContext::IntegrationPopup;
    }
    // The right-click menu is modal too: it takes the keys before any pane.
    if state.context_menu_open {
        return KeyContext::ContextMenu;
    }
    if state.theme_popup_open {
        return KeyContext::ThemePopup;
    }
    if state.help_open {
        return KeyContext::Help;
    }
    if state.export_popup_open || state.import_popup_open {
        return KeyContext::ImportExport;
    }
    match state.input_mode {
        InputMode::TagInput => KeyContext::TagInput,
        InputMode::CollectionInput => KeyContext::CollectionInput,
        InputMode::EditCommand => KeyContext::EditCommand,
        // Unreachable: this mode is only set alongside the import/export
        // popup, which the guard above already claimed.
        InputMode::ImportExport => KeyContext::ImportExport,
        InputMode::Normal => pane_context(state),
    }
}

pub fn handle(state: &mut AppState, key: KeyEvent) -> Action {
    let context = active_context(state);
    state.check_key_buffer_timeout();
    let pending = state.key_buffer;

    match state.keymap.clone().resolve(context, pending, &key) {
        Resolved::Pending => {
            state.set_key_buffer(crate::keymap::KeyChord::new(key.code, key.modifiers));
            Action::None
        }
        Resolved::Action(ctx, action) => {
            state.clear_key_buffer();
            dispatch(state, ctx, action)
        }
        Resolved::Text(ctx, c) => {
            state.clear_key_buffer();
            insert_char(state, ctx, c);
            Action::None
        }
        Resolved::Unbound => {
            state.clear_key_buffer();
            Action::None
        }
    }
}

/// Runs `action` as if it had been pressed in the pane behind any overlay.
/// The help popup uses this so a shortcut it lists cannot behave differently
/// from its keybinding.
pub fn dispatch_in_pane(state: &mut AppState, action: KeyAction) -> Action {
    let context = pane_context(state);
    dispatch(state, context, action)
}

pub fn dispatch(state: &mut AppState, context: KeyContext, action: KeyAction) -> Action {
    // Cancel means the same thing everywhere, so it is resolved before the
    // per-context handlers rather than in each of them: `cancel_or_quit`
    // already knows the staging order.
    if action == KeyAction::Cancel {
        return if state.cancel_or_quit() {
            Action::Exit
        } else {
            Action::None
        };
    }
    match context {
        KeyContext::IntegrationPopup => integration_popup(state, action),
        KeyContext::ContextMenu => context_menu(state, action),
        KeyContext::ThemePopup => theme_popup(state, action),
        KeyContext::Help => help::dispatch(state, action),
        KeyContext::ImportExport => import_export::dispatch(state, action),
        KeyContext::TagInput => tag::dispatch(state, action),
        KeyContext::CollectionInput => collection::dispatch(state, action),
        KeyContext::EditCommand => edit::dispatch(state, action),
        KeyContext::Global
        | KeyContext::Search
        | KeyContext::History
        | KeyContext::CollectionsList
        | KeyContext::CollectionItems => normal::dispatch(state, context, action),
    }
}

fn insert_char(state: &mut AppState, context: KeyContext, c: char) {
    match context {
        KeyContext::Search => normal::insert_char(state, c),
        KeyContext::Help => help::insert_char(state, c),
        KeyContext::TagInput => tag::insert_char(state, c),
        KeyContext::CollectionInput => collection::insert_char(state, c),
        KeyContext::ImportExport => import_export::insert_char(state, c),
        KeyContext::EditCommand => edit::insert_char(state, c),
        _ => {}
    }
}

fn integration_popup(state: &mut AppState, action: KeyAction) -> Action {
    // After a write the popup is a result view; nothing left to confirm.
    if state.integration_installed {
        state.integration_popup_open = false;
        return Action::None;
    }
    match action {
        KeyAction::Confirm => {
            // A reload only comes back when ctrlr can reach the prompt line
            // through --output-file; otherwise the popup reports the result and
            // the user restarts the shell themselves.
            match state.install_integration() {
                Some(reload) => Action::Execute(reload),
                None => Action::None,
            }
        }
        KeyAction::Decline => {
            state.dismiss_integration_popup();
            Action::None
        }
        _ => Action::None,
    }
}

fn context_menu(state: &mut AppState, action: KeyAction) -> Action {
    match action {
        KeyAction::NavigateDown => state.navigate_context_menu(1),
        KeyAction::NavigateUp => state.navigate_context_menu(-1),
        KeyAction::Confirm => return mouse::activate_context_menu(state),
        _ => {}
    }
    Action::None
}

fn theme_popup(state: &mut AppState, action: KeyAction) -> Action {
    match action {
        KeyAction::NavigateDown => state.navigate_theme_popup_down(),
        KeyAction::NavigateUp => state.navigate_theme_popup_up(),
        KeyAction::Confirm => state.apply_theme_and_close(),
        _ => {}
    }
    Action::None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{Command, InputMode};
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    fn cmd(text: &str) -> Command {
        Command {
            id: crate::hash::hash_command(text),
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

    fn state() -> AppState {
        AppState::new(vec![cmd("git status"), cmd("cargo test")], None)
    }

    fn esc() -> KeyEvent {
        KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)
    }

    fn ctrl_c() -> KeyEvent {
        KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)
    }

    /// Both cancel keys have to resolve to the same action in every context,
    /// which is what the two defaults promise.
    #[test]
    fn test_esc_and_ctrl_c_resolve_to_cancel_everywhere() {
        let state = state();
        for &context in crate::keymap::KeyContext::ALL {
            for key in [esc(), ctrl_c()] {
                // The owning context may be `Global` for a pane — what matters
                // is that both keys back out, and identically. The startup
                // offer is the one that answers with `Decline` instead:
                // dismissing it records the decline.
                let resolved = state.keymap.resolve(context, None, &key);
                assert!(
                    matches!(
                        resolved,
                        Resolved::Action(_, KeyAction::Cancel | KeyAction::Decline)
                    ),
                    "{:?} does not back out of {:?}, it resolved to {:?}",
                    key.code,
                    context,
                    resolved
                );
                assert_eq!(
                    resolved,
                    state.keymap.resolve(context, None, &esc()),
                    "Ctrl+C and Esc disagree on {:?}",
                    context
                );
            }
        }
    }

    /// Both keys must agree in every state, which is the whole point of routing
    /// them through one `cancel_or_quit`.
    fn assert_both_keys<F, G>(setup: F, check: G)
    where
        F: Fn(&mut AppState),
        G: Fn(&AppState, &Action),
    {
        for key in [esc(), ctrl_c()] {
            let mut state = state();
            setup(&mut state);
            let action = handle(&mut state, key);
            check(&state, &action);
        }
    }

    #[test]
    fn test_quit_key_on_clean_state_exits() {
        assert_both_keys(|_| {}, |_, action| assert_eq!(*action, Action::Exit));
    }

    #[test]
    fn test_quit_key_clears_search_before_exiting() {
        assert_both_keys(
            |state| {
                state.search_query = "git".into();
                state.filter_commands();
            },
            |state, action| {
                assert_eq!(*action, Action::None);
                assert!(state.search_query.is_empty());
            },
        );
    }

    #[test]
    fn test_quit_key_closes_help_first() {
        assert_both_keys(
            |state| {
                state.help_open = true;
                state.help_search_query = "fav".into();
            },
            |state, action| {
                assert_eq!(*action, Action::None);
                assert!(!state.help_open);
                assert!(state.help_search_query.is_empty());
            },
        );
    }

    #[test]
    fn test_quit_key_closes_theme_popup_first() {
        assert_both_keys(
            |state| state.theme_popup_open = true,
            |state, action| {
                assert_eq!(*action, Action::None);
                assert!(!state.theme_popup_open);
            },
        );
    }

    #[test]
    fn test_quit_key_closes_context_menu_first() {
        assert_both_keys(
            |state| state.context_menu_open = true,
            |state, action| {
                assert_eq!(*action, Action::None);
                assert!(!state.context_menu_open);
            },
        );
    }

    #[test]
    fn test_quit_key_closes_export_and_import_popups_first() {
        for open in ["export", "import"] {
            assert_both_keys(
                |state| {
                    if open == "export" {
                        state.export_popup_open = true;
                    } else {
                        state.import_popup_open = true;
                    }
                },
                |state, action| {
                    assert_eq!(*action, Action::None);
                    assert!(!state.export_popup_open);
                    assert!(!state.import_popup_open);
                },
            );
        }
    }

    #[test]
    fn test_quit_key_dismisses_integration_popup_first() {
        assert_both_keys(
            |state| state.integration_popup_open = true,
            |state, action| {
                assert_eq!(*action, Action::None);
                assert!(!state.integration_popup_open);
            },
        );
    }

    #[test]
    fn test_quit_key_leaves_tag_input_before_exiting() {
        assert_both_keys(
            |state| {
                state.input_mode = InputMode::TagInput;
                state.tag_input = "wip".into();
                state.tag_cursor_index = Some(0);
            },
            |state, action| {
                assert_eq!(*action, Action::None);
                assert_eq!(state.input_mode, InputMode::Normal);
                assert!(state.tag_input.is_empty());
                assert_eq!(state.tag_cursor_index, None);
            },
        );
    }

    #[test]
    fn test_quit_key_leaves_collection_input_before_exiting() {
        assert_both_keys(
            |state| {
                state.input_mode = InputMode::CollectionInput;
                state.collection_input_mode = crate::app::CollectionInputMode::NewCollection;
                state.collection_input_text = "work".into();
                state.editing_collection_id = Some("abc".into());
            },
            |state, action| {
                assert_eq!(*action, Action::None);
                assert_eq!(state.input_mode, InputMode::Normal);
                assert!(state.collection_input_text.is_empty());
                assert_eq!(state.editing_collection_id, None);
                assert_eq!(
                    state.collection_input_mode,
                    crate::app::CollectionInputMode::None
                );
            },
        );
    }

    /// The staging rule: as long as something is open, cancelling never quits.
    #[test]
    fn test_cancel_never_quits_while_something_is_open() {
        type Opener = (&'static str, fn(&mut AppState));
        let openers: Vec<Opener> = vec![
            ("integration", |s| s.integration_popup_open = true),
            ("context menu", |s| s.context_menu_open = true),
            ("theme", |s| s.theme_popup_open = true),
            ("help", |s| s.help_open = true),
            ("export", |s| s.export_popup_open = true),
            ("import", |s| s.import_popup_open = true),
            ("tag input", |s| s.input_mode = InputMode::TagInput),
            ("collection input", |s| {
                s.input_mode = InputMode::CollectionInput
            }),
        ];
        for (name, open) in openers {
            let mut state = state();
            open(&mut state);
            assert!(
                !state.cancel_or_quit(),
                "cancelling with {} open must not quit",
                name
            );
        }
    }

    /// The characterisation table for the chain: pane context first, then
    /// `Global`, with the search bar absorbing text in between. This is what
    /// the old three sequential match blocks in `normal.rs` achieved through
    /// per-arm `active_pane != ActivePane::Search` guards, and it is the piece
    /// most likely to break silently.
    #[test]
    fn test_chain_resolution_table() {
        use crate::keymap::KeyContext as C;
        let km = state().keymap;
        let n = KeyModifiers::NONE;
        let ctrl = KeyModifiers::CONTROL;

        let table: &[(C, KeyCode, KeyModifiers, Resolved)] = &[
            // Letters and digits type in the search bar rather than firing the
            // global action they own elsewhere.
            (
                C::Search,
                KeyCode::Char('1'),
                n,
                Resolved::Text(C::Search, '1'),
            ),
            (
                C::Search,
                KeyCode::Char('?'),
                n,
                Resolved::Text(C::Search, '?'),
            ),
            (
                C::Search,
                KeyCode::Char('<'),
                n,
                Resolved::Text(C::Search, '<'),
            ),
            (
                C::Search,
                KeyCode::Char('.'),
                n,
                Resolved::Text(C::Search, '.'),
            ),
            (
                C::Search,
                KeyCode::Char('c'),
                n,
                Resolved::Text(C::Search, 'c'),
            ),
            (
                C::Search,
                KeyCode::Char('g'),
                n,
                Resolved::Text(C::Search, 'g'),
            ),
            (
                C::Search,
                KeyCode::Char('G'),
                n,
                Resolved::Text(C::Search, 'G'),
            ),
            // ... and the same keys do fire from a list pane.
            (
                C::History,
                KeyCode::Char('1'),
                n,
                Resolved::Action(C::Global, KeyAction::ViewHistory),
            ),
            (
                C::History,
                KeyCode::Char('?'),
                n,
                Resolved::Action(C::Global, KeyAction::ShowHelp),
            ),
            (
                C::History,
                KeyCode::Char('<'),
                n,
                Resolved::Action(C::Global, KeyAction::ShrinkPane),
            ),
            (
                C::History,
                KeyCode::Char('.'),
                n,
                Resolved::Action(C::Global, KeyAction::ScopeCwd),
            ),
            (
                C::History,
                KeyCode::Char('c'),
                n,
                Resolved::Action(C::Global, KeyAction::AddToCollection),
            ),
            (
                C::History,
                KeyCode::Char('G'),
                n,
                Resolved::Action(C::Global, KeyAction::GoToBottom),
            ),
            // Non-character keys are never absorbed, so they reach Global even
            // from the search bar.
            (
                C::Search,
                KeyCode::Enter,
                n,
                Resolved::Action(C::Global, KeyAction::Execute),
            ),
            (
                C::Search,
                KeyCode::Tab,
                n,
                Resolved::Action(C::Global, KeyAction::SwitchPane),
            ),
            (
                C::Search,
                KeyCode::Down,
                n,
                Resolved::Action(C::Global, KeyAction::NavigateDown),
            ),
            (
                C::Search,
                KeyCode::F(1),
                n,
                Resolved::Action(C::Global, KeyAction::ShowHelp),
            ),
            // Ctrl+u clears in the search bar and pages everywhere else.
            (
                C::Search,
                KeyCode::Char('u'),
                ctrl,
                Resolved::Action(C::Search, KeyAction::ClearSearch),
            ),
            (
                C::History,
                KeyCode::Char('u'),
                ctrl,
                Resolved::Action(C::Global, KeyAction::PageUp),
            ),
            // The same letter, three meanings, decided by the pane.
            (
                C::History,
                KeyCode::Char('d'),
                n,
                Resolved::Action(C::History, KeyAction::ToggleDetails),
            ),
            (
                C::CollectionItems,
                KeyCode::Char('d'),
                n,
                Resolved::Action(C::CollectionItems, KeyAction::ToggleDetails),
            ),
            (
                C::CollectionsList,
                KeyCode::Char('d'),
                n,
                Resolved::Action(C::CollectionsList, KeyAction::DeleteCollection),
            ),
            (
                C::History,
                KeyCode::Char('e'),
                n,
                Resolved::Action(C::History, KeyAction::EditCommand),
            ),
            (
                C::CollectionsList,
                KeyCode::Char('e'),
                n,
                Resolved::Action(C::CollectionsList, KeyAction::EditCollection),
            ),
            // Backspace walks back out of a query from a list pane.
            (
                C::History,
                KeyCode::Backspace,
                n,
                Resolved::Action(C::History, KeyAction::DeleteCharBackward),
            ),
            (
                C::Search,
                KeyCode::Backspace,
                n,
                Resolved::Action(C::Search, KeyAction::DeleteCharBackward),
            ),
            // Overlays are exclusive: Ctrl+T must not reach the theme picker
            // from behind the help popup.
            (C::Help, KeyCode::Char('t'), ctrl, Resolved::Unbound),
            (C::Help, KeyCode::Char('t'), n, Resolved::Text(C::Help, 't')),
            (
                C::TagInput,
                KeyCode::Tab,
                n,
                Resolved::Action(C::TagInput, KeyAction::AcceptSuggestion),
            ),
            // Nothing at all.
            (C::History, KeyCode::Char('z'), n, Resolved::Unbound),
        ];

        for (context, code, mods, expected) in table {
            let key = KeyEvent::new(*code, *mods);
            assert_eq!(
                km.resolve(*context, None, &key),
                *expected,
                "{:?} + {:?} in {:?}",
                code,
                mods,
                context
            );
        }
    }

    /// `gg` needs both halves and a pending buffer; a lone `g` does nothing.
    #[test]
    fn test_the_gg_chord_needs_both_halves() {
        use crate::keymap::{KeyChord, KeyContext as C};
        let km = state().keymap;
        let g = KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE);
        let pending = KeyChord::new(g.code, g.modifiers);

        assert_eq!(km.resolve(C::History, None, &g), Resolved::Pending);
        assert_eq!(
            km.resolve(C::History, Some(pending), &g),
            Resolved::Action(C::Global, KeyAction::GoToTop)
        );
        // A different second key drops the chord rather than firing it.
        let z = KeyEvent::new(KeyCode::Char('z'), KeyModifiers::NONE);
        assert_eq!(km.resolve(C::History, Some(pending), &z), Resolved::Unbound);
    }

    /// Two overlays at once: the topmost closes and the other survives, so a
    /// single keypress never tears down more than one layer.
    #[test]
    fn test_cancel_closes_one_layer_at_a_time() {
        let mut state = state();
        state.theme_popup_open = true;
        state.help_open = true;
        assert!(!state.cancel_or_quit());
        assert!(!state.theme_popup_open, "theme is checked before help");
        assert!(state.help_open, "help must survive the first cancel");
        assert!(!state.cancel_or_quit());
        assert!(!state.help_open);
    }
}

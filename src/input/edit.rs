//! Editing the selected command before it is handed to the shell.
//!
//! The shell's `fc` opens the last command in an editor; this is the same idea
//! against any row in the list. Enter emits whatever is in the buffer — the
//! stored command is left alone, and no DB row is written for the edited text
//! (see [`AppState::commit_edit`]).

use crate::app::{Action, AppState};
use crate::keymap::KeyAction;

pub fn insert_char(state: &mut AppState, c: char) {
    state.edit_input.insert(c);
}

pub fn dispatch(state: &mut AppState, action: KeyAction) -> Action {
    match action {
        KeyAction::Confirm => {
            // `None` means the line trimmed to nothing. Stay in edit mode
            // rather than emitting an empty command, which the shell widgets
            // would read as "cancelled".
            if let Some(cmd) = state.commit_edit() {
                return Action::Execute(cmd);
            }
        }
        // The same key that opened the line takes it one step further out, to
        // a real editor. readline spells this Ctrl+X Ctrl+E; there is no chord
        // machinery here, and Ctrl+X is unambiguous inside this mode.
        KeyAction::OpenExternalEditor => {
            return Action::OpenEditor(state.edit_input.value().to_owned());
        }
        KeyAction::CursorLeft => state.edit_input.left(),
        KeyAction::CursorRight => state.edit_input.right(),
        KeyAction::CursorHome => state.edit_input.home(),
        KeyAction::CursorEnd => state.edit_input.end(),
        KeyAction::DeleteCharBackward => state.edit_input.backspace(),
        KeyAction::DeleteCharForward => state.edit_input.delete(),
        KeyAction::KillLine => state.edit_input.kill_to_start(),
        _ => {}
    }
    Action::None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{Command, InputMode};
    use crate::input;
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

    fn editing(text: &str) -> AppState {
        let mut state = AppState::new(vec![cmd(text)], None);
        state.begin_edit_command();
        state
    }

    fn plain(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE)
    }

    /// Deliberately through `input::handle` rather than `dispatch`: that is
    /// what pins the keymap entry as well as the behaviour.
    fn press(state: &mut AppState, code: KeyCode) -> Action {
        input::handle(state, KeyEvent::new(code, KeyModifiers::NONE))
    }

    #[test]
    fn test_begin_edit_loads_the_selection_with_the_cursor_at_the_end() {
        let state = editing("git status");
        assert_eq!(state.input_mode, InputMode::EditCommand);
        assert_eq!(state.edit_input.value(), "git status");
        assert_eq!(state.edit_input.cursor(), 10);
        assert_eq!(state.edit_origin.as_deref(), Some("git status"));
    }

    /// Entering the mode with nothing selected would render an empty box that
    /// Enter could not resolve.
    #[test]
    fn test_begin_edit_on_an_empty_list_is_a_no_op() {
        let mut state = AppState::new(vec![], None);
        state.begin_edit_command();
        assert_eq!(state.input_mode, InputMode::Normal);
        assert_eq!(state.edit_origin, None);
    }

    #[test]
    fn test_typing_edits_the_line_and_enter_emits_it() {
        let mut state = editing("git status");
        for c in " -sb".chars() {
            input::handle(&mut state, plain(c));
        }
        let action = press(&mut state, KeyCode::Enter);
        assert_eq!(action, Action::Execute("git status -sb".into()));
        assert_eq!(state.input_mode, InputMode::Normal);
    }

    #[test]
    fn test_cursor_keys_move_within_the_line() {
        let mut state = editing("git status");
        press(&mut state, KeyCode::Home);
        input::handle(&mut state, plain('s'));
        input::handle(&mut state, plain('u'));
        press(&mut state, KeyCode::End);
        input::handle(&mut state, plain('!'));
        assert_eq!(state.edit_input.value(), "sugit status!");

        press(&mut state, KeyCode::Left);
        press(&mut state, KeyCode::Backspace);
        assert_eq!(state.edit_input.value(), "sugit statu!");

        press(&mut state, KeyCode::Left);
        press(&mut state, KeyCode::Delete);
        assert_eq!(state.edit_input.value(), "sugit stat!");
    }

    #[test]
    fn test_ctrl_u_clears_to_the_start() {
        let mut state = editing("sudo rm -rf /");
        input::handle(
            &mut state,
            KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL),
        );
        assert!(state.edit_input.is_empty());
    }

    /// An empty output file is how `run_tui` signals "cancelled" to the shell
    /// widgets, so an emptied line must not be emitted as a command.
    #[test]
    fn test_enter_on_an_empty_line_stays_in_edit_mode() {
        let mut state = editing("ls");
        input::handle(
            &mut state,
            KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL),
        );
        let action = press(&mut state, KeyCode::Enter);
        assert_eq!(action, Action::None);
        assert_eq!(state.input_mode, InputMode::EditCommand);
    }

    #[test]
    fn test_enter_on_a_whitespace_only_line_stays_in_edit_mode() {
        let mut state = editing("ls");
        state.edit_input.set_value("   ");
        let action = press(&mut state, KeyCode::Enter);
        assert_eq!(action, Action::None);
        assert_eq!(state.input_mode, InputMode::EditCommand);
    }

    /// Cancelling routes through the shared quit key, so this also pins that
    /// `cancel_or_quit` reaches the edit mode rather than exiting ctrlr.
    #[test]
    fn test_escape_and_ctrl_c_discard_the_edit() {
        for key in [
            KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
        ] {
            let mut state = editing("git status");
            input::handle(&mut state, plain('!'));
            let action = input::handle(&mut state, key);
            assert_eq!(action, Action::None, "cancelling an edit must not quit");
            assert_eq!(state.input_mode, InputMode::Normal);
            assert!(state.edit_input.is_empty());
            assert_eq!(state.edit_origin, None);
            assert_eq!(
                state.commands[0].text, "git status",
                "the stored command is untouched"
            );
        }
    }

    /// The rule that keeps the DB honest: the original did not run, so its
    /// count must not move and no row is written for the edited text. The
    /// variant reappears on its own once the shell logs it to `runs.log`.
    #[test]
    fn test_committing_a_changed_line_leaves_the_stored_command_alone() {
        let mut state = editing("git status");
        input::handle(&mut state, plain('!'));
        let action = press(&mut state, KeyCode::Enter);
        assert_eq!(action, Action::Execute("git status!".into()));
        assert_eq!(state.commands.len(), 1);
        assert_eq!(state.commands[0].text, "git status");
        assert_eq!(state.commands[0].use_count, 0);
    }

    /// An unchanged line is Enter with extra steps, so it counts like Enter.
    #[test]
    fn test_committing_an_unchanged_line_counts_as_a_run() {
        let mut state = editing("git status");
        let action = press(&mut state, KeyCode::Enter);
        assert_eq!(action, Action::Execute("git status".into()));
        assert_eq!(state.commands[0].use_count, 1);
    }

    #[test]
    fn test_ctrl_x_hands_the_current_buffer_to_the_editor() {
        let mut state = editing("git status");
        input::handle(&mut state, plain('!'));
        let action = input::handle(
            &mut state,
            KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL),
        );
        assert_eq!(action, Action::OpenEditor("git status!".into()));
        assert_eq!(
            state.input_mode,
            InputMode::EditCommand,
            "the mode survives the handoff so the result can be confirmed"
        );
    }

    #[test]
    fn test_e_opens_the_edit_line_from_the_history_pane() {
        let mut state = AppState::new(vec![cmd("git status")], None);
        state.active_pane = crate::app::ActivePane::History;
        input::handle(&mut state, plain('e'));
        assert_eq!(state.input_mode, InputMode::EditCommand);
    }

    /// `e` types in the search bar, which is why Ctrl+x exists as the global
    /// way in.
    #[test]
    fn test_e_types_in_the_search_bar_but_ctrl_x_still_edits() {
        let mut state = AppState::new(vec![cmd("git status")], None);
        assert_eq!(state.active_pane, crate::app::ActivePane::Search);
        input::handle(&mut state, plain('e'));
        assert_eq!(state.input_mode, InputMode::Normal);
        assert_eq!(state.search_query, "e");

        state.search_query.clear();
        state.filter_commands();
        input::handle(
            &mut state,
            KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL),
        );
        assert_eq!(state.input_mode, InputMode::EditCommand);
    }
}

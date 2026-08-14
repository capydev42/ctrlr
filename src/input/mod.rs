pub mod collection;
pub mod help;
pub mod import_export;
pub mod mouse;
pub mod normal;
pub mod tag;

use crate::app::{Action, AppState, InputMode};
use crossterm::event::{KeyCode, KeyEvent};

pub fn handle(state: &mut AppState, key: KeyEvent) -> Action {
    // Checked first: it is offered on startup, before the user has done
    // anything else.
    if state.integration_popup_open {
        return handle_integration_popup(state, key);
    }
    // The right-click menu is modal too: it takes the keys before any pane.
    if state.context_menu_open {
        return handle_context_menu(state, key);
    }
    if state.theme_popup_open {
        return handle_theme_popup(state, key);
    }
    if state.help_open {
        return help::handle(state, key);
    }
    if state.export_popup_open || state.import_popup_open {
        return import_export::handle(state, key);
    }
    match state.input_mode {
        InputMode::TagInput => tag::handle(state, key),
        InputMode::CollectionInput => collection::handle(state, key),
        InputMode::ImportExport => Action::None,
        InputMode::Normal => normal::handle(state, key),
    }
}

fn handle_integration_popup(state: &mut AppState, key: KeyEvent) -> Action {
    // After a write the popup is a result view; nothing left to confirm.
    if state.integration_installed {
        state.integration_popup_open = false;
        return Action::None;
    }

    match key.code {
        KeyCode::Enter | KeyCode::Char('u') | KeyCode::Char('y') => {
            // A reload only comes back when ctrlr can reach the prompt line
            // through --output-file; otherwise the popup reports the result and
            // the user restarts the shell themselves.
            match state.install_integration() {
                Some(reload) => Action::Execute(reload),
                None => Action::None,
            }
        }
        KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('q') => {
            state.dismiss_integration_popup();
            Action::None
        }
        _ => Action::None,
    }
}

fn handle_context_menu(state: &mut AppState, key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Char('j') | KeyCode::Down => {
            state.navigate_context_menu(1);
            Action::None
        }
        KeyCode::Char('k') | KeyCode::Up => {
            state.navigate_context_menu(-1);
            Action::None
        }
        KeyCode::Enter => mouse::activate_context_menu(state),
        KeyCode::Esc | KeyCode::Char('q') => {
            state.close_context_menu();
            Action::None
        }
        _ => Action::None,
    }
}

fn handle_theme_popup(state: &mut AppState, key: KeyEvent) -> Action {
    match (key.code, key.modifiers) {
        (KeyCode::Char('j'), _) | (KeyCode::Down, _) => {
            state.navigate_theme_popup_down();
        }
        (KeyCode::Char('k'), _) | (KeyCode::Up, _) => {
            state.navigate_theme_popup_up();
        }
        (KeyCode::Enter, _) => {
            state.apply_theme_and_close();
        }
        (KeyCode::Esc, _) => {
            state.close_theme_popup();
        }
        _ => {}
    }
    Action::None
}

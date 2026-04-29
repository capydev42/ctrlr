pub mod collection;
pub mod help;
pub mod normal;
pub mod tag;

use crate::app::{Action, AppState, InputMode};
use crossterm::event::{KeyCode, KeyEvent};

pub fn handle(state: &mut AppState, key: KeyEvent) -> Action {
    if state.theme_popup_open {
        return handle_theme_popup(state, key);
    }
    if state.help_open {
        return help::handle(state, key);
    }
    match state.input_mode {
        InputMode::TagInput => tag::handle(state, key),
        InputMode::CollectionInput => collection::handle(state, key),
        InputMode::Normal => normal::handle(state, key),
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

pub mod collection;
pub mod normal;
pub mod tag;

use crate::app::{Action, AppState, InputMode};
use crossterm::event::KeyEvent;

pub fn handle(state: &mut AppState, key: KeyEvent) -> Action {
    match state.input_mode {
        InputMode::TagInput => tag::handle(state, key),
        InputMode::CollectionInput => collection::handle(state, key),
        InputMode::Normal => normal::handle(state, key),
    }
}

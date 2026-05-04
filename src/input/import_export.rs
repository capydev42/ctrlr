use crate::app::{Action, AppState, ImportExportMode};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

pub fn handle(state: &mut AppState, key: KeyEvent) -> Action {
    match (key.code, key.modifiers) {
        (KeyCode::Char(c), KeyModifiers::NONE) => {
            state.import_export_file_path.push(c);
        }
        (KeyCode::Backspace, _) => {
            state.import_export_file_path.pop();
        }
        (KeyCode::Up, _) | (KeyCode::Char('p'), KeyModifiers::CONTROL)
            if state.import_export_mode == ImportExportMode::Import
                || state.import_export_mode == ImportExportMode::ImportPreview =>
        {
            state.import_mode_index = 0;
            state.import_preview = None;
            state.import_export_mode = ImportExportMode::Import;
        }
        (KeyCode::Down, _) | (KeyCode::Char('n'), KeyModifiers::CONTROL)
            if state.import_export_mode == ImportExportMode::Import
                || state.import_export_mode == ImportExportMode::ImportPreview =>
        {
            state.import_mode_index = 1;
        }
        (KeyCode::Enter, _) => match state.import_export_mode {
            ImportExportMode::Export => {
                state.execute_export();
            }
            ImportExportMode::Import => {
                state.preview_import();
            }
            ImportExportMode::ImportPreview => {
                state.execute_import();
            }
        },
        (KeyCode::Esc, _) => {
            state.close_import_export_popup();
        }
        _ => {}
    }
    Action::None
}

use crate::app::{Action, AppState, ImportExportMode};
use crate::keymap::KeyAction;

pub fn dispatch(state: &mut AppState, action: KeyAction) -> Action {
    // Only the import side has a mode to move between; on the export popup the
    // arrows have nothing to select.
    let importing = matches!(
        state.import_export_mode,
        ImportExportMode::Import | ImportExportMode::ImportPreview
    );
    match action {
        KeyAction::DeleteCharBackward => {
            state.import_export_file_path.pop();
        }
        KeyAction::KillLine => {
            state.import_export_file_path.clear();
            // The preview describes the path that was just cleared, so it has
            // to go with it.
            state.import_preview = None;
            if state.import_export_mode == ImportExportMode::ImportPreview {
                state.import_export_mode = ImportExportMode::Import;
            }
        }
        KeyAction::NavigateUp if importing => {
            state.import_mode_index = 0;
            state.import_preview = None;
            state.import_export_mode = ImportExportMode::Import;
        }
        KeyAction::NavigateDown if importing => {
            state.import_mode_index = 1;
        }
        KeyAction::Confirm => match state.import_export_mode {
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
        _ => {}
    }
    Action::None
}

pub fn insert_char(state: &mut AppState, c: char) {
    state.import_export_file_path.push(c);
}

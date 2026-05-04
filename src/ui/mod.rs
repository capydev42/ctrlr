pub mod collections;
pub mod components;
pub mod history;
pub mod layout;
pub mod popups;
pub mod theme;

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
};

use crate::app::{AppState, CollectionInputMode, InputMode, ViewMode};

pub fn render(frame: &mut Frame, state: &mut AppState) {
    let area = frame.area();
    state.set_terminal_height(area.height);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(2),
        ])
        .split(area);

    components::render_search_bar(frame, state, chunks[0]);
    components::render_tabs(frame, state, chunks[1]);
    components::render_footer(frame, state, chunks[3]);

    match state.view_mode {
        ViewMode::History | ViewMode::Favorites => {
            let (list_area, details_area) = if state.show_details {
                let content_chunks = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Percentage(65), Constraint::Percentage(35)])
                    .split(chunks[2]);
                (content_chunks[0], Some(content_chunks[1]))
            } else {
                (chunks[2], None)
            };
            history::render_history_list(frame, state, list_area);
            if let Some(details_area) = details_area {
                history::render_details(frame, state, details_area);
            }
        }
        ViewMode::Collections => {
            collections::render_collections_view(frame, state, chunks[2]);
        }
    }

    if state.input_mode == InputMode::TagInput {
        popups::render_tag_popup(frame, state, area);
    }

    if state.input_mode == InputMode::CollectionInput {
        match state.collection_input_mode {
            CollectionInputMode::AddToCollectionSearch => {
                popups::render_add_command_popup(frame, state, area);
            }
            CollectionInputMode::ConfirmDeleteCollection
            | CollectionInputMode::ConfirmDeleteCommand => {
                popups::render_delete_confirm_popup(frame, state, area);
            }
            _ => {
                popups::render_collection_popup(frame, state, area);
            }
        }
    }

    if state.help_open {
        popups::render_help_popup(frame, state, area);
    }

    if state.theme_popup_open {
        popups::render_theme_popup(frame, state, area);
    }

    if state.export_popup_open || state.import_popup_open {
        popups::render_import_export_popup(frame, state, area);
    }
}

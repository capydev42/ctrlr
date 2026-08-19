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
use crate::ui::layout::Hitboxes;

pub fn render(frame: &mut Frame, state: &mut AppState) {
    let area = frame.area();
    state.set_terminal_size(area.width, area.height);
    // Rebuilt from scratch every frame so a hitbox can never outlive the
    // widget that drew it; the renderers below fill in what they own.
    state.hit = Hitboxes::default();

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
            let details = state.show_details.then_some(state.details_width);
            let areas = layout::split_content(chunks[2], None, details);
            state.hit.content = chunks[2];
            state.hit.dividers = areas.dividers;
            history::render_history_list(frame, state, areas.list);
            if let Some(details_area) = areas.details {
                history::render_details(frame, state, details_area);
            }
        }
        ViewMode::Collections => {
            collections::render_collections_view(frame, state, chunks[2]);
        }
    }

    if state.input_mode == InputMode::EditCommand {
        popups::render_edit_command_popup(frame, state, area);
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

    // Above the view but below the modal popups: the menu is dismissed
    // whenever one of those opens.
    if state.context_menu_open {
        popups::render_context_menu(frame, state, area);
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

    // Drawn last so the startup offer sits above anything else on screen.
    if state.integration_popup_open {
        popups::render_integration_popup(frame, state, area);
    }
}

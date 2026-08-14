use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    text::Line,
    widgets::{Block, BorderType, List, ListItem},
};

use crate::app::{ActivePane, AppState};

use super::components::command_with_right_tags;

pub fn render_collections_view(frame: &mut Frame, state: &mut AppState, area: Rect) {
    let show_details = state.active_pane == ActivePane::CollectionItems && state.show_details;
    let areas = super::layout::split_content(
        area,
        Some(state.collections_width),
        show_details.then_some(state.details_width),
    );

    if let Some(collections_area) = areas.collections {
        render_collection_list(frame, state, collections_area);
    }
    render_collection_commands(frame, state, areas.list);
    if let Some(details_area) = areas.details {
        super::history::render_details(frame, state, details_area);
    }
}

pub fn render_collection_list(frame: &mut Frame, state: &mut AppState, area: Rect) {
    state.hit.collections_list = area;
    let theme = &state.current_theme;
    let items: Vec<ListItem> = if state.collections.is_empty() {
        vec![ListItem::new("No collections yet")]
    } else {
        state
            .collections
            .iter()
            .enumerate()
            .map(|(idx, col)| {
                let prefix = if idx == state.selected_collection_index {
                    "> "
                } else {
                    "  "
                };
                ListItem::new(format!("{}{}", prefix, col.name))
            })
            .collect()
    };

    let is_focused = state.active_pane == ActivePane::CollectionsList;
    let border_color = if is_focused {
        theme.focus_border
    } else {
        theme.unfocus_border
    };

    let list = List::new(items)
        .block(
            Block::bordered()
                .title(if is_focused {
                    "[Collections]"
                } else {
                    "Collections"
                })
                .border_type(BorderType::Rounded)
                .border_style(Style::new().fg(border_color)),
        )
        .highlight_style(
            Style::default()
                .bg(theme.highlight_bg)
                .fg(theme.highlight_fg),
        );

    state
        .collection_list_state
        .select(Some(state.selected_collection_index));
    frame.render_stateful_widget(list, area, &mut state.collection_list_state);
}

pub fn render_collection_commands(frame: &mut Frame, state: &mut AppState, area: Rect) {
    // Same hitbox slot as the history list: both draw `state.filtered`, and
    // only one of them is ever on screen.
    state.hit.list = area;
    let theme = &state.current_theme;
    let is_focused = state.active_pane == ActivePane::CollectionItems;
    let border_color = if is_focused {
        theme.focus_border
    } else {
        theme.unfocus_border
    };

    let title = if state.collections.is_empty() {
        "Commands".to_string()
    } else if let Some(col) = state.selected_collection() {
        if is_focused {
            format!("[{}]", col.name)
        } else {
            col.name.clone()
        }
    } else {
        "Commands".to_string()
    };

    let items: Vec<ListItem> = if state.collections.is_empty() {
        vec![ListItem::new("Create a collection first")]
    } else if state.selected_collection().is_some() && state.filtered.is_empty() {
        vec![ListItem::new("No commands match search")]
    } else if state.selected_collection().is_some() {
        let width = area.width.saturating_sub(4);
        let mut result = std::vec::Vec::new();
        for (i, c) in state.filtered.iter().enumerate() {
            let fav = if c.favorite { "* " } else { "  " };
            let mut line = Line::from(ratatui::text::Span::raw(fav));
            let indices = state.matched_indices.get(i).and_then(|m| m.as_ref());
            let line_with_tags = command_with_right_tags(&c.text, indices, &c.tags, width, theme);
            line.spans.extend(line_with_tags.spans);
            result.push(ratatui::widgets::ListItem::new(line));
        }
        result
    } else {
        vec![ListItem::new("Select a collection")]
    };

    let list = List::new(items)
        .block(
            Block::bordered()
                .title(title.as_str())
                .border_type(BorderType::Rounded)
                .border_style(Style::new().fg(border_color)),
        )
        .highlight_style(
            Style::default()
                .bg(theme.highlight_bg)
                .fg(theme.highlight_fg),
        )
        .highlight_symbol("> ");

    state
        .collection_items_list_state
        .select(Some(state.selected_index));
    frame.render_stateful_widget(list, area, &mut state.collection_items_list_state);
}

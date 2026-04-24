use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::Line,
    widgets::{Block, BorderType, List, ListItem},
};

use crate::app::{ActivePane, AppState};

use super::components::command_with_right_tags;

pub fn render_collections_view(frame: &mut Frame, state: &mut AppState, area: Rect) {
    if state.active_pane == ActivePane::CollectionItems && state.show_details {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(20),
                Constraint::Percentage(45),
                Constraint::Percentage(35),
            ])
            .split(area);

        render_collection_list(frame, state, chunks[0]);
        render_collection_commands(frame, state, chunks[1]);
        super::history::render_details(frame, state, chunks[2]);
    } else {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
            .split(area);

        render_collection_list(frame, state, chunks[0]);
        render_collection_commands(frame, state, chunks[1]);
    }
}

pub fn render_collection_list(frame: &mut Frame, state: &mut AppState, area: Rect) {
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

    let border_color = if state.active_pane == ActivePane::CollectionsList {
        Color::Yellow
    } else {
        Color::DarkGray
    };

    let list = List::new(items)
        .block(
            Block::bordered()
                .title("[Collections]")
                .border_type(BorderType::Rounded)
                .border_style(Style::new().fg(border_color)),
        )
        .highlight_style(Style::default().bg(Color::Blue).fg(Color::White));

    state
        .collection_list_state
        .select(Some(state.selected_collection_index));
    frame.render_stateful_widget(list, area, &mut state.collection_list_state);
}

pub fn render_collection_commands(frame: &mut Frame, state: &mut AppState, area: Rect) {
    let border_color = if state.active_pane == ActivePane::CollectionItems {
        Color::Yellow
    } else {
        Color::DarkGray
    };

    let title = if state.collections.is_empty() {
        "Commands".to_string()
    } else if let Some(col) = state.selected_collection() {
        col.name.clone()
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
            let line_with_tags = command_with_right_tags(&c.text, indices, &c.tags, width);
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
        .highlight_style(Style::default().bg(Color::Blue).fg(Color::White))
        .highlight_symbol("> ");

    state
        .collection_items_list_state
        .select(Some(state.selected_index));
    frame.render_stateful_widget(list, area, &mut state.collection_items_list_state);
}

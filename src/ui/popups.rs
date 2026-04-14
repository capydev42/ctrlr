use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::Span,
    widgets::{
        Block, BorderType, Clear, List, ListItem, Paragraph, Scrollbar, ScrollbarOrientation,
    },
};

use crate::app::{AppState, CollectionInputMode};

use super::layout::center_rect;

pub fn render_tag_popup(frame: &mut Frame, state: &mut AppState, area: Rect) {
    let tags = state.selected_command_tags();
    let suggestions = if state.tag_cursor_index.is_none() {
        state.filtered_tags()
    } else {
        Vec::new()
    };

    let search_lower = state.tag_input.trim().to_lowercase();
    let exact_match = suggestions.iter().any(|t| t.to_lowercase() == search_lower);
    let show_create = !state.tag_input.trim().is_empty() && !exact_match;

    let input_height = if tags.is_empty() { 3 } else { 4 };
    let sugg_count = suggestions.len().min(5);
    let create_line = if show_create { 1 } else { 0 };
    let sugg_height = if !suggestions.is_empty() || show_create {
        (sugg_count + create_line) as u16 + 2
    } else {
        0
    };
    let hint_height = 1u16;
    let popup_height = input_height + sugg_height + hint_height;
    let popup_width = 60u16;

    let centered = center_rect(popup_width, popup_height, area);

    frame.render_widget(Clear, centered);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(input_height),
            Constraint::Length(sugg_height),
            Constraint::Length(hint_height),
        ])
        .split(centered);

    let mut spans: Vec<Span> = Vec::new();
    spans.push(Span::raw("Tags: "));

    for (i, tag) in tags.iter().enumerate() {
        if Some(i) == state.tag_cursor_index {
            spans.push(Span::styled(
                format!("[ {} ]", tag),
                Style::default().fg(Color::Black).bg(Color::Yellow),
            ));
            spans.push(Span::raw(" "));
        } else {
            spans.push(Span::styled(
                format!(" {} ", tag),
                Style::default().fg(Color::White).bg(Color::DarkGray),
            ));
            spans.push(Span::raw(" "));
        }
    }

    if state.tag_cursor_index.is_none() || tags.is_empty() {
        spans.push(Span::styled(
            format!("{}▋", state.tag_input),
            Style::default().fg(Color::White),
        ));
    }

    frame.render_widget(
        Paragraph::new(ratatui::text::Line::from(spans)).block(
            Block::bordered()
                .title("[Edit Tags]")
                .border_type(BorderType::Rounded)
                .border_style(Style::new().fg(Color::Yellow)),
        ),
        chunks[0],
    );

    if !suggestions.is_empty() || show_create {
        let mut sugg_items: Vec<ListItem> = suggestions
            .iter()
            .enumerate()
            .map(|(i, tag)| {
                if i == state.tag_selected_index {
                    ListItem::new(format!("> {}", tag))
                        .style(Style::new().bg(Color::Blue).fg(Color::Black))
                } else {
                    ListItem::new(format!("  {}", tag))
                }
            })
            .collect();

        if show_create {
            let create_text = format!("+ Create \"{}\"", state.tag_input.trim());
            if state.tag_selected_index == suggestions.len() {
                sugg_items.push(
                    ListItem::new(format!("> {}", create_text))
                        .style(Style::new().fg(Color::Green).add_modifier(Modifier::BOLD)),
                );
            } else {
                sugg_items.push(
                    ListItem::new(format!("  {}", create_text))
                        .style(Style::new().fg(Color::Green).add_modifier(Modifier::BOLD)),
                );
            }
        }

        let total_items = sugg_items.len();
        let sugg_height = (total_items as u16 + 1).max(3);
        let sugg_area = Rect::new(chunks[1].x, chunks[1].y, chunks[1].width - 1, sugg_height);
        let scrollbar_area = Rect::new(
            chunks[1].x + chunks[1].width - 1,
            chunks[1].y,
            1,
            sugg_height,
        );

        let visible_rows = 5usize;
        let offset = state.tag_selected_index.saturating_sub(visible_rows / 2);
        *state.tag_popup_list_state.offset_mut() = offset;
        state
            .tag_popup_list_state
            .select(Some(state.tag_selected_index));

        let sugg_list = List::new(sugg_items)
            .block(Block::bordered().title("Suggestions"))
            .highlight_style(Style::new().bg(Color::Blue).fg(Color::Black));
        frame.render_stateful_widget(sugg_list, sugg_area, &mut state.tag_popup_list_state);

        if total_items > 3 {
            let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .style(Style::new().fg(Color::DarkGray));
            let mut scrollbar_state = ratatui::widgets::ScrollbarState::new(total_items)
                .position(state.tag_selected_index);
            frame.render_stateful_widget(scrollbar, scrollbar_area, &mut scrollbar_state);
        }
    }

    let hint = "↑/↓: Navigate | Type: Filter | Enter: Select/Create | Esc: Cancel";
    frame.render_widget(
        Paragraph::new(hint)
            .style(Style::new().fg(Color::DarkGray))
            .alignment(Alignment::Center),
        chunks[2],
    );
}

pub fn render_collection_popup(frame: &mut Frame, state: &mut AppState, area: Rect) {
    let popup_height = 8u16;
    let popup_width = 45u16;
    let centered = center_rect(popup_width, popup_height, area);

    frame.render_widget(Clear, centered);

    let (title, hint) = match state.collection_input_mode {
        CollectionInputMode::NewCollection => (
            "[New Collection]",
            "Type name | Enter: Create | Esc: Cancel",
        ),
        CollectionInputMode::EditCollection => (
            "[Rename Collection]",
            "Type name | Enter: Save | Esc: Cancel",
        ),
        CollectionInputMode::AddToCollection => (
            "[Add to Collection]",
            "Type to filter | ↑/↓ Navigate | Enter: Select/Create | Esc: Cancel",
        ),
        CollectionInputMode::AddToCollectionSearch => return,
        CollectionInputMode::None => return,
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(centered);

    let search_text = format!("Search: {}{}", state.collection_input_text, "▋");
    frame.render_widget(
        Paragraph::new(search_text).style(Style::new().fg(Color::Yellow)),
        chunks[0],
    );

    if state.collection_input_mode == CollectionInputMode::AddToCollection {
        let filtered = state.filtered_collections(&state.collection_input_text);
        let active_cmd = state.active_command();
        let cmd_col_ids: Vec<&str> = active_cmd
            .map(|c| c.collection_ids.iter().map(|s| s.as_str()).collect())
            .unwrap_or_default();

        let search_lower = state.collection_input_text.to_lowercase();
        let exact_match = filtered
            .iter()
            .any(|c| c.name.to_lowercase() == search_lower);

        let mut items: Vec<ListItem> = filtered
            .iter()
            .enumerate()
            .map(|(idx, col)| {
                let prefix = if cmd_col_ids.contains(&col.id.as_str()) {
                    "✔ "
                } else {
                    "  "
                };
                if idx == state.collection_popup_index {
                    ListItem::new(format!("> {}{}", prefix, col.name))
                        .style(Style::new().bg(Color::Blue).fg(Color::Black))
                } else {
                    ListItem::new(format!("  {}{}", prefix, col.name))
                }
            })
            .collect();

        if !state.collection_input_text.is_empty() && !exact_match {
            let create_text = format!("+ Create \"{}\"", state.collection_input_text);
            if state.collection_popup_index == filtered.len() {
                items.push(
                    ListItem::new(format!("> {}", create_text))
                        .style(Style::new().fg(Color::Green).add_modifier(Modifier::BOLD)),
                );
            } else {
                items.push(
                    ListItem::new(format!("  {}", create_text))
                        .style(Style::new().fg(Color::Green).add_modifier(Modifier::BOLD)),
                );
            }
        }

        let total_items = items.len();
        let list_area = Rect::new(
            chunks[2].x,
            chunks[2].y,
            chunks[2].width - 1,
            chunks[2].height,
        );
        let scrollbar_area = Rect::new(
            chunks[2].x + chunks[2].width - 1,
            chunks[2].y,
            1,
            chunks[2].height,
        );

        let visible_rows = 5usize;
        let offset = state
            .collection_popup_index
            .saturating_sub(visible_rows / 2);
        *state.collection_popup_list_state.offset_mut() = offset;
        state
            .collection_popup_list_state
            .select(Some(state.collection_popup_index));

        let list = List::new(items)
            .block(
                Block::bordered()
                    .title(title)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::new().fg(Color::Yellow)),
            )
            .highlight_style(Style::new().bg(Color::Blue).fg(Color::White));
        frame.render_stateful_widget(list, list_area, &mut state.collection_popup_list_state);

        if total_items > 3 {
            let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .style(Style::new().fg(Color::DarkGray));
            let mut scrollbar_state = ratatui::widgets::ScrollbarState::new(total_items)
                .position(state.collection_popup_index);
            frame.render_stateful_widget(scrollbar, scrollbar_area, &mut scrollbar_state);
        }
    } else {
        let input_display = if state.collection_input_text.is_empty() {
            "▋".to_string()
        } else {
            format!("{}▋", state.collection_input_text)
        };
        frame.render_widget(
            Paragraph::new(input_display)
                .style(Style::new().fg(Color::White))
                .block(
                    Block::bordered()
                        .title(title)
                        .border_type(BorderType::Rounded)
                        .border_style(Style::new().fg(Color::Yellow)),
                ),
            chunks[2],
        );
    }

    frame.render_widget(
        Paragraph::new(hint)
            .style(Style::new().fg(Color::DarkGray))
            .alignment(Alignment::Center),
        chunks[3],
    );
}

pub fn render_add_command_popup(frame: &mut Frame, state: &mut AppState, area: Rect) {
    let results = state.search_results_for_add_command();
    let search_text = state.collection_input_text.trim();
    let has_search = !search_text.is_empty();

    let input_height = 3u16;
    let results_count = results.len();
    let create_row = if has_search { 1 } else { 0 };
    let total_rows = results_count.max(3) + create_row;
    let sugg_count = total_rows.min(5);
    let sugg_height = sugg_count.max(3) as u16 + 1;
    let hint_height = 1u16;
    let popup_height = input_height + sugg_height + hint_height;
    let popup_width = 65u16;

    let centered = center_rect(popup_width, popup_height, area);

    frame.render_widget(Clear, centered);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(input_height),
            Constraint::Length(sugg_height),
            Constraint::Length(hint_height),
        ])
        .split(centered);

    let search_display = format!("Search: {}{}", state.collection_input_text, "▋");
    frame.render_widget(
        Paragraph::new(search_display)
            .style(Style::new().fg(Color::White))
            .block(
                Block::bordered()
                    .title("[Add Command]")
                    .border_type(BorderType::Rounded)
                    .border_style(Style::new().fg(Color::Cyan)),
            ),
        chunks[0],
    );

    let results_count = results.len();

    if total_rows > 0 {
        let mut items: Vec<ListItem> = results
            .iter()
            .take(5)
            .enumerate()
            .map(|(i, cmd)| {
                if i == state.add_command_search_index {
                    ListItem::new(format!("> {}", cmd.text))
                        .style(Style::new().bg(Color::Blue).fg(Color::White))
                } else {
                    ListItem::new(format!("  {}", cmd.text))
                }
            })
            .collect();

        if has_search {
            let create_text = format!("+ Create \"{}\"", state.collection_input_text.trim());
            let create_idx = results_count;
            if state.add_command_search_index == create_idx {
                items.push(
                    ListItem::new(format!("> {}", create_text))
                        .style(Style::new().fg(Color::Green).add_modifier(Modifier::BOLD)),
                );
            } else {
                items.push(
                    ListItem::new(format!("  {}", create_text))
                        .style(Style::new().fg(Color::Green).add_modifier(Modifier::BOLD)),
                );
            }
        }

        let total_items = items.len();
        let list_area = Rect::new(
            chunks[1].x,
            chunks[1].y,
            chunks[1].width - 1,
            chunks[1].height,
        );
        let scrollbar_area = Rect::new(
            chunks[1].x + chunks[1].width - 1,
            chunks[1].y,
            1,
            chunks[1].height,
        );

        let visible_rows = 5usize;
        let offset = state
            .add_command_search_index
            .saturating_sub(visible_rows / 2);
        *state.collection_popup_list_state.offset_mut() = offset;
        state
            .collection_popup_list_state
            .select(Some(state.add_command_search_index));

        let list = List::new(items)
            .block(Block::bordered().title("Commands"))
            .highlight_style(Style::new().bg(Color::Blue).fg(Color::White));
        frame.render_stateful_widget(list, list_area, &mut state.collection_popup_list_state);

        if total_items > 3 {
            let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .style(Style::new().fg(Color::DarkGray));
            let mut scrollbar_state = ratatui::widgets::ScrollbarState::new(total_items)
                .position(state.add_command_search_index);
            frame.render_stateful_widget(scrollbar, scrollbar_area, &mut scrollbar_state);
        }
    }

    let hint = "↑/↓ Navigate | Enter: Add/Create | Esc: Cancel";
    frame.render_widget(
        Paragraph::new(hint)
            .style(Style::new().fg(Color::DarkGray))
            .alignment(Alignment::Center),
        chunks[2],
    );
}

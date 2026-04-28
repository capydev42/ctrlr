use ratatui::{
    Frame,
    layout::{Alignment, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, List, ListItem, Paragraph, Wrap},
};

use crate::app::{ActivePane, AppState, ViewMode};

use super::components::{FOCUS_BORDER, UNFOCUS_BORDER, command_with_right_tags, tag_span};

pub fn section(title: &str) -> Line<'_> {
    Line::from(Span::styled(
        format!("─ {} ─", title),
        Style::new().fg(Color::Blue).bold(),
    ))
}

pub fn render_history_list(frame: &mut Frame, state: &mut AppState, area: Rect) {
    let items: Vec<ListItem> = if state.filtered.is_empty() {
        vec![ListItem::new("No results found")]
    } else {
        let width = area.width.saturating_sub(4);
        let mut result = std::vec::Vec::new();
        for (i, c) in state.filtered.iter().enumerate() {
            let favorite_str = if c.favorite { "* " } else { "  " };
            let mut line = Line::from(ratatui::text::Span::raw(favorite_str));
            let idx = state.matched_indices.get(i).and_then(|m| m.as_ref());
            let cmd_line = command_with_right_tags(&c.text, idx, &c.tags, width);
            line.spans.extend(cmd_line.spans);
            result.push(ratatui::widgets::ListItem::new(line));
        }
        result
    };

    let list_title = match state.view_mode {
        ViewMode::History => {
            if state.active_pane == ActivePane::History {
                "[History]".to_string()
            } else {
                "History".to_string()
            }
        }
        ViewMode::Favorites => {
            if state.active_pane == ActivePane::History {
                "[Favorites]".to_string()
            } else {
                "Favorites".to_string()
            }
        }
        ViewMode::Collections => state
            .selected_collection()
            .map(|c| c.name.clone())
            .unwrap_or_else(|| "Commands".to_string()),
    };

    let is_focused = state.active_pane == ActivePane::History;
    let border_color = if is_focused {
        FOCUS_BORDER
    } else {
        UNFOCUS_BORDER
    };

    let list = List::new(items)
        .block(
            Block::bordered()
                .title(list_title.as_str())
                .border_type(BorderType::Rounded)
                .border_style(Style::new().fg(border_color)),
        )
        .highlight_style(Style::default().bg(Color::Blue).fg(Color::White))
        .highlight_symbol("> ");

    frame.render_stateful_widget(list, area, &mut state.list_state);
}

pub fn render_details(frame: &mut Frame, state: &mut AppState, area: Rect) {
    if area.width < 5 || area.height < 3 {
        return;
    }

    if state.filtered.is_empty() {
        let is_focused = state.active_pane == ActivePane::History;
        let border_color = if is_focused {
            FOCUS_BORDER
        } else {
            UNFOCUS_BORDER
        };
        frame.render_widget(
            Paragraph::new("No command selected")
                .alignment(Alignment::Center)
                .block(
                    Block::bordered()
                        .title(if is_focused { "[Details]" } else { "Details" })
                        .border_type(BorderType::Rounded)
                        .border_style(Style::new().fg(border_color)),
                ),
            area,
        );
        return;
    }

    let cmd = match state.active_command() {
        Some(c) => c,
        None => return,
    };

    let mut lines: Vec<Line> = Vec::new();

    lines.push(section("Command"));
    lines.push(Line::from(cmd.text.clone()));
    lines.push(Line::from(""));

    if !cmd.tags.is_empty() {
        lines.push(section("Tags"));
        for tag in &cmd.tags {
            lines.push(Line::from(vec![tag_span(tag)]));
        }
        lines.push(Line::from(""));
    }

    if !cmd.collection_ids.is_empty() {
        lines.push(section("Collections"));
        for col_id in &cmd.collection_ids {
            if let Some(col) = state.collections.iter().find(|c| &c.id == col_id) {
                lines.push(Line::from(format!("- {}", col.name)));
            }
        }
        lines.push(Line::from(""));
    }

    lines.push(section("Usage"));
    lines.push(Line::from(format!("Used: {}x", cmd.use_count)));
    if let Some(ts) = cmd.last_used {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        let ago = now - ts;
        let ago_str = if ago < 60 {
            format!("{}s ago", ago)
        } else if ago < 3600 {
            format!("{}m ago", ago / 60)
        } else if ago < 86400 {
            format!("{}h ago", ago / 3600)
        } else {
            format!("{}d ago", ago / 86400)
        };
        lines.push(Line::from(format!("Last used: {}", ago_str)));
    }
    lines.push(Line::from(""));

    lines.push(section("Favorite"));
    let fav_text = if cmd.favorite { "* yes" } else { "○ no" };
    let fav_style = if cmd.favorite {
        Style::new().fg(Color::Yellow)
    } else {
        Style::new()
    };
    lines.push(Line::from(Span::styled(fav_text, fav_style)));

    let block = Block::bordered()
        .title("Details")
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(Color::DarkGray));

    frame.render_widget(
        Paragraph::new(lines).block(block).wrap(Wrap { trim: true }),
        area,
    );
}

use ratatui::{
    Frame,
    layout::{Alignment, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Block, BorderType, List, ListItem, Paragraph, Wrap},
};

use crate::app::{ActivePane, AppState, ViewMode};

use super::components::{command_with_right_tags, tag_span};

pub fn section<'a>(title: &str, theme: &crate::ui::theme::Theme) -> Line<'a> {
    Line::from(Span::styled(
        format!("─ {} ─", title),
        Style::new().fg(theme.section_fg).bold(),
    ))
}

pub fn render_history_list(frame: &mut Frame, state: &mut AppState, area: Rect) {
    state.hit.list = area;

    // Only the rows about to be shown are built. `filtered` is the whole shell
    // history, so building all of it every frame costs tens of milliseconds
    // and makes anything that redraws continuously — a divider drag — trail
    // the pointer. The offset is computed here and written back, so the rest
    // of the code still reads it off the `ListState`.
    let viewport = area.height.saturating_sub(2) as usize;
    let offset = super::layout::scroll_offset(
        state.list_state.offset(),
        state.selected_index,
        state.filtered.len(),
        viewport,
    );
    *state.list_state.offset_mut() = offset;

    let theme = &state.current_theme;
    let items: Vec<ListItem> = if state.filtered.is_empty() {
        vec![ListItem::new("No results found")]
    } else {
        let width = area.width.saturating_sub(4);
        let end = (offset + viewport).min(state.filtered.len());
        let mut result = std::vec::Vec::new();
        for (i, c) in state.filtered[offset..end].iter().enumerate() {
            let favorite_str = if c.favorite { "* " } else { "  " };
            let mut line = Line::from(ratatui::text::Span::raw(favorite_str));
            let idx = state
                .matched_indices
                .get(offset + i)
                .and_then(|m| m.as_ref());
            let cmd_line = command_with_right_tags(&c.text, idx, &c.tags, width, theme);
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
        theme.focus_border
    } else {
        theme.unfocus_border
    };

    let list = List::new(items)
        .block(
            Block::bordered()
                .title(list_title.as_str())
                .border_type(BorderType::Rounded)
                .border_style(Style::new().fg(border_color)),
        )
        .highlight_style(
            Style::default()
                .bg(theme.highlight_bg)
                .fg(theme.highlight_fg),
        )
        .highlight_symbol("> ");

    // The widget only ever sees the window, so its selection is relative to
    // the offset. `state.list_state` keeps the absolute selection the input
    // handlers set, and the absolute offset written above.
    let mut window_state = ratatui::widgets::ListState::default();
    if !state.filtered.is_empty() {
        window_state.select(Some(state.selected_index.saturating_sub(offset)));
    }
    frame.render_stateful_widget(list, area, &mut window_state);
}

pub fn render_details(frame: &mut Frame, state: &mut AppState, area: Rect) {
    if area.width < 5 || area.height < 3 {
        return;
    }
    state.hit.details = area;

    let theme = &state.current_theme;

    if state.filtered.is_empty() {
        let is_focused = state.active_pane == ActivePane::History;
        let border_color = if is_focused {
            theme.focus_border
        } else {
            theme.unfocus_border
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

    /// Coarse "how long ago", shared by the last-used and last-run lines.
    fn ago(ts: i64) -> String {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        let ago = now - ts;
        if ago < 60 {
            format!("{}s ago", ago)
        } else if ago < 3600 {
            format!("{}m ago", ago / 60)
        } else if ago < 86400 {
            format!("{}h ago", ago / 3600)
        } else {
            format!("{}d ago", ago / 86400)
        }
    }

    let mut lines: Vec<Line> = Vec::new();

    lines.push(section("Command", theme));
    lines.push(Line::from(cmd.text.clone()));
    lines.push(Line::from(""));

    if !cmd.tags.is_empty() {
        lines.push(section("Tags", theme));
        for tag in &cmd.tags {
            lines.push(Line::from(vec![tag_span(tag, theme)]));
        }
        lines.push(Line::from(""));
    }

    if !cmd.collection_ids.is_empty() {
        lines.push(section("Collections", theme));
        for col_id in &cmd.collection_ids {
            if let Some(col) = state.collections.iter().find(|c| &c.id == col_id) {
                lines.push(Line::from(format!("- {}", col.name)));
            }
        }
        lines.push(Line::from(""));
    }

    lines.push(section("Usage", theme));
    lines.push(Line::from(format!("Used: {}x", cmd.use_count)));
    if let Some(ts) = cmd.last_used {
        lines.push(Line::from(format!("Last used: {}", ago(ts))));
    }
    lines.push(Line::from(""));

    // Only shown once the shell integration has recorded something: the run log
    // fills going forward, so an older install has nothing here yet.
    if let Some(summary) = state
        .db
        .as_ref()
        .and_then(|conn| crate::storage::runs::run_summary(conn, &cmd.id))
    {
        lines.push(section("Runs", theme));
        lines.push(Line::from(format!("Recorded: {}x", summary.total)));
        if cmd.runs_here > 0 {
            lines.push(Line::from(format!("Here: {}x", cmd.runs_here)));
        }
        if let Some(code) = summary.last_exit {
            let style = if code == 0 {
                Style::new()
            } else {
                Style::new().fg(theme.favorite_fg)
            };
            lines.push(Line::from(Span::styled(
                format!("Last exit: {}", code),
                style,
            )));
        }
        if let Some(ts) = summary.last_run {
            lines.push(Line::from(format!("Last run: {}", ago(ts))));
        }
        for (dir, count) in &summary.top_dirs {
            lines.push(Line::from(format!("- {} ({}x)", dir, count)));
        }
        lines.push(Line::from(""));
    }

    lines.push(section("Favorite", theme));
    let fav_text = if cmd.favorite { "* yes" } else { "○ no" };
    let fav_style = if cmd.favorite {
        Style::new().fg(theme.favorite_fg)
    } else {
        Style::new()
    };
    lines.push(Line::from(Span::styled(fav_text, fav_style)));

    let block = Block::bordered()
        .title("Details")
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(theme.unfocus_border));

    frame.render_widget(
        Paragraph::new(lines).block(block).wrap(Wrap { trim: true }),
        area,
    );
}

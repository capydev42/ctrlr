use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, BorderType, Clear, List, ListItem, Paragraph, Scrollbar, ScrollbarOrientation,
    },
};

use crate::app::{AppState, CollectionInputMode};
use crate::ui::theme::CatppuccinFlavor;

use super::layout::center_rect;

const VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn render_tag_popup(frame: &mut Frame, state: &mut AppState, area: Rect) {
    let theme = &state.current_theme;
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
    let create_line = if show_create { 1 } else { 0 };
    let results_count = suggestions.len() + create_line;
    let sugg_height = 3.max(results_count) as u16 + 2;
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
                format!("[{}]", tag),
                Style::default()
                    .fg(theme.highlight_fg)
                    .bg(theme.tag_selected_bg),
            ));
            spans.push(Span::raw(" "));
        } else {
            spans.push(Span::styled(
                format!("[{}]", tag),
                Style::default().fg(theme.tag_fg).bg(theme.tag_bg),
            ));
            spans.push(Span::raw(" "));
        }
    }

    if state.tag_cursor_index.is_none() || tags.is_empty() {
        spans.push(Span::styled(
            format!("{}▋", state.tag_input),
            Style::default().fg(theme.input_text),
        ));
    }

    frame.render_widget(
        Paragraph::new(ratatui::text::Line::from(spans)).block(
            Block::bordered()
                .title("[Edit Tags]")
                .border_type(BorderType::Rounded)
                .border_style(Style::new().fg(theme.tag_popup_border)),
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
                        .style(Style::new().bg(theme.highlight_bg).fg(theme.highlight_fg))
                } else {
                    ListItem::new(format!("  {}", tag))
                }
            })
            .collect();

        if show_create {
            let create_text = format!("+ Create \"{}\"", state.tag_input.trim());
            if state.tag_selected_index == suggestions.len() {
                sugg_items.push(
                    ListItem::new(format!("> {}", create_text)).style(
                        Style::new()
                            .fg(theme.create_fg)
                            .add_modifier(Modifier::BOLD),
                    ),
                );
            } else {
                sugg_items.push(
                    ListItem::new(format!("  {}", create_text)).style(
                        Style::new()
                            .fg(theme.create_fg)
                            .add_modifier(Modifier::BOLD),
                    ),
                );
            }
        }

        let total_items = sugg_items.len();
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
            .highlight_style(Style::new().bg(theme.highlight_bg).fg(theme.highlight_fg));
        frame.render_stateful_widget(sugg_list, sugg_area, &mut state.tag_popup_list_state);

        if total_items > 3 {
            let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .style(Style::new().fg(theme.scrollbar_fg));
            let mut scrollbar_state = ratatui::widgets::ScrollbarState::new(total_items)
                .position(state.tag_selected_index);
            frame.render_stateful_widget(scrollbar, scrollbar_area, &mut scrollbar_state);
        }
    }

    let hint = "↑/↓: Navigate | Type: Filter | Enter: Select/Create | Esc: Cancel";
    frame.render_widget(
        Paragraph::new(hint)
            .style(Style::new().fg(theme.hint_fg))
            .alignment(Alignment::Center),
        chunks[2],
    );
}

pub fn render_collection_popup(frame: &mut Frame, state: &mut AppState, area: Rect) {
    let theme = &state.current_theme;
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
        CollectionInputMode::ConfirmDeleteCollection
        | CollectionInputMode::ConfirmDeleteCommand => return,
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
        Paragraph::new(search_text).style(Style::new().fg(theme.input_text)),
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
                        .style(Style::new().bg(theme.highlight_bg).fg(theme.highlight_fg))
                } else {
                    ListItem::new(format!("  {}{}", prefix, col.name))
                }
            })
            .collect();

        if !state.collection_input_text.is_empty() && !exact_match {
            let create_text = format!("+ Create \"{}\"", state.collection_input_text);
            if state.collection_popup_index == filtered.len() {
                items.push(
                    ListItem::new(format!("> {}", create_text)).style(
                        Style::new()
                            .fg(theme.create_fg)
                            .add_modifier(Modifier::BOLD),
                    ),
                );
            } else {
                items.push(
                    ListItem::new(format!("  {}", create_text)).style(
                        Style::new()
                            .fg(theme.create_fg)
                            .add_modifier(Modifier::BOLD),
                    ),
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
                    .border_style(Style::new().fg(theme.popup_border)),
            )
            .highlight_style(Style::new().bg(theme.highlight_bg).fg(theme.highlight_fg));
        frame.render_stateful_widget(list, list_area, &mut state.collection_popup_list_state);

        if total_items > 3 {
            let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .style(Style::new().fg(theme.scrollbar_fg));
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
                .style(Style::new().fg(theme.input_text))
                .block(
                    Block::bordered()
                        .title(title)
                        .border_type(BorderType::Rounded)
                        .border_style(Style::new().fg(theme.popup_border)),
                ),
            chunks[2],
        );
    }

    frame.render_widget(
        Paragraph::new(hint)
            .style(Style::new().fg(theme.hint_fg))
            .alignment(Alignment::Center),
        chunks[3],
    );
}

pub fn render_add_command_popup(frame: &mut Frame, state: &mut AppState, area: Rect) {
    let theme = &state.current_theme;
    let results = state.search_results_for_add_command();
    let search_text = state.collection_input_text.trim();
    let has_search = !search_text.is_empty();

    let input_height = 3u16;
    let create_row = if has_search { 1 } else { 0 };
    let total_rows = results.len() + create_row;
    let sugg_height = (total_rows.min(5) as u16).max(3) + 1;
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
            .style(Style::new().fg(theme.input_text))
            .block(
                Block::bordered()
                    .title("[Add Command]")
                    .border_type(BorderType::Rounded)
                    .border_style(Style::new().fg(theme.popup_border)),
            ),
        chunks[0],
    );

    let total_rows = results.len() + if has_search { 1 } else { 0 };
    let take_count = 5.min(total_rows.saturating_sub(1));

    if !results.is_empty() || has_search {
        let mut items: Vec<ListItem> = results
            .iter()
            .take(take_count)
            .enumerate()
            .map(|(i, cmd)| {
                if i == state.add_command_search_index {
                    ListItem::new(format!("> {}", cmd.text))
                        .style(Style::new().bg(theme.highlight_bg).fg(theme.highlight_fg))
                } else {
                    ListItem::new(format!("  {}", cmd.text))
                }
            })
            .collect();

        if has_search {
            let create_text = format!("+ Create \"{}\"", state.collection_input_text.trim());
            let create_idx = results.len();
            if state.add_command_search_index == create_idx {
                items.push(
                    ListItem::new(format!("> {}", create_text)).style(
                        Style::new()
                            .fg(theme.create_fg)
                            .add_modifier(Modifier::BOLD),
                    ),
                );
            } else {
                items.push(
                    ListItem::new(format!("  {}", create_text)).style(
                        Style::new()
                            .fg(theme.create_fg)
                            .add_modifier(Modifier::BOLD),
                    ),
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
            .highlight_style(Style::new().bg(theme.highlight_bg).fg(theme.highlight_fg));
        frame.render_stateful_widget(list, list_area, &mut state.collection_popup_list_state);

        if total_items > 3 {
            let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .style(Style::new().fg(theme.scrollbar_fg));
            let mut scrollbar_state = ratatui::widgets::ScrollbarState::new(total_items)
                .position(state.add_command_search_index);
            frame.render_stateful_widget(scrollbar, scrollbar_area, &mut scrollbar_state);
        }
    }

    let hint = "↑/↓ Navigate | Enter: Add/Create | Esc: Cancel";
    frame.render_widget(
        Paragraph::new(hint)
            .style(Style::new().fg(theme.hint_fg))
            .alignment(Alignment::Center),
        chunks[2],
    );
}

pub fn render_delete_confirm_popup(frame: &mut Frame, state: &mut AppState, area: Rect) {
    let theme = &state.current_theme;
    let message = match state.collection_input_mode {
        CollectionInputMode::ConfirmDeleteCollection => {
            format!("Delete collection '{}'?", state.delete_confirm_text)
        }
        CollectionInputMode::ConfirmDeleteCommand => {
            format!("Remove '{}' from collection?", state.delete_confirm_text)
        }
        _ => return,
    };

    let popup_height = 10u16;
    let popup_width = 55u16;
    let centered = center_rect(popup_width, popup_height, area);

    frame.render_widget(Clear, centered);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(centered);

    frame.render_widget(
        Paragraph::new(message)
            .style(
                Style::new()
                    .fg(theme.favorite_fg)
                    .add_modifier(Modifier::BOLD),
            )
            .alignment(Alignment::Center)
            .block(
                Block::bordered()
                    .title("[Confirm Delete]")
                    .border_type(BorderType::Rounded)
                    .border_style(Style::new().fg(theme.popup_border)),
            ),
        chunks[0],
    );

    frame.render_widget(
        Paragraph::new("This action cannot be undone.")
            .style(Style::new().fg(theme.hint_fg))
            .alignment(Alignment::Center),
        chunks[1],
    );

    frame.render_widget(
        Paragraph::new("Enter: Delete  |  Esc: Cancel")
            .style(Style::new().fg(theme.input_text))
            .alignment(Alignment::Center),
        chunks[2],
    );
}

pub fn render_help_popup(frame: &mut Frame, state: &mut AppState, area: Rect) {
    let theme = &state.current_theme;
    let shortcuts = &state.help_filtered_shortcuts;
    let selected_index = state.help_selected_index;

    let search_height = 3u16;
    let hint_height = 1u16;
    let page_size = area.height.saturating_sub(8).max(5);
    let list_height = page_size;
    let popup_height = search_height + list_height + hint_height;
    let popup_width = (area.width - 4).clamp(50, 90);

    let centered = center_rect(popup_width, popup_height, area);

    frame.render_widget(Clear, centered);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(search_height),
            Constraint::Length(list_height),
            Constraint::Length(hint_height),
        ])
        .split(centered);

    let search_display = format!("Search: {}{}", state.help_search_query, "▋");
    frame.render_widget(
        Paragraph::new(search_display)
            .style(Style::new().fg(theme.input_text))
            .block(
                Block::bordered()
                    .title("[Help]")
                    .title(format!("[ctrlr v{}]", VERSION))
                    .border_type(BorderType::Rounded)
                    .border_style(Style::new().fg(theme.help_search_border)),
            ),
        chunks[0],
    );

    if !shortcuts.is_empty() {
        let rendered_selected = {
            let mut headers = 0;
            let mut prev: Option<&str> = None;
            for (i, sc) in shortcuts.iter().enumerate() {
                if i <= selected_index {
                    if prev != Some(sc.category) {
                        headers += 1;
                    }
                    prev = Some(sc.category);
                }
            }
            selected_index + headers
        };

        let area = chunks[1];
        let keys_width = (area.width / 6).max(10);
        let name_width = (area.width / 4).max(15);

        let mut current_category: Option<&str> = None;
        let mut rendered_items: Vec<ListItem> = Vec::new();

        for sc in shortcuts {
            if current_category != Some(sc.category) {
                current_category = Some(sc.category);
                let header = Line::from(vec![Span::styled(
                    sc.category,
                    Style::new()
                        .fg(theme.header_fg)
                        .add_modifier(Modifier::UNDERLINED)
                        .add_modifier(Modifier::BOLD),
                )]);
                rendered_items.push(ListItem::new(header).style(Style::new().bg(theme.header_bg)));
            }

            let keys_str: String = sc
                .keys
                .iter()
                .map(|&k| match k {
                    "PageDown" => "[PgDn]".to_owned(),
                    "PageUp" => "[PgUp]".to_owned(),
                    "Backspace" => "[BkSp]".to_owned(),
                    "Delete" => "[Del]".to_owned(),
                    "Escape" => "[Esc]".to_owned(),
                    "Return" => "[Ent]".to_owned(),
                    _ => format!("[{}]", k),
                })
                .collect::<Vec<_>>()
                .join(" ");
            let line = Line::from(vec![
                Span::styled(
                    format!("{:width$}", keys_str, width = keys_width as usize),
                    Style::new().fg(theme.help_keys_fg),
                ),
                Span::raw(" "),
                Span::styled(
                    format!("{:width$}", sc.action_name, width = name_width as usize),
                    Style::new()
                        .fg(theme.help_name_fg)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(" "),
                Span::styled(sc.description, Style::new().fg(theme.help_desc_fg)),
            ]);
            rendered_items.push(ListItem::new(line));
        }

        let total_items = rendered_items.len();
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

        let visible_rows = list_height as usize;
        let offset = rendered_selected.saturating_sub(visible_rows / 2);
        *state.help_list_state.offset_mut() = offset;
        state.help_list_state.select(Some(rendered_selected));

        let list = List::new(rendered_items)
            .block(Block::bordered().title("Shortcuts"))
            .highlight_style(Style::new().bg(theme.highlight_bg).fg(theme.highlight_fg));
        frame.render_stateful_widget(list, list_area, &mut state.help_list_state);

        if total_items > visible_rows {
            let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .style(Style::new().fg(theme.scrollbar_fg));
            let mut scrollbar_state =
                ratatui::widgets::ScrollbarState::new(total_items).position(rendered_selected);
            frame.render_stateful_widget(scrollbar, scrollbar_area, &mut scrollbar_state);
        }
    } else {
        frame.render_widget(
            Paragraph::new("No matching shortcuts")
                .style(Style::new().fg(theme.hint_fg))
                .alignment(Alignment::Center),
            chunks[1],
        );
    }

    frame.render_widget(
        Paragraph::new("? Help | ↑/↓ Navigate | Enter: Execute | Esc: Close")
            .style(Style::new().fg(theme.hint_fg))
            .alignment(Alignment::Center),
        chunks[2],
    );
}

pub fn render_theme_popup(frame: &mut Frame, state: &mut AppState, area: Rect) {
    let theme = &state.current_theme;
    let flavors = CatppuccinFlavor::all();

    let popup_height = 8u16;
    let popup_width = 35u16;
    let centered = center_rect(popup_width, popup_height, area);

    frame.render_widget(Clear, centered);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(centered);

    let mut items: Vec<ListItem> = Vec::new();
    for (i, flavor) in flavors.iter().enumerate() {
        let flavor_theme = flavor.theme();
        let accent = flavor_theme.focus_border;
        let name = flavor.to_string();
        if i == state.theme_popup_index {
            let line = Line::from(vec![
                Span::raw(" ● "),
                Span::styled("██", Style::new().fg(accent)),
                Span::raw(" "),
                Span::styled(
                    name,
                    Style::new()
                        .fg(theme.input_text)
                        .add_modifier(Modifier::BOLD),
                ),
            ]);
            items.push(
                ListItem::new(line)
                    .style(Style::new().bg(theme.highlight_bg).fg(theme.highlight_fg)),
            );
        } else {
            let line = Line::from(vec![
                Span::raw(" ○ "),
                Span::styled("██", Style::new().fg(accent)),
                Span::raw(" "),
                Span::styled(name, Style::new().fg(theme.input_text)),
            ]);
            items.push(ListItem::new(line));
        }
    }

    let list = List::new(items)
        .block(
            Block::bordered()
                .title("[Select Theme]")
                .border_type(BorderType::Rounded)
                .border_style(Style::new().fg(theme.popup_border)),
        )
        .highlight_style(Style::new().bg(theme.highlight_bg).fg(theme.highlight_fg));

    state
        .theme_popup_list_state
        .select(Some(state.theme_popup_index));
    frame.render_stateful_widget(list, chunks[1], &mut state.theme_popup_list_state);

    let hint = "↑/↓: Navigate | Enter: Apply | Esc: Cancel";
    frame.render_widget(
        Paragraph::new(hint)
            .style(Style::new().fg(theme.hint_fg))
            .alignment(Alignment::Center),
        chunks[2],
    );
}

pub fn render_import_export_popup(frame: &mut Frame, state: &mut AppState, area: Rect) {
    let theme = &state.current_theme;
    let is_export = state.export_popup_open;
    let has_preview = state.import_preview.is_some();

    let title = if is_export { "Export" } else { "Import" };
    let popup_width = 60u16;

    let has_path = !state.import_export_file_path.is_empty();

    let mode_height = if !is_export { 3 } else { 0 };
    let preview_height = if has_preview && !is_export { 4 } else { 0 };
    let hint_height = 1u16;
    let input_height = 3u16;

    let popup_height = input_height + mode_height + preview_height + hint_height;

    let centered = center_rect(popup_width, popup_height, area);
    frame.render_widget(Clear, centered);

    let mut constraints = vec![Constraint::Length(input_height)];
    if !is_export {
        constraints.push(Constraint::Length(mode_height));
    }
    if has_preview && !is_export {
        constraints.push(Constraint::Length(preview_height));
    }
    constraints.push(Constraint::Length(hint_height));

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(centered);

    let input_text = Paragraph::new(Line::from(vec![
        Span::styled("File: ", Style::default().fg(theme.input_text)),
        Span::styled(
            format!("{}▋", state.import_export_file_path),
            Style::default().fg(theme.input_text),
        ),
    ]))
    .block(
        Block::bordered()
            .title(format!("[{}]", title))
            .border_type(BorderType::Rounded)
            .border_style(Style::new().fg(theme.popup_border)),
    );
    frame.render_widget(input_text, chunks[0]);

    let mut chunk_idx = 1;

    if !is_export {
        let merge_style = if state.import_mode_index == 0 {
            Style::default()
                .fg(theme.highlight_fg)
                .bg(theme.highlight_bg)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.hint_fg)
        };

        let replace_style = if state.import_mode_index == 1 {
            Style::default()
                .fg(theme.highlight_fg)
                .bg(theme.highlight_bg)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.hint_fg)
        };

        let mode_text = Line::from(vec![
            Span::styled("(1) Merge ", merge_style),
            Span::raw(" | "),
            Span::styled("(2) Replace ", replace_style),
        ]);

        frame.render_widget(
            Paragraph::new(mode_text).alignment(Alignment::Center),
            chunks[chunk_idx],
        );
        chunk_idx += 1;

        if has_preview {
            let preview = state.import_preview.as_ref().unwrap();
            let preview_lines = vec![
                Line::from(vec![Span::styled(
                    "Preview: ",
                    Style::default().add_modifier(Modifier::BOLD),
                )]),
                Line::from(vec![Span::raw(format!(
                    "  + {} commands, + {} collections",
                    preview.new_commands, preview.new_collections
                ))]),
                Line::from(vec![Span::raw(format!(
                    "  ~ {} duplicates (skipped)",
                    preview.duplicates
                ))]),
            ];
            frame.render_widget(
                Paragraph::new(preview_lines).alignment(Alignment::Left),
                chunks[chunk_idx],
            );
            chunk_idx += 1;
        }
    }

    let hint = if is_export {
        if has_path {
            "Enter: Export | Esc: Cancel"
        } else {
            "Type path | Enter: Export | Esc: Cancel"
        }
    } else if has_preview {
        "Enter: Import | Esc: Cancel"
    } else if has_path {
        "Enter: Preview | ↑/↓: Mode | Esc: Cancel"
    } else {
        "Type path | Enter: Preview | Esc: Cancel"
    };

    frame.render_widget(
        Paragraph::new(hint)
            .style(Style::new().fg(theme.hint_fg))
            .alignment(Alignment::Center),
        chunks[chunk_idx],
    );
}

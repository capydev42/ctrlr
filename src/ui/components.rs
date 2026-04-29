use std::collections::HashSet;

use ratatui::{
    layout::Alignment,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::app::{ActivePane, ViewMode};
use crate::ui::theme::Theme;

const MAX_VISIBLE_TAGS: usize = 3;

pub fn tag_span<'a>(tag: &'a str, theme: &Theme) -> Span<'a> {
    Span::styled(
        format!("[{}]", tag),
        Style::new().fg(theme.tag_fg).bg(theme.tag_bg),
    )
}

pub fn tags_overflow_span(overflow: usize, theme: &Theme) -> Span<'static> {
    Span::styled(
        format!("+{} more", overflow),
        Style::new().fg(theme.hint_fg).italic(),
    )
}

pub fn command_with_right_tags<'a>(
    cmd_text: &'a str,
    cmd_indices: Option<&HashSet<usize>>,
    tags: &'a [String],
    available_width: u16,
    theme: &Theme,
) -> Line<'a> {
    let tags_width: usize = tags
        .iter()
        .take(MAX_VISIBLE_TAGS)
        .map(|t| t.len() + 4)
        .sum::<usize>()
        + if tags.len() > MAX_VISIBLE_TAGS {
            format!("+{} more", tags.len() - MAX_VISIBLE_TAGS).len() + 1
        } else {
            0
        };

    let cmd_width = available_width as isize - tags_width as isize - 1;
    let cmd_width = cmd_width.max(5) as u16;

    let mut line = Line::default();

    if let Some(indices) = cmd_indices {
        let truncated: String = cmd_text.chars().take(cmd_width as usize).collect();
        let chars: Vec<char> = truncated.chars().collect();
        let mut char_idx = 0;

        for c in chars {
            let idx_in_truncated = indices
                .iter()
                .any(|&i| i >= char_idx && i < char_idx + c.len_utf8());

            if idx_in_truncated {
                line.spans.push(Span::styled(
                    c.to_string(),
                    Style::new().fg(theme.match_highlight_fg).bold(),
                ));
            } else {
                line.spans.push(Span::raw(c.to_string()));
            }
            char_idx += c.len_utf8();
        }

        if cmd_text.chars().count() > cmd_width as usize {
            line.spans.push(Span::raw("…"));
        }
    } else {
        let truncated: String = cmd_text.chars().take(cmd_width as usize).collect();
        line.spans.push(Span::raw(truncated));
        if cmd_text.chars().count() > cmd_width as usize {
            line.spans.push(Span::raw("…"));
        }
    }

    let actual_cmd_len = line.spans.iter().fold(0usize, |acc, s| acc + s.width());
    let right_padding = (available_width as usize).saturating_sub(tags_width + actual_cmd_len + 1);
    if right_padding > 0 {
        for _ in 0..right_padding {
            line.spans.push(Span::raw(" "));
        }
    }

    for tag in tags.iter().take(MAX_VISIBLE_TAGS) {
        line.spans.push(tag_span(tag, theme));
        line.spans.push(Span::raw(" "));
    }

    if tags.len() > MAX_VISIBLE_TAGS {
        let overflow = tags.len() - MAX_VISIBLE_TAGS;
        line.spans.push(tags_overflow_span(overflow, theme));
    }

    line
}

pub fn render_search_bar(
    frame: &mut ratatui::Frame,
    state: &crate::app::AppState,
    area: ratatui::layout::Rect,
) {
    use ratatui::widgets::{Block, BorderType};

    let theme = &state.current_theme;
    let cursor = if state.active_pane == ActivePane::Search {
        "▋"
    } else {
        ""
    };
    let search_text = format!("{}{}", state.search_query, cursor);
    let search_border_color = if state.active_pane == ActivePane::Search {
        theme.focus_border
    } else {
        theme.unfocus_border
    };

    frame.render_widget(
        Paragraph::new(search_text).block(
            Block::bordered()
                .title(if state.active_pane == ActivePane::Search {
                    "[Search]"
                } else {
                    "Search"
                })
                .border_type(BorderType::Rounded)
                .border_style(Style::new().fg(search_border_color)),
        ),
        area,
    );
}

pub fn render_tabs(
    frame: &mut ratatui::Frame,
    state: &crate::app::AppState,
    area: ratatui::layout::Rect,
) {
    use ratatui::widgets::Paragraph;

    let theme = &state.current_theme;
    let history_count = state.commands.len();
    let favorites_count = state.commands.iter().filter(|c| c.favorite).count();
    let collections_count = state.collections.len();

    let tab_history = format!("1 History ({})", history_count);
    let tab_favorites = format!("2 Favorites ({})", favorites_count);
    let tab_collections = format!("3 Collections ({})", collections_count);

    let line = Line::from(vec![
        tab(&tab_history, state.view_mode == ViewMode::History, theme),
        Span::raw("   "),
        tab(
            &tab_favorites,
            state.view_mode == ViewMode::Favorites,
            theme,
        ),
        Span::raw("   "),
        tab(
            &tab_collections,
            state.view_mode == ViewMode::Collections,
            theme,
        ),
    ]);

    frame.render_widget(Paragraph::new(line).alignment(Alignment::Center), area);
}

fn tab<'a>(label: &str, active: bool, theme: &Theme) -> Span<'a> {
    if active {
        Span::styled(
            format!(" {} ", label),
            Style::new()
                .fg(theme.tab_active_fg)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        )
    } else {
        Span::styled(
            format!(" {} ", label),
            Style::new().fg(theme.tab_inactive_fg),
        )
    }
}

pub fn render_footer(
    frame: &mut ratatui::Frame,
    state: &crate::app::AppState,
    area: ratatui::layout::Rect,
) {
    use ratatui::widgets::Paragraph;

    let footer_text = if let Some(msg) = &state.status_message {
        msg.clone()
    } else {
        match state.view_mode {
            ViewMode::History | ViewMode::Favorites => {
                match state.active_pane {
                    ActivePane::Search => {
                        format!("? Help | 1: History | 2: Favorites | 3: Collections | Ctrl+T: Theme ({}) | /: Search | Backspace: Delete | ↑/↓: Navigate | Enter: Select ", state.current_theme.name())
                    }
                    ActivePane::History => {
                        format!("? Help | 1: History | 2: Favorites | 3: Collections | Ctrl+T: Theme ({}) | c: Add to Collection | /: Search | d: Details | t: Tag | j/k or ↑/↓: Navigate | f: Favorite | Enter: Select | Esc: Exit ", state.current_theme.name())
                    }
                    _ => "".into(),
                }
            }
            ViewMode::Collections => {
                match state.active_pane {
                    ActivePane::CollectionsList => {
                        "? Help | j/k or ↑/↓: Navigate | Enter: Show commands | n: New | e: Edit | d: Delete | Tab: Switch pane ".into()
                    }
                    ActivePane::CollectionItems => {
                        "? Help | j/k or ↑/↓: Navigate | Enter: Select | c: Add | d: Details | r: Remove | Tab: Switch pane ".into()
                    }
                    ActivePane::Search => {
                        "? Help | j/k: Navigate | Backspace: Delete | Enter: Select | 1/2/3: Switch view ".into()
                    }
                    ActivePane::History => {
                        "? Help | j/k: Navigate | Enter: Select | 1/2/3: Switch view ".into()
                    }
                }
            }
        }
    };

    frame.render_widget(Paragraph::new(footer_text), area);
}

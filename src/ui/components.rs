use std::collections::HashSet;

use ratatui::{
    layout::Alignment,
    style::{Color, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::app::{ActivePane, ViewMode};

pub fn highlight_text<'a>(text: &'a str, indices: &HashSet<usize>) -> Line<'a> {
    let chars: Vec<char> = text.chars().collect();
    let mut spans = Vec::new();
    let mut in_match = false;
    for (i, c) in chars.iter().enumerate() {
        if indices.contains(&i) {
            spans.push(Span::styled(
                c.to_string(),
                Style::new().fg(Color::Yellow).bold(),
            ));
            in_match = true;
        } else if in_match {
            spans.push(Span::raw(c.to_string()));
            in_match = false;
        } else {
            spans.push(Span::raw(c.to_string()));
        }
    }
    Line::from(spans)
}

pub fn render_search_bar(
    frame: &mut ratatui::Frame,
    state: &crate::app::AppState,
    area: ratatui::layout::Rect,
) {
    use ratatui::widgets::{Block, BorderType};

    let cursor = if state.active_pane == ActivePane::Search {
        "▋"
    } else {
        ""
    };
    let search_text = format!("Search: {}{}", state.search_query, cursor);
    let search_border_color = if state.active_pane == ActivePane::Search {
        Color::Yellow
    } else {
        Color::DarkGray
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

    let history_tab = if state.view_mode == ViewMode::History {
        Span::styled(
            "● History ",
            Style::new().fg(Color::Yellow).bg(Color::Blue).bold(),
        )
    } else {
        Span::raw("○ History ")
    };

    let favorites_tab = if state.view_mode == ViewMode::Favorites {
        Span::styled(
            "● Favorites ",
            Style::new().fg(Color::Yellow).bg(Color::Blue).bold(),
        )
    } else {
        Span::raw("○ Favorites ")
    };

    let collections_tab = if state.view_mode == ViewMode::Collections {
        Span::styled(
            "● Collections",
            Style::new().fg(Color::Yellow).bg(Color::Blue).bold(),
        )
    } else {
        Span::raw("○ Collections")
    };

    let line = Line::from(vec![history_tab, favorites_tab, collections_tab]);

    frame.render_widget(Paragraph::new(line).alignment(Alignment::Center), area);
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
                        " 1: History | 2: Favorites | 3: Collections | /: Search | Backspace: Delete | ↑/↓: Navigate | Enter: Select ".into()
                    }
                    ActivePane::History => {
                        " 1: History | 2: Favorites | 3: Collections | c: Add to Collection | /: Search | d: Details | t: Tag | j/k or ↑/↓: Navigate | f: Favorite | Enter: Select | Esc: Exit ".into()
                    }
                    _ => "".into(),
                }
            }
            ViewMode::Collections => {
                match state.active_pane {
                    ActivePane::CollectionsList => {
                        " j/k or ↑/↓: Navigate | Enter: Show commands | n: New | e: Edit | d: Delete | Tab: Switch pane ".into()
                    }
                    ActivePane::CollectionItems => {
                        " j/k or ↑/↓: Navigate | Enter: Select | c: Add | d: Details | r: Remove | Tab: Switch pane ".into()
                    }
                    ActivePane::Search => {
                        " j/k: Navigate | Backspace: Delete | Enter: Select | 1/2/3: Switch view ".into()
                    }
                    ActivePane::History => {
                        " j/k: Navigate | Enter: Select | 1/2/3: Switch view ".into()
                    }
                }
            }
        }
    };

    frame.render_widget(Paragraph::new(footer_text), area);
}

use std::collections::HashSet;

use ratatui::{
    layout::Alignment,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::app::{ActivePane, ViewMode};
use crate::keymap::{KeyAction, KeyContext};
use crate::ui::theme::Theme;

const MAX_VISIBLE_TAGS: usize = 3;

pub fn tag_span<'a>(tag: &'a str, theme: &Theme) -> Span<'a> {
    Span::styled(
        format!("[{}]", tag),
        Style::new().fg(theme.tag_fg).bg(theme.tag_bg),
    )
}

/// A run of command text, highlighted when it matched the search query.
fn matched_span(text: String, matched: bool, theme: &Theme) -> Span<'static> {
    if matched {
        Span::styled(text, Style::new().fg(theme.match_highlight_fg).bold())
    } else {
        Span::raw(text)
    }
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
        let mut char_idx = 0;

        // Consecutive characters that share a highlight state go into one
        // span. Emitting one span per character costs ~40 allocations a row,
        // and the history list rebuilds every row on every frame — with a
        // thousand commands that alone is tens of milliseconds per redraw.
        let mut run = String::new();
        let mut run_matched = false;
        for c in cmd_text.chars().take(cmd_width as usize) {
            // A linear scan beats hashing here: a query matches a handful of
            // positions, so the set is tiny and `contains` measured slower.
            let matched = indices
                .iter()
                .any(|&i| i >= char_idx && i < char_idx + c.len_utf8());

            if !run.is_empty() && matched != run_matched {
                line.spans
                    .push(matched_span(std::mem::take(&mut run), run_matched, theme));
            }
            run_matched = matched;
            run.push(c);
            char_idx += c.len_utf8();
        }
        if !run.is_empty() {
            line.spans.push(matched_span(run, run_matched, theme));
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
        // One span, not one per space: this is most of a row's width.
        line.spans.push(Span::raw(" ".repeat(right_padding)));
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
    state: &mut crate::app::AppState,
    area: ratatui::layout::Rect,
) {
    use ratatui::widgets::{Block, BorderType};

    state.hit.search = area;
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
    state: &mut crate::app::AppState,
    area: ratatui::layout::Rect,
) {
    use ratatui::widgets::Paragraph;

    let theme = &state.current_theme;
    let history_count = state.commands.len();
    let favorites_count = state.commands.iter().filter(|c| c.favorite).count();
    let collections_count = state.collections.len();

    let tab_history = format!("Alt+1 History ({})", history_count);
    let tab_favorites = format!("Alt+2 Favorites ({})", favorites_count);
    let tab_collections = format!("Alt+3 Collections ({})", collections_count);

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

    // The line is centred, so walk the spans from the centred start to learn
    // where each tab actually landed. Spans 0, 2 and 4 are the tabs; 1 and 3
    // are the separators.
    let total = line.width() as u16;
    let mut x = area.x + area.width.saturating_sub(total) / 2;
    for (i, span) in line.spans.iter().enumerate() {
        let width = span.width() as u16;
        if i % 2 == 0 {
            state.hit.tabs[i / 2] = ratatui::layout::Rect::new(x, area.y, width, area.height);
        }
        x = x.saturating_add(width);
    }

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

    // An active scope changes what the list contains, so it stays visible
    // instead of living in the transient status message.
    let scope = if state.scope_to_cwd {
        format!("[{}] ", state.cwd_display())
    } else {
        String::new()
    };

    let footer_text = if let Some(msg) = &state.status_message {
        format!("{}{}", scope, msg)
    } else {
        let context = crate::input::pane_context(state);
        // Which actions each pane advertises stays an editorial choice — the
        // footer is one line and the help popup lists everything. The *keys*
        // are looked up, so a rebind shows here without anyone remembering.
        let items: &[(KeyAction, &str)] = match (&state.view_mode, &state.active_pane) {
            (ViewMode::Collections, ActivePane::CollectionsList) => &[
                (KeyAction::ShowHelp, "Help"),
                (KeyAction::NavigateDown, "Navigate"),
                (KeyAction::Execute, "Show commands"),
                (KeyAction::NewCollection, "New"),
                (KeyAction::EditCollection, "Rename"),
                (KeyAction::DeleteCollection, "Delete"),
                (KeyAction::SwitchPane, "Switch pane"),
            ],
            (ViewMode::Collections, ActivePane::CollectionItems) => &[
                (KeyAction::ShowHelp, "Help"),
                (KeyAction::NavigateDown, "Navigate"),
                (KeyAction::Execute, "Select"),
                (KeyAction::AddToCollection, "Add"),
                (KeyAction::ToggleDetails, "Details"),
                (KeyAction::RemoveFromCollection, "Remove"),
                (KeyAction::SwitchPane, "Switch pane"),
            ],
            (ViewMode::Collections, _) => &[
                (KeyAction::ShowHelp, "Help"),
                (KeyAction::NavigateDown, "Navigate"),
                (KeyAction::Execute, "Select"),
            ],
            (_, ActivePane::Search) => &[
                (KeyAction::ShowHelp, "Help"),
                (KeyAction::ChangeTheme, "Theme"),
                (KeyAction::ScopeCwd, "Here"),
                (KeyAction::FocusSearch, "Search"),
                (KeyAction::NavigateDown, "Navigate"),
                (KeyAction::Execute, "Select"),
            ],
            _ => &[
                (KeyAction::ShowHelp, "Help"),
                (KeyAction::ChangeTheme, "Theme"),
                (KeyAction::AddToCollection, "Add to Collection"),
                (KeyAction::ScopeCwd, "Here"),
                (KeyAction::ToggleDetails, "Details"),
                (KeyAction::EditTags, "Tag"),
                (KeyAction::NavigateDown, "Navigate"),
                (KeyAction::ToggleFavorite, "Favorite"),
                (KeyAction::Execute, "Select"),
                (KeyAction::Cancel, "Exit"),
            ],
        };
        // The active theme's name rides along on its own key hint.
        let theme_label = format!("Theme ({})", state.current_theme.name());
        let items: Vec<(KeyAction, &str)> = items
            .iter()
            .map(|(a, label)| {
                if *a == KeyAction::ChangeTheme {
                    (*a, theme_label.as_str())
                } else {
                    (*a, *label)
                }
            })
            .collect();
        format!("{}{} ", scope, hint_line(state, context, &items))
    };

    frame.render_widget(Paragraph::new(footer_text), area);
}

/// `"? Help | Ctrl+T: Theme | ..."`, with the keys read out of the live keymap.
///
/// Only the first key for each action: the footer is one line, and the help
/// popup is where the alternatives belong. An action with no key in this
/// context is dropped rather than shown keyless.
pub fn hint_line(
    state: &crate::app::AppState,
    context: KeyContext,
    items: &[(KeyAction, &str)],
) -> String {
    items
        .iter()
        .filter_map(|(action, label)| {
            let key = state
                .keymap
                .keys_for(context, *action)
                .into_iter()
                .next()
                .or_else(|| {
                    context
                        .falls_through_to_global()
                        .then(|| {
                            state
                                .keymap
                                .keys_for(KeyContext::Global, *action)
                                .into_iter()
                                .next()
                        })
                        .flatten()
                })?;
            Some(format!("{}: {}", key, label))
        })
        .collect::<Vec<_>>()
        .join(" | ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn theme() -> Theme {
        Theme::default()
    }

    /// The rendered text must not depend on how the spans are grouped.
    fn rendered(line: &Line) -> String {
        line.spans.iter().map(|s| s.content.as_ref()).collect()
    }

    #[test]
    fn test_components_highlight_runs_are_coalesced() {
        let indices: HashSet<usize> = [0, 1, 2, 6, 7].into_iter().collect();
        let line = command_with_right_tags("git status", Some(&indices), &[], 40, &theme());

        // g-i-t matched, " st" not, "at" matched, "us" not — four runs, not
        // ten single-character spans.
        let text_spans: Vec<&str> = line
            .spans
            .iter()
            .map(|s| s.content.as_ref())
            .take(4)
            .collect();
        assert_eq!(text_spans, vec!["git", " st", "at", "us"]);
    }

    #[test]
    fn test_components_highlighted_style_follows_the_match() {
        let indices: HashSet<usize> = [0, 1, 2].into_iter().collect();
        let line = command_with_right_tags("git status", Some(&indices), &[], 40, &theme());

        assert_eq!(line.spans[0].content.as_ref(), "git");
        assert_eq!(line.spans[0].style.fg, Some(theme().match_highlight_fg));
        assert_eq!(line.spans[1].style.fg, None, "unmatched text is unstyled");
    }

    #[test]
    fn test_components_text_matches_the_unhighlighted_render() {
        let indices: HashSet<usize> = [1, 4, 5].into_iter().collect();
        let with = command_with_right_tags("cargo build", Some(&indices), &[], 40, &theme());
        let without = command_with_right_tags("cargo build", None, &[], 40, &theme());
        assert_eq!(rendered(&with), rendered(&without));
    }

    #[test]
    fn test_components_padding_is_a_single_span() {
        let line = command_with_right_tags("ls", None, &[], 40, &theme());
        // Command, then one padding span — not one span per column.
        assert_eq!(line.spans.len(), 2);
        assert_eq!(line.spans[1].content.as_ref().trim(), "");
        assert_eq!(line.width(), 39);
    }

    #[test]
    fn test_components_long_command_is_truncated_with_ellipsis() {
        let long = "x".repeat(200);
        let indices: HashSet<usize> = [0].into_iter().collect();
        let line = command_with_right_tags(&long, Some(&indices), &[], 40, &theme());
        assert!(rendered(&line).contains('…'));
        assert!(line.width() <= 40);
    }

    #[test]
    fn test_components_multibyte_command_is_not_split() {
        let indices: HashSet<usize> = [0].into_iter().collect();
        let line = command_with_right_tags("écho héllo", Some(&indices), &[], 40, &theme());
        assert!(rendered(&line).starts_with("écho héllo"));
    }
}

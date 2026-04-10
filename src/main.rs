use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    DefaultTerminal, Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, BorderType, Clear, List, ListItem, ListState, Paragraph, Scrollbar,
        ScrollbarOrientation, Wrap,
    },
};

mod cli;
mod history;
mod state;
mod storage;

use state::{ActivePane, AppState, CollectionInputMode, Command, InputMode, ViewMode};
use std::io;
use std::time::Duration;

fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;
    cli::run()
}

pub fn run_tui(output_file: Option<String>) -> color_eyre::Result<Option<String>> {
    if !atty::is(atty::Stream::Stdin) || !atty::is(atty::Stream::Stdout) {
        eprintln!("Error: ctrlr must be run from a terminal. Stdin or stdout is not a TTY.");
        return Ok(None);
    }
    let mut terminal = ratatui::init();
    let result = app(&mut terminal, output_file.clone());
    ratatui::restore();

    match result {
        Ok(Some(cmd)) => {
            if let Some(path) = output_file {
                match std::fs::write(&path, &cmd) {
                    Ok(()) => Ok(Some(cmd)),
                    Err(e) => {
                        eprintln!("Failed to write output file: {}", e);
                        std::process::exit(1);
                    }
                }
            } else {
                Ok(Some(cmd))
            }
        }
        Ok(None) => {
            if let Some(path) = output_file {
                if let Err(e) = std::fs::write(&path, "") {
                    eprintln!("Failed to write output file: {}", e);
                }
                std::process::exit(1);
            }
            Ok(None)
        }
        Err(e) => {
            if let Some(path) = output_file {
                let _ = std::fs::write(&path, "");
            }
            Err(color_eyre::Report::new(e))
        }
    }
}

fn app(terminal: &mut DefaultTerminal, _output_file: Option<String>) -> io::Result<Option<String>> {
    let mut db = match storage::init_db() {
        Ok(conn) => Some(conn),
        Err(e) => {
            eprintln!("Failed to initialize database: {}", e);
            None
        }
    };
    let mut commands = history::load_history();
    commands = history::deduplicate(commands);
    commands.reverse();

    if let Some(ref mut conn) = db {
        let cmd_refs: Vec<(&str, String)> = commands
            .iter()
            .map(|c| (c.text.as_str(), c.id.clone()))
            .collect();
        if let Err(e) = storage::commands::ensure_commands_exist(conn, &cmd_refs) {
            eprintln!("Failed to save commands: {}", e);
        }

        for cmd in &mut commands {
            if let Some(meta) = storage::load_metadata(conn, &cmd.text) {
                cmd.favorite = meta.favorite;
                cmd.use_count = meta.use_count;
                cmd.last_used = meta.last_used;
            }
            let tags = storage::load_tags(conn, &cmd.text);
            if !tags.is_empty() {
                cmd.tags = tags;
            }
            let collections = storage::collections::get_collections_for_command(conn, &cmd.text)
                .unwrap_or_default();
            if !collections.is_empty() {
                cmd.collection_ids = collections;
            }
        }
    }

    let mut state = AppState::new(commands, db);
    state.load_collections();
    let mut list_state = ListState::default();
    let mut collection_list_state = ListState::default();
    let mut collection_items_list_state = ListState::default();
    let mut tag_popup_list_state = ListState::default();
    let mut collection_popup_list_state = ListState::default();
    list_state.select(Some(0));
    collection_list_state.select(Some(0));
    collection_items_list_state.select(Some(0));
    tag_popup_list_state.select(Some(0));
    collection_popup_list_state.select(Some(0));

    loop {
        if let Some(ts) = state.status_timestamp {
            let should_clear =
                state.status_message.is_some() && ts.elapsed() > Duration::from_secs(2);
            if should_clear {
                state.status_message = None;
                state.status_timestamp = None;
            }
        }

        terminal.draw(|f| {
            render(
                f,
                &state,
                &mut list_state,
                &mut collection_list_state,
                &mut collection_items_list_state,
                &mut tag_popup_list_state,
                &mut collection_popup_list_state,
            )
        })?;
        if let Event::Key(key) = crossterm::event::read()? {
            if key.code == KeyCode::Esc
                && state.input_mode != InputMode::TagInput
                && state.input_mode != InputMode::CollectionInput
                && state.handle_esc()
            {
                break Ok(None);
            }
            if let Some(cmd) =
                handle_key(&mut state, &mut list_state, &mut collection_list_state, key)
            {
                break Ok(Some(cmd));
            }
        }
    }
}

fn handle_key(
    state: &mut AppState,
    list_state: &mut ListState,
    _collection_list_state: &mut ListState,
    key: KeyEvent,
) -> Option<String> {
    if state.input_mode == InputMode::CollectionInput {
        return handle_collection_input(state, key);
    }

    if state.input_mode == InputMode::TagInput {
        match (key.code, key.modifiers) {
            (KeyCode::Left, _) => {
                let tags = state.selected_command_tags();
                if !tags.is_empty() {
                    if let Some(idx) = state.tag_cursor_index {
                        state.tag_cursor_index =
                            Some(if idx == 0 { tags.len() - 1 } else { idx - 1 });
                    } else {
                        state.tag_cursor_index = Some(tags.len() - 1);
                    }
                }
            }
            (KeyCode::Right, _) => {
                let tags = state.selected_command_tags();
                if let Some(idx) = state.tag_cursor_index {
                    state.tag_cursor_index = Some(if idx + 1 >= tags.len() { 0 } else { idx + 1 });
                }
            }
            (KeyCode::Backspace, _) => {
                if let Some(idx) = state.tag_cursor_index {
                    let mut tags = state.selected_command_tags();
                    if idx < tags.len() {
                        tags.remove(idx);
                        let new_len = tags.len();
                        state.set_tags(tags);
                        state.tag_cursor_index = if new_len == 0 {
                            None
                        } else if idx >= new_len {
                            Some(new_len - 1)
                        } else {
                            Some(idx)
                        };
                    }
                } else if state.tag_input.is_empty() {
                    let tags = state.selected_command_tags();
                    if !tags.is_empty() {
                        state.tag_cursor_index = Some(tags.len() - 1);
                    }
                } else {
                    state.tag_input.pop();
                    state.tag_selected_index = 0;
                }
            }
            (KeyCode::Char(c), KeyModifiers::NONE) => {
                if state.tag_cursor_index.is_some() {
                    state.tag_cursor_index = None;
                }
                state.tag_input.push(c);
            }
            (KeyCode::Tab, _) => {
                if state.tag_cursor_index.is_none() {
                    let suggestions = state.filtered_tags();
                    if !suggestions.is_empty() && state.tag_selected_index < suggestions.len() {
                        let tag = suggestions[state.tag_selected_index].clone();
                        state.apply_selected_tag(tag);
                        state.tag_selected_index = 0;
                    }
                }
            }
            (KeyCode::Up, _) => {
                state.tag_selected_index = state.tag_selected_index.saturating_sub(1);
            }
            (KeyCode::Down, _) => {
                let suggestions = state.filtered_tags();
                let show_create = !state.tag_input.trim().is_empty()
                    && !suggestions
                        .iter()
                        .any(|t| t.to_lowercase() == state.tag_input.trim().to_lowercase());

                let max_index = if show_create {
                    suggestions.len()
                } else {
                    suggestions.len().saturating_sub(1)
                };
                state.tag_selected_index = (state.tag_selected_index + 1).min(max_index);
            }
            (KeyCode::Enter, _) => {
                let search_text = state.tag_input.trim().to_string();
                let selected_idx = state.tag_selected_index;

                let suggestions = state.filtered_tags();

                if search_text.is_empty() {
                    state.tag_input.clear();
                    state.tag_cursor_index = None;
                    state.input_mode = InputMode::Normal;
                    state.tag_selected_index = 0;
                } else {
                    let search_lower = search_text.to_lowercase();
                    let exact_match = suggestions.iter().any(|t| t.to_lowercase() == search_lower);

                    let mut new_tag: Option<String> = None;

                    if selected_idx < suggestions.len() {
                        new_tag = Some(suggestions[selected_idx].clone());
                    } else if !exact_match {
                        new_tag = Some(search_text.clone());
                    }

                    if let Some(tag) = new_tag {
                        let mut tags = state.selected_command_tags();
                        if !tags.contains(&tag) {
                            tags.push(tag);
                            tags.sort();
                            state.set_tags(tags);
                        }
                    }

                    state.tag_input.clear();
                    state.tag_cursor_index = None;
                    state.input_mode = InputMode::Normal;
                    state.tag_selected_index = 0;
                }
            }
            (KeyCode::Esc, _) => {
                state.input_mode = InputMode::Normal;
                state.tag_input.clear();
                state.tag_selected_index = 0;
                state.tag_cursor_index = None;
            }
            _ => {}
        }
        return None;
    }

    match (key.code, key.modifiers) {
        (KeyCode::Tab, _) => {
            state.switch_pane();
            return None;
        }
        (KeyCode::Char('1'), KeyModifiers::NONE) => {
            state.view_mode = ViewMode::History;
            state.active_pane = ActivePane::History;
            state.filter_commands();
            return None;
        }
        (KeyCode::Char('2'), KeyModifiers::NONE) => {
            state.view_mode = ViewMode::Favorites;
            state.active_pane = ActivePane::History;
            state.filter_commands();
            return None;
        }
        (KeyCode::Char('3'), KeyModifiers::NONE) => {
            state.view_mode = ViewMode::Collections;
            state.active_pane = ActivePane::CollectionsList;
            state.load_collection_commands();
            state.filter_commands();
            return None;
        }
        (KeyCode::Char('c'), KeyModifiers::NONE) => {
            if state.active_pane == ActivePane::Search {
                state.add_to_search('c');
            } else {
                let has_selection = match state.view_mode {
                    ViewMode::Collections => {
                        matches!(state.active_pane, ActivePane::CollectionItems)
                            && !state.collection_commands.is_empty()
                    }
                    _ => !state.filtered.is_empty(),
                };
                if has_selection {
                    state.collection_input_mode = CollectionInputMode::AddToCollection;
                    state.input_mode = InputMode::CollectionInput;
                }
            }
            return None;
        }
        (KeyCode::Enter, _) => {
            if state.view_mode == ViewMode::Collections {
                match state.active_pane {
                    ActivePane::CollectionsList => {
                        state.load_collection_commands();
                        state.active_pane = ActivePane::CollectionItems;
                        state.selected_index = 0;
                        return None;
                    }
                    ActivePane::CollectionItems => {
                        let cmd = state.filtered.get(state.selected_index).cloned();
                        if let Some(ref c) = cmd {
                            state.mark_executed_for_text(&c.text);
                        }
                        return cmd.map(|c| c.text);
                    }
                    _ => return None,
                }
            }
            let cmd = state.selected_command();
            state.mark_executed();
            return cmd;
        }
        _ => {}
    }

    match (key.code, key.modifiers) {
        (KeyCode::Up, _) => match state.view_mode {
            ViewMode::Collections => match state.active_pane {
                ActivePane::CollectionsList => state.navigate_collection_up(),
                ActivePane::CollectionItems => {
                    state.navigate_up();
                    list_state.select(Some(state.selected_index));
                }
                _ => {
                    if state.active_pane == ActivePane::Search {
                        state.active_pane = ActivePane::History;
                    }
                    state.navigate_up();
                    list_state.select(Some(state.selected_index));
                }
            },
            _ => {
                if state.active_pane == ActivePane::Search {
                    state.active_pane = ActivePane::History;
                }
                state.navigate_up();
                list_state.select(Some(state.selected_index));
            }
        },
        (KeyCode::Down, _) => match state.view_mode {
            ViewMode::Collections => match state.active_pane {
                ActivePane::CollectionsList => state.navigate_collection_down(),
                ActivePane::CollectionItems => {
                    state.navigate_down();
                    list_state.select(Some(state.selected_index));
                }
                _ => {
                    if state.active_pane == ActivePane::Search {
                        state.active_pane = ActivePane::History;
                    }
                    state.navigate_down();
                    list_state.select(Some(state.selected_index));
                }
            },
            _ => {
                if state.active_pane == ActivePane::Search {
                    state.active_pane = ActivePane::History;
                }
                state.navigate_down();
                list_state.select(Some(state.selected_index));
            }
        },
        (KeyCode::Esc, _) => {
            state.handle_esc();
        }
        _ => {}
    }

    match state.active_pane {
        ActivePane::Search => match (key.code, key.modifiers) {
            (KeyCode::Char(c), KeyModifiers::NONE) => {
                state.add_to_search(c);
            }
            (KeyCode::Backspace, _) => {
                state.remove_from_search();
            }
            _ => {}
        },
        ActivePane::History => match (key.code, key.modifiers) {
            (KeyCode::Char('/'), KeyModifiers::NONE) => {
                state.active_pane = ActivePane::Search;
            }
            (KeyCode::Char('t'), KeyModifiers::NONE) => {
                state.input_mode = InputMode::TagInput;
                state.tag_input = String::new();
                state.tag_selected_index = 0;
                state.tag_cursor_index = None;
            }
            (KeyCode::Char('j'), KeyModifiers::NONE) => {
                state.navigate_down();
                list_state.select(Some(state.selected_index));
            }
            (KeyCode::Char('k'), KeyModifiers::NONE) => {
                state.navigate_up();
                list_state.select(Some(state.selected_index));
            }
            (KeyCode::Char('f'), KeyModifiers::NONE) => {
                state.toggle_favorite();
            }
            (KeyCode::Char('d'), KeyModifiers::NONE) => {
                state.show_details = !state.show_details;
            }
            _ => {}
        },
        ActivePane::CollectionsList => match (key.code, key.modifiers) {
            (KeyCode::Char('/'), KeyModifiers::NONE) => {
                state.active_pane = ActivePane::Search;
            }
            (KeyCode::Char('n'), KeyModifiers::NONE) => {
                state.collection_input_mode = CollectionInputMode::NewCollection;
                state.collection_input_text.clear();
                state.input_mode = InputMode::CollectionInput;
            }
            (KeyCode::Char('e'), KeyModifiers::NONE) => {
                if state.selected_collection().is_some() {
                    state.editing_collection_id = state.selected_collection().map(|c| c.id.clone());
                    state.collection_input_text = state
                        .selected_collection()
                        .map(|c| c.name.clone())
                        .unwrap_or_default();
                    state.collection_input_mode = CollectionInputMode::EditCollection;
                    state.input_mode = InputMode::CollectionInput;
                }
            }
            (KeyCode::Char('d'), KeyModifiers::NONE) => {
                state.delete_collection();
            }
            (KeyCode::Char('j'), KeyModifiers::NONE) => {
                state.navigate_collection_down();
            }
            (KeyCode::Char('k'), KeyModifiers::NONE) => {
                state.navigate_collection_up();
            }
            _ => {}
        },
        ActivePane::CollectionItems => match (key.code, key.modifiers) {
            (KeyCode::Char('/'), KeyModifiers::NONE) => {
                state.active_pane = ActivePane::Search;
            }
            (KeyCode::Char('d'), KeyModifiers::NONE) => {
                state.show_details = !state.show_details;
            }
            (KeyCode::Char('r'), KeyModifiers::NONE) => {
                if let Some(cmd) = state.filtered.get(state.selected_index) {
                    let text = cmd.text.clone();
                    state.remove_command_from_collection(&text);
                }
            }
            (KeyCode::Char('j'), KeyModifiers::NONE) => {
                state.navigate_down();
            }
            (KeyCode::Char('k'), KeyModifiers::NONE) => {
                state.navigate_up();
            }
            _ => {}
        },
    }
    None
}

fn handle_collection_input(state: &mut AppState, key: KeyEvent) -> Option<String> {
    match (key.code, key.modifiers) {
        (KeyCode::Char(c), KeyModifiers::NONE) => {
            if state.collection_input_mode == CollectionInputMode::AddToCollection {
                state.collection_input_text.push(c);
                state.collection_popup_index = 0;
            } else {
                state.collection_input_text.push(c);
            }
        }
        (KeyCode::Backspace, _) => {
            if state.collection_input_mode == CollectionInputMode::AddToCollection {
                if !state.collection_input_text.is_empty() {
                    state.collection_input_text.pop();
                    state.collection_popup_index = state.collection_popup_index.saturating_sub(1);
                }
            } else {
                state.collection_input_text.pop();
            }
        }
        (KeyCode::Up, _) => {
            if state.collection_input_mode == CollectionInputMode::AddToCollection {
                state.collection_popup_index = state.collection_popup_index.saturating_sub(1);
            }
        }
        (KeyCode::Down, _) => {
            if state.collection_input_mode == CollectionInputMode::AddToCollection {
                let search_text = &state.collection_input_text;
                let show_create = !search_text.is_empty()
                    && !state
                        .filtered_collections(search_text)
                        .iter()
                        .any(|c| c.name.to_lowercase() == search_text.to_lowercase());

                let filtered_count = state.filtered_collections(search_text).len();
                let max_index = if show_create {
                    filtered_count
                } else {
                    filtered_count.saturating_sub(1)
                };
                state.collection_popup_index = (state.collection_popup_index + 1).min(max_index);
            }
        }
        (KeyCode::Enter, _) => {
            match state.collection_input_mode {
                CollectionInputMode::NewCollection => {
                    if !state.collection_input_text.is_empty() {
                        state.create_collection(state.collection_input_text.clone());
                    }
                }
                CollectionInputMode::EditCollection => {
                    let id = state.editing_collection_id.clone();
                    let text = state.collection_input_text.clone();
                    if let (Some(id), false) = (id, text.is_empty()) {
                        state.rename_collection(&id, text);
                    }
                }
                CollectionInputMode::AddToCollection => {
                    let search_text = state.collection_input_text.trim().to_string();
                    let selected_idx = state.collection_popup_index;
                    let cmd_text = state.active_command().map(|c| c.text.clone());

                    let filtered = state.filtered_collections(&search_text);

                    if selected_idx < filtered.len() {
                        let col_id = filtered[selected_idx].id.clone();
                        if let Some(text) = cmd_text {
                            state.add_command_to_collection(&text, &col_id);
                        }
                    } else if !search_text.is_empty() && selected_idx == filtered.len() {
                        state.create_collection(search_text.clone());
                        let new_col_id = state
                            .collections
                            .iter()
                            .find(|c| c.name.to_lowercase() == search_text.to_lowercase())
                            .map(|c| c.id.clone());
                        if let (Some(col_id), Some(text)) = (new_col_id, cmd_text) {
                            state.add_command_to_collection(&text, &col_id);
                        }
                    }
                }
                CollectionInputMode::None => {}
            }
            state.input_mode = InputMode::Normal;
            state.collection_input_mode = CollectionInputMode::None;
            state.collection_input_text.clear();
            state.editing_collection_id = None;
        }
        (KeyCode::Esc, _) => {
            state.input_mode = InputMode::Normal;
            state.collection_input_mode = CollectionInputMode::None;
            state.collection_input_text.clear();
            state.editing_collection_id = None;
        }
        _ => {}
    }
    None
}

fn render(
    frame: &mut Frame,
    state: &AppState,
    list_state: &mut ListState,
    collection_list_state: &mut ListState,
    collection_items_list_state: &mut ListState,
    tag_popup_list_state: &mut ListState,
    collection_popup_list_state: &mut ListState,
) {
    let area = frame.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(2),
        ])
        .split(area);

    render_search_bar(frame, state, chunks[0]);
    render_tabs(frame, state, chunks[1]);
    render_footer(frame, state, chunks[3]);

    match state.view_mode {
        ViewMode::History | ViewMode::Favorites => {
            let (list_area, details_area) = if state.show_details {
                let content_chunks = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Percentage(65), Constraint::Percentage(35)])
                    .split(chunks[2]);
                (content_chunks[0], Some(content_chunks[1]))
            } else {
                (chunks[2], None)
            };
            render_history_list(frame, state, list_state, list_area);
            if let Some(details_area) = details_area {
                render_details(frame, state, details_area);
            }
        }
        ViewMode::Collections => {
            render_collections_view(
                frame,
                state,
                collection_list_state,
                collection_items_list_state,
                chunks[2],
            );
        }
    }

    if state.input_mode == InputMode::TagInput {
        render_tag_popup(frame, state, tag_popup_list_state, area);
    }

    if state.input_mode == InputMode::CollectionInput {
        render_collection_popup(frame, state, collection_popup_list_state, area);
    }
}

fn render_search_bar(frame: &mut Frame, state: &AppState, area: Rect) {
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

fn render_tabs(frame: &mut Frame, state: &AppState, area: Rect) {
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

fn render_history_list(
    frame: &mut Frame,
    state: &AppState,
    list_state: &mut ListState,
    area: Rect,
) {
    let items: Vec<ListItem> = if state.filtered.is_empty() {
        vec![ListItem::new("No results found")]
    } else {
        state
            .filtered
            .iter()
            .enumerate()
            .map(|(idx, cmd)| {
                let tags = cmd
                    .tags
                    .iter()
                    .map(|t| format!("#{}", t))
                    .collect::<Vec<_>>()
                    .join(" ");

                let favorite_str = if cmd.favorite { "⭐" } else { " " };

                let mut spans = vec![Span::raw(format!("{:<2} ", favorite_str))];

                if let Some(Some(indices)) = state.matched_indices.get(idx) {
                    let chars: Vec<char> = cmd.text.chars().collect();
                    let mut in_match = false;
                    for (i, c) in chars.iter().enumerate() {
                        if indices.contains(&i) {
                            if !in_match {
                                spans.push(Span::styled(
                                    c.to_string(),
                                    Style::new().fg(Color::Yellow).bold(),
                                ));
                                in_match = true;
                            } else {
                                spans.push(Span::styled(
                                    c.to_string(),
                                    Style::new().fg(Color::Yellow).bold(),
                                ));
                            }
                        } else if in_match {
                            spans.push(Span::raw(c.to_string()));
                            in_match = false;
                        } else {
                            spans.push(Span::raw(c.to_string()));
                        }
                    }
                } else {
                    spans.push(Span::raw(&cmd.text));
                }

                spans.push(Span::raw(format!(" {}", tags)));

                ListItem::new(Line::from(spans))
            })
            .collect()
    };

    let history_border_color = if state.active_pane == ActivePane::History {
        Color::Yellow
    } else {
        Color::DarkGray
    };

    let list_title = match state.view_mode {
        ViewMode::History => {
            if state.active_pane == ActivePane::History {
                "[History]"
            } else {
                "History"
            }
        }
        ViewMode::Favorites => {
            if state.active_pane == ActivePane::History {
                "[Favorites]"
            } else {
                "Favorites"
            }
        }
        ViewMode::Collections => {
            if let Some(col) = state.selected_collection() {
                &col.name
            } else {
                "Commands"
            }
        }
    };

    let list = List::new(items)
        .block(
            Block::bordered()
                .title(list_title)
                .border_type(BorderType::Rounded)
                .border_style(Style::new().fg(history_border_color)),
        )
        .highlight_style(Style::default().bg(Color::Blue).fg(Color::White)) // pff overthink color choice ...
        .highlight_symbol("> ");

    frame.render_stateful_widget(list, area, list_state);
}

fn section(title: &str) -> Line<'_> {
    Line::from(Span::styled(
        format!("─ {} ─", title),
        Style::new().fg(Color::Blue).bold(),
    ))
}

fn render_details(frame: &mut Frame, state: &AppState, area: Rect) {
    if area.width < 5 || area.height < 3 {
        return;
    }

    if state.filtered.is_empty() {
        frame.render_widget(
            Paragraph::new("No command selected")
                .alignment(Alignment::Center)
                .block(Block::bordered().title("Details")),
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
            lines.push(Line::from(format!("#{}", tag)));
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
    let fav_text = if cmd.favorite { "⭐ yes" } else { "○ no" };
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

fn render_footer(frame: &mut Frame, state: &AppState, area: Rect) {
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

fn render_tag_popup(frame: &mut Frame, state: &AppState, list_state: &mut ListState, area: Rect) {
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
        Paragraph::new(Line::from(spans)).block(
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
        *list_state.offset_mut() = offset;
        list_state.select(Some(state.tag_selected_index));

        let sugg_list = List::new(sugg_items)
            .block(Block::bordered().title("Suggestions"))
            .highlight_style(Style::new().bg(Color::Blue).fg(Color::Black));
        frame.render_stateful_widget(sugg_list, sugg_area, list_state);

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

fn render_collections_view(
    frame: &mut Frame,
    state: &AppState,
    list_state: &mut ListState,
    items_list_state: &mut ListState,
    area: Rect,
) {
    if state.active_pane == ActivePane::CollectionItems && state.show_details {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(20),
                Constraint::Percentage(45),
                Constraint::Percentage(35),
            ])
            .split(area);

        render_collection_list(frame, state, list_state, chunks[0]);
        render_collection_commands(frame, state, items_list_state, chunks[1]);
        render_details(frame, state, chunks[2]);
    } else {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
            .split(area);

        render_collection_list(frame, state, list_state, chunks[0]);
        render_collection_commands(frame, state, items_list_state, chunks[1]);
    }
}

fn render_collection_list(
    frame: &mut Frame,
    state: &AppState,
    list_state: &mut ListState,
    area: Rect,
) {
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

    list_state.select(Some(state.selected_collection_index));
    frame.render_stateful_widget(list, area, list_state);
}

fn render_collection_commands(
    frame: &mut Frame,
    state: &AppState,
    list_state: &mut ListState,
    area: Rect,
) {
    let border_color = if state.active_pane == ActivePane::CollectionItems {
        Color::Yellow
    } else {
        Color::DarkGray
    };

    let (title, items): (&str, Vec<ListItem>) = if state.collections.is_empty() {
        ("Commands", vec![ListItem::new("Create a collection first")])
    } else if let Some(col) = state.selected_collection() {
        if state.filtered.is_empty() {
            (&col.name, vec![ListItem::new("No commands match search")])
        } else {
            (
                &col.name,
                state
                    .filtered
                    .iter()
                    .enumerate()
                    .map(|(idx, cmd)| {
                        let tags = cmd
                            .tags
                            .iter()
                            .map(|t| format!("#{}", t))
                            .collect::<Vec<_>>()
                            .join(" ");
                        let fav = if cmd.favorite { "⭐" } else { " " };
                        let text = if let Some(Some(indices)) = state.matched_indices.get(idx) {
                            let chars: Vec<char> = cmd.text.chars().collect();
                            let mut spans = Vec::new();
                            let mut in_match = false;
                            for (i, c) in chars.iter().enumerate() {
                                if indices.contains(&i) {
                                    if !in_match {
                                        spans.push(Span::styled(
                                            c.to_string(),
                                            Style::new().fg(Color::Yellow).bold(),
                                        ));
                                        in_match = true;
                                    } else {
                                        spans.push(Span::styled(
                                            c.to_string(),
                                            Style::new().fg(Color::Yellow).bold(),
                                        ));
                                    }
                                } else if in_match {
                                    spans.push(Span::raw(c.to_string()));
                                    in_match = false;
                                } else {
                                    spans.push(Span::raw(c.to_string()));
                                }
                            }
                            Line::from(spans)
                        } else {
                            Line::raw(&cmd.text)
                        };
                        let mut line = Line::from(vec![Span::raw(format!("{} ", fav))]);
                        line.spans.extend(text.spans);
                        line.spans.push(Span::raw(format!(" {}", tags)));
                        ListItem::new(line)
                    })
                    .collect(),
            )
        }
    } else {
        ("Commands", vec![ListItem::new("Select a collection")])
    };

    let list = List::new(items)
        .block(
            Block::bordered()
                .title(title)
                .border_type(BorderType::Rounded)
                .border_style(Style::new().fg(border_color)),
        )
        .highlight_style(Style::default().bg(Color::Blue).fg(Color::White))
        .highlight_symbol("> ");

    list_state.select(Some(state.selected_index));
    frame.render_stateful_widget(list, area, list_state);
}

fn render_collection_popup(
    frame: &mut Frame,
    state: &AppState,
    list_state: &mut ListState,
    area: Rect,
) {
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
        *list_state.offset_mut() = offset;
        list_state.select(Some(state.collection_popup_index));

        let list = List::new(items)
            .block(
                Block::bordered()
                    .title(title)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::new().fg(Color::Yellow)),
            )
            .highlight_style(Style::new().bg(Color::Blue).fg(Color::White));
        frame.render_stateful_widget(list, list_area, list_state);

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
            Paragraph::new(input_display).block(
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

fn center_rect(width: u16, height: u16, area: Rect) -> Rect {
    let vertical = Rect::new(
        area.x,
        area.y + (area.height.saturating_sub(height)) / 2,
        area.width.min(width),
        height.min(area.height),
    );
    Rect::new(
        area.x + (area.width.saturating_sub(width)) / 2,
        vertical.y,
        vertical.width,
        vertical.height,
    )
}

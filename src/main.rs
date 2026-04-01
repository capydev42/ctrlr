use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Clear, List, ListItem, ListState, Paragraph},
    DefaultTerminal, Frame,
};

mod history;
mod storage;

use std::io;
use std::time::{Duration, Instant};

#[derive(Clone, Debug, PartialEq)]
enum ActivePane {
    Search,
    History,
}

#[derive(Clone, Debug, PartialEq)]
enum InputMode {
    Normal,
    TagInput,
}

#[derive(Clone, Debug)]
struct Command {
    id: u32,
    text: String,
    tags: Vec<String>,
    favorite: bool,
    _context: String,
    use_count: i32,
    last_used: Option<i64>,
}

struct AppState {
    commands: Vec<Command>,
    filtered: Vec<Command>,
    selected_index: usize,
    search_query: String,
    status_message: Option<String>,
    status_timestamp: Option<Instant>,
    active_pane: ActivePane,
    input_mode: InputMode,
    tag_input: String,
    tag_selected_index: usize,
    tag_cursor_index: Option<usize>,
    db: Option<rusqlite::Connection>,
}

impl AppState {
    fn new(commands: Vec<Command>, db: Option<rusqlite::Connection>) -> Self {
        let filtered = commands.clone();
        Self {
            commands,
            filtered,
            selected_index: 0,
            search_query: String::new(),
            status_message: None,
            status_timestamp: None,
            active_pane: ActivePane::Search,
            input_mode: InputMode::Normal,
            tag_input: String::new(),
            tag_selected_index: 0,
            tag_cursor_index: None,
            db,
        }
    }

    fn selected_command_tags(&self) -> Vec<String> {
        self.filtered
            .get(self.selected_index)
            .map(|c| c.tags.clone())
            .unwrap_or_default()
    }

    fn current_tag_fragment(&self) -> String {
        self.tag_input
            .split(',')
            .next_back()
            .unwrap_or("")
            .trim()
            .to_string()
    }

    fn get_all_tags(&self) -> Vec<String> {
        let mut tags: Vec<String> = self.commands
            .iter()
            .flat_map(|c| c.tags.clone())
            .collect();
        tags.sort();
        tags.dedup();
        tags
    }

    fn filtered_tags(&self) -> Vec<String> {
        let fragment = self.current_tag_fragment().to_lowercase();
        let tags = self.get_all_tags();

        if fragment.is_empty() {
            return tags;
        }

        tags.into_iter()
            .filter(|t| t.to_lowercase().contains(&fragment))
            .collect()
    }

    fn apply_selected_tag(&mut self, tag: String) {
        let mut parts: Vec<String> = self.tag_input
            .split(',')
            .map(|s| s.trim().to_string())
            .collect();

        if parts.is_empty() {
            parts.push(tag);
        } else {
            parts.pop();
            parts.push(tag);
        }

        self.tag_input = parts.join(", ") + ", ";
    }

    fn set_tags(&mut self, tags: Vec<String>) {
        let current_id = self.filtered.get(self.selected_index).map(|c| c.id);
        if let Some(id) = current_id
            && let Some(cmd) = self.commands.iter_mut().find(|c| c.id == id)
        {
            cmd.tags = tags.clone();
            
            if let Some(ref mut conn) = self.db {
                use storage::tags;
                tags::set_tags_for_command(conn, &cmd.text, &tags).ok();
            }
            
            self.status_message = Some("🏷️ Tags updated".into());
            self.status_timestamp = Some(Instant::now());
        }
        self.filter_commands();
        if let Some(id) = current_id {
            self.selected_index = self.filtered.iter().position(|c| c.id == id).unwrap_or(0);
        }
    }

    fn switch_pane(&mut self) {
        self.active_pane = match self.active_pane {
            ActivePane::Search => ActivePane::History,
            ActivePane::History => ActivePane::Search,
        };
    }

    fn navigate_up(&mut self) {
        self.selected_index = if self.selected_index == 0 {
            self.filtered.len().saturating_sub(1)
        } else {
            self.selected_index - 1
        };
    }

    fn navigate_down(&mut self) {
        self.selected_index = (self.selected_index + 1) % self.filtered.len().max(1);
    }

    fn add_to_search(&mut self, c: char) {
        self.search_query.push(c);
        self.filter_commands();
    }

    fn remove_from_search(&mut self) {
        self.search_query.pop();
        self.filter_commands();
    }

    fn filter_commands(&mut self) {
        if self.search_query.is_empty() {
            self.filtered = self.commands.clone();
        } else {
            let query = self.search_query.to_lowercase();
            self.filtered = self
                .commands
                .iter()
                .filter(|cmd| {
                    cmd.text.to_lowercase().contains(&query)
                        || cmd.tags.iter().any(|t| t.to_lowercase().contains(&query))
                })
                .cloned()
                .collect();
        }
        self.filtered.sort_by(|a, b| b.favorite.cmp(&a.favorite));
        self.selected_index = 0;
    }

    fn handle_esc(&mut self) -> bool {
        if self.search_query.is_empty() {
            true
        } else {
            self.search_query.clear();
            self.filter_commands();
            false
        }
    }

    fn selected_command(&self) -> Option<String> {
        if self.filtered.is_empty() {
            return None;
        }
        self.filtered
            .get(self.selected_index)
            .map(|c| c.text.clone())
    }

    fn mark_executed(&mut self) {
        if let Some(selected) = self.filtered.get(self.selected_index)
            && let Some(cmd) = self.commands.iter_mut().find(|c| c.id == selected.id)
        {
            cmd.use_count += 1;
            cmd.last_used = Some(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs() as i64)
                    .unwrap_or(0),
            );

            if let Some(ref conn) = self.db {
                use storage::commands;
                commands::increment_use_count(conn, &cmd.text).ok();
            }
        }
    }

    fn toggle_favorite(&mut self) {
        if let Some(selected) = self.filtered.get(self.selected_index)
            && let Some(cmd) = self.commands.iter_mut().find(|c| c.id == selected.id)
        {
            cmd.favorite = !cmd.favorite;
            
            if let Some(ref conn) = self.db {
                use storage::commands;
                commands::update_favorite(conn, &cmd.text, cmd.favorite).ok();
            }
            
            self.status_message = Some(if cmd.favorite {
                format!("⭐ Favorited: {}", cmd.text)
            } else {
                format!("⭐ Unfavorited: {}", cmd.text)
            });
            self.status_timestamp = Some(Instant::now());
        }
        self.filter_commands();
    }
}

fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;

    if let Some(cmd) = ratatui::run(app)? {
        println!("{}", cmd);
    }
    Ok(())
}

fn app(terminal: &mut DefaultTerminal) -> io::Result<Option<String>> {
    let mut db = storage::init_db().ok();
    let mut commands = history::load_history();
    
    if let Some(ref mut conn) = db {
        let cmd_refs: Vec<(&str, String)> = commands
            .iter()
            .map(|c| {
                let id = storage::commands::hash_command(&c.text);
                (c.text.as_str(), id)
            })
            .collect();
        storage::commands::ensure_commands_exist(conn, &cmd_refs).ok();

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
        }
    }
    
    let mut state = AppState::new(commands, db);
    let mut list_state = ListState::default();
    list_state.select(Some(0));

    loop {
        if let (Some(_), Some(ts)) = (&state.status_message, state.status_timestamp)
            && ts.elapsed() > Duration::from_secs(2)
        {
            state.status_message = None;
            state.status_timestamp = None;
        }

        terminal.draw(|f| render(f, &state, &mut list_state))?;
        if let Event::Key(key) = crossterm::event::read()? {
            if key.code == KeyCode::Esc 
                && state.input_mode != InputMode::TagInput 
                && state.handle_esc() 
            {
                break Ok(None);
            }
            if let Some(cmd) = handle_key(&mut state, &mut list_state, key) {
                break Ok(Some(cmd));
            }
        }
    }
}

fn handle_key(state: &mut AppState, list_state: &mut ListState, key: KeyEvent) -> Option<String> {
    if state.input_mode == InputMode::TagInput {
        match (key.code, key.modifiers) {
            (KeyCode::Left, _) => {
                let tags = state.selected_command_tags();
                if !tags.is_empty() {
                    if let Some(idx) = state.tag_cursor_index {
                        state.tag_cursor_index = Some(if idx == 0 { tags.len() - 1 } else { idx - 1 });
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
            // think about also supporting ctrl+p/n additionally
            (KeyCode::Up, _) => {
                let len = state.filtered_tags().len();
                if len > 0 {
                    state.tag_selected_index = state.tag_selected_index.saturating_sub(1);
                }
            }
            (KeyCode::Down, _) => {
                let len = state.filtered_tags().len();
                if len > 0 {
                    state.tag_selected_index = (state.tag_selected_index + 1) % len;
                }
            }
            (KeyCode::Enter, _) => {
                let new_tags: Vec<String> = state.tag_input
                    .split(',')
                    .map(|t| t.trim().to_string())
                    .filter(|t| !t.is_empty())
                    .collect();

                let mut tags = state.selected_command_tags();
                tags.extend(new_tags);
                tags.sort();
                tags.dedup();
                state.set_tags(tags);
                state.tag_input.clear();
                state.tag_cursor_index = None;
                state.input_mode = InputMode::Normal;
                state.tag_selected_index = 0;
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
            state.active_pane = ActivePane::Search;
            return None;
        }
        (KeyCode::Char('2'), KeyModifiers::NONE) => {
            state.active_pane = ActivePane::History;
            return None;
        }
        (KeyCode::Enter, _) => {
            let cmd = state.selected_command();
            state.mark_executed();
            return cmd;
        }
        _ => {}
    }

    match (key.code, key.modifiers) {
        (KeyCode::Up, _) => {
            if state.active_pane == ActivePane::Search {
                state.active_pane = ActivePane::History;
            }
            state.navigate_up();
            list_state.select(Some(state.selected_index));
        }
        (KeyCode::Down, _) => {
            if state.active_pane == ActivePane::Search {
                state.active_pane = ActivePane::History;
            }
            state.navigate_down();
            list_state.select(Some(state.selected_index));
        }
        (KeyCode::Esc, _) => {
            state.handle_esc();
        }
        _ => {}
    }

    match state.active_pane {
        ActivePane::Search => {
            match (key.code, key.modifiers) {
                (KeyCode::Char(c), KeyModifiers::NONE) => {
                    state.add_to_search(c);
                }
                (KeyCode::Backspace, _) => {
                    state.remove_from_search();
                }
                _ => {}
            }
        }
        ActivePane::History => {
            match (key.code, key.modifiers) {
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
                _ => {}
            }
        }
    }
    None
}

fn render(frame: &mut Frame, state: &AppState, list_state: &mut ListState) {
    let area = frame.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(2),
        ])
        .split(area);

    render_search_bar(frame, state, chunks[0]);
    render_history_list(frame, state, list_state, chunks[1]);
    render_footer(frame, state, chunks[2]);

    if state.input_mode == InputMode::TagInput {
        render_tag_popup(frame, state, area);
    }
}

fn render_search_bar(frame: &mut Frame, state: &AppState, area: Rect) {
    let cursor = if state.active_pane == ActivePane::Search { "▋" } else { "" };
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

fn render_history_list(frame: &mut Frame, state: &AppState, list_state: &mut ListState, area: Rect) {
    let items: Vec<ListItem> = if state.filtered.is_empty() {
        vec![ListItem::new("No results found")]
    } else {
        state
            .filtered
            .iter()
            .map(|cmd| {
                let tags = cmd
                    .tags
                    .iter()
                    .map(|t| format!("#{}", t))
                    .collect::<Vec<_>>()
                    .join(" ");
                ListItem::new(format!(
                    "{:<2} {:<50} {}",
                    if cmd.favorite { "⭐" } else { " " },
                    cmd.text,
                    tags
                ))
            })
            .collect()
    };

    let history_border_color = if state.active_pane == ActivePane::History {
        Color::Yellow
    } else {
        Color::DarkGray
    };

    let list = List::new(items)
        .block(
            Block::bordered()
                .title(if state.active_pane == ActivePane::History {
                    "[History]"
                } else {
                    "History"
                })
                .border_type(BorderType::Rounded)
                .border_style(Style::new().fg(history_border_color)),
        )
        .highlight_style(Style::default().bg(Color::Blue).fg(Color::White)) // pff overthink color choice ...
        .highlight_symbol("> ");

    frame.render_stateful_widget(list, area, list_state);
}

fn render_footer(frame: &mut Frame, state: &AppState, area: Rect) {
    let footer_text = if let Some(msg) = &state.status_message {
        msg.clone()
    } else {
        match state.active_pane { 
            // help page would be nice .. -> check how others handle this in tuis 
            ActivePane::Search => {
                " 2: History | Type to search | Backspace: Delete | ↑/↓: Navigate | Enter: Select ".into()
            }
            ActivePane::History => {
                " /: Search | 1: Search | t: Tag | j/k or ↑/↓: Navigate | f: Favorite | Enter: Select | Esc: Exit ".into()
            }
        }
    };

    frame.render_widget(Paragraph::new(footer_text), area);
}

fn render_tag_popup(frame: &mut Frame, state: &AppState, area: Rect) {
    // lets see if i understand the popup in two weeks still, -> should refactor...
    let tags = state.selected_command_tags();
    let suggestions = if state.tag_cursor_index.is_none() {
        state.filtered_tags()
    } else {
        Vec::new()
    };
    let has_suggestions = !suggestions.is_empty() && !state.tag_input.is_empty();

    let input_height = if tags.is_empty() { 3 } else { 4 };
    let sugg_count = suggestions.len().min(5);
    let sugg_height = if has_suggestions { sugg_count as u16 + 2 } else { 0 };
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

    if has_suggestions {
        let sugg_items: Vec<ListItem> = suggestions
            .iter()
            .take(5)
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

        let sugg_height = (sugg_items.len() as u16 + 1).max(3);
        let sugg_area = Rect::new(
            chunks[1].x,
            chunks[1].y,
            chunks[1].width,
            sugg_height,
        );

        let sugg_list = List::new(sugg_items).block(Block::bordered().title("Suggestions"));
        frame.render_widget(sugg_list, sugg_area);
    }

    let hint = if !tags.is_empty() {
        "←/→: Select Tag | Backspace: Delete | Type: Add New | Enter: Save"
    } else {
        "Type to add tags | Enter: Save | Esc: Cancel"
    };
    frame.render_widget(
        Paragraph::new(hint)
            .style(Style::new().fg(Color::DarkGray))
            .alignment(Alignment::Center),
        chunks[2],
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

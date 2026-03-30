use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    widgets::{Block, BorderType, List, ListItem, ListState, Paragraph},
    DefaultTerminal, Frame,
};

use std::io;
use std::time::{Duration, Instant};

#[derive(Clone, Debug, PartialEq)]
enum ActivePane {
    Search,
    History,
}

#[derive(Clone, Debug)]
struct Command {
    id: u32,
    text: String,
    tags: Vec<String>,
    favorite: bool,
    _context: String,
}

struct AppState {
    commands: Vec<Command>,
    filtered: Vec<Command>,
    selected_index: usize,
    search_query: String,
    status_message: Option<String>,
    status_timestamp: Option<Instant>,
    active_pane: ActivePane,
}

impl AppState {
    fn new(commands: Vec<Command>) -> Self {
        let filtered = commands.clone();
        Self {
            commands,
            filtered,
            selected_index: 0,
            search_query: String::new(),
            status_message: None,
            status_timestamp: None,
            active_pane: ActivePane::Search,
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

    fn toggle_favorite(&mut self) {
        if let Some(selected) = self.filtered.get(self.selected_index)
            && let Some(cmd) = self.commands.iter_mut().find(|c| c.id == selected.id)
        {
            cmd.favorite = !cmd.favorite;
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
    let commands = vec![
        Command {
            id: 0,
            text: "ls -la".into(),
            tags: vec!["general".into()],
            favorite: true,
            _context: String::default(),
        },
        Command {
            id: 1,
            text: "docker build -t myapp .".into(),
            tags: vec!["docker".into()],
            favorite: true,
            _context: String::default(),
        },
        Command {
            id: 2,
            text: "git status".into(),
            tags: vec!["git".into()],
            favorite: false,
            _context: String::default(),
        },
    ];

    let mut state = AppState::new(commands);
    let mut list_state = ListState::default();
    list_state.select(Some(0));

    loop {
        // Clear status after 2 seconds
        if let (Some(_), Some(ts)) = (&state.status_message, state.status_timestamp)
            && ts.elapsed() > Duration::from_secs(2)
        {
            state.status_message = None;
            state.status_timestamp = None;
        }



        terminal.draw(|f| render(f, &state, &mut list_state))?;
        if let Event::Key(key) = crossterm::event::read()? {
            // quit
            if key.code == KeyCode::Esc && state.handle_esc() {
                break Ok(None);
            }
            if let Some(cmd) = handle_key(&mut state, &mut list_state, key) {
                break Ok(Some(cmd));
            }
        }
    }
}

fn handle_key(state: &mut AppState, list_state: &mut ListState, key: KeyEvent) -> Option<String> {
    // Keys that return early
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
        (KeyCode::Enter, _) => return state.selected_command(),
        _ => {}
    }

    // Keys that work in both panes (navigation, escape)
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

    // Pane-specific keys
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

    // search bar
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
        chunks[0],
    );

    //History List
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
        .highlight_style(Style::default()
            .bg(Color::Blue)
            .fg(Color::White)
        )
        .highlight_symbol("> ");

    frame.render_stateful_widget(list, chunks[1], list_state);

    //Footer
    let footer_text = if let Some(msg) = &state.status_message {
        msg.clone()
    } else {
        match state.active_pane {
            ActivePane::Search => {
                " 2: History | Type to search | Backspace: Delete | ↑/↓: Navigate | Enter: Select ".into()
            }
            ActivePane::History => {
                " /: Search | 1: Search | j/k or ↑/↓: Navigate | f: Favorite | Enter: Select | Esc: Exit ".into()
            }
        }
    };
    frame.render_widget(Paragraph::new(footer_text), chunks[2]);
}

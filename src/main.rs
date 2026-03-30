use crossterm::event::{Event, KeyCode, KeyEvent};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    widgets::{Block, BorderType, List, ListItem, ListState, Paragraph},
    DefaultTerminal, Frame,
};

use std::io;

#[derive(Clone, Debug)]
struct Command {
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
}

impl AppState {
    fn new(commands: Vec<Command>) -> Self {
        let filtered = commands.clone();
        Self {
            commands,
            filtered,
            selected_index: 0,
            search_query: String::new(),
        }
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
        self.filtered
            .get(self.selected_index)
            .map(|c| c.text.clone())
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
            text: "ls -la".into(),
            tags: vec!["general".into()],
            favorite: true,
            _context: String::default(),
        },
        Command {
            text: "docker build -t myapp .".into(),
            tags: vec!["docker".into()],
            favorite: true,
            _context: String::default(),
        },
        Command {
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
    match key.code {
        KeyCode::Up => {
            state.navigate_up();
            list_state.select(Some(state.selected_index));
            None
        }
        KeyCode::Down => {
            state.navigate_down();
            list_state.select(Some(state.selected_index));
            None
        }
        KeyCode::Char(c) => {
            state.add_to_search(c);
            None
        }
        KeyCode::Backspace => {
            state.remove_from_search();
            None
        }
        KeyCode::Enter => state.selected_command(),
        KeyCode::Esc => {
            state.handle_esc();
            None
        }
        _ => None,
    }
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
    let search_text = format!("Search: {}", state.search_query);
    frame.render_widget(
        Paragraph::new(search_text).block(
            Block::bordered()
                .title("cltr")
                .border_type(BorderType::Rounded),
        ),
        chunks[0],
    );

    //History List
    let items: Vec<ListItem> = state
        .filtered
        .iter()
        .map(|cmd| {
            let tags = cmd
                .tags
                .iter()
                .map(|t| format!("#{}", t))
                .collect::<Vec<_>>()
                .join(" ");
            let fav = if cmd.favorite { " *" } else { "" };
            ListItem::new(format!("{}  {}{}", cmd.text, tags, fav))
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::bordered()
                .title("History")
                .border_type(BorderType::Rounded),
        )
        .highlight_style(Style::new().bg(Color::Cyan))
        .highlight_symbol("> ");

    frame.render_stateful_widget(list, chunks[1], list_state);

    //Footer
    frame.render_widget(
        Paragraph::new(
            " ↑ / ↓ : Navigate  | Enter : Select | f : Favorite | t : Tag | Esc : Exit ",
        ),
        chunks[2],
    );
}

use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use fuzzy_matcher::FuzzyMatcher;
use fuzzy_matcher::skim::SkimMatcherV2;
use ratatui::{
    DefaultTerminal, Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Clear, List, ListItem, ListState, Paragraph, Wrap},
};

mod cli;
mod history;
mod storage;

use std::collections::HashSet;
use std::io;
use std::time::{Duration, Instant};

use storage::collections::Collection;

#[derive(Clone, Debug, PartialEq)]
enum ActivePane {
    Search,
    History,
    CollectionsList,
    CollectionItems,
}

#[derive(Clone, Debug, PartialEq)]
enum ViewMode {
    History,
    Favorites,
    Collections,
}

#[derive(Clone, Debug, PartialEq)]
enum InputMode {
    Normal,
    TagInput,
    CollectionInput,
}

#[derive(Clone, Debug, PartialEq)]
enum CollectionInputMode {
    None,
    AddToCollection,
    NewCollection,
    EditCollection,
}

#[derive(Clone, Debug)]
struct Command {
    id: String,
    text: String,
    tags: Vec<String>,
    collection_ids: Vec<String>,
    favorite: bool,
    _context: Vec<String>,
    use_count: i32,
    last_used: Option<i64>,
}

struct AppState {
    commands: Vec<Command>,
    filtered: Vec<Command>,
    matched_indices: Vec<Option<HashSet<usize>>>,
    selected_index: usize,
    search_query: String,
    status_message: Option<String>,
    status_timestamp: Option<Instant>,
    active_pane: ActivePane,
    view_mode: ViewMode,
    show_details: bool,
    input_mode: InputMode,
    tag_input: String,
    tag_selected_index: usize,
    tag_cursor_index: Option<usize>,
    db: Option<rusqlite::Connection>,
    matcher: SkimMatcherV2,
    collections: Vec<Collection>,
    selected_collection_index: usize,
    collection_selected_index: usize,
    collection_commands: Vec<Command>,
    collection_input_mode: CollectionInputMode,
    collection_input_text: String,
    editing_collection_id: Option<String>,
}

impl AppState {
    fn new(commands: Vec<Command>, db: Option<rusqlite::Connection>) -> Self {
        let filtered = commands.clone();
        let matched_indices = vec![None; filtered.len()];
        Self {
            commands,
            filtered,
            matched_indices,
            selected_index: 0,
            search_query: String::new(),
            status_message: None,
            status_timestamp: None,
            active_pane: ActivePane::Search,
            view_mode: ViewMode::History,
            show_details: true,
            input_mode: InputMode::Normal,
            tag_input: String::new(),
            tag_selected_index: 0,
            tag_cursor_index: None,
            db,
            matcher: SkimMatcherV2::default(),
            collections: Vec::new(),
            selected_collection_index: 0,
            collection_selected_index: 0,
            collection_commands: Vec::new(),
            collection_input_mode: CollectionInputMode::None,
            collection_input_text: String::new(),
            editing_collection_id: None,
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
        let mut tags: Vec<String> = self.commands.iter().flat_map(|c| c.tags.clone()).collect();
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
        let mut parts: Vec<String> = self
            .tag_input
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
        let current_id = self.filtered.get(self.selected_index).map(|c| c.id.clone());

        if let Some(ref id) = current_id {
            let cmd = self.commands.iter_mut().find(|c| &c.id == id);
            if let Some(cmd) = cmd {
                cmd.tags = tags.clone();

                if let Some(ref mut conn) = self.db {
                    use storage::tags;
                    if let Err(e) = tags::set_tags_for_command(conn, &cmd.text, &tags) {
                        eprintln!("DB error saving tags: {}", e);
                    }
                }

                self.status_message = Some("🏷️ Tags updated".into());
                self.status_timestamp = Some(Instant::now());
            }
        }
        self.filter_commands();
        if let Some(ref id) = current_id {
            self.selected_index = self.filtered.iter().position(|c| c.id == *id).unwrap_or(0);
        }
    }

    fn switch_pane(&mut self) {
        self.active_pane = match (&self.view_mode, &self.active_pane) {
            (ViewMode::Collections, ActivePane::CollectionsList) => ActivePane::CollectionItems,
            (ViewMode::Collections, ActivePane::CollectionItems) => ActivePane::Search,
            (ViewMode::Collections, ActivePane::Search) => ActivePane::CollectionsList,
            (_, ActivePane::Search) => ActivePane::History,
            (_, ActivePane::History) => ActivePane::Search,
            _ => ActivePane::Search,
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
        let base_commands: Vec<&Command> = match self.view_mode {
            ViewMode::History => self.commands.iter().collect(),
            ViewMode::Favorites => self.commands.iter().filter(|c| c.favorite).collect(),
            ViewMode::Collections => {
                if let Some(col) = self.selected_collection() {
                    self.commands
                        .iter()
                        .filter(|c| c.collection_ids.contains(&col.id))
                        .collect()
                } else {
                    vec![]
                }
            }
        };

        if self.search_query.is_empty() {
            self.filtered = base_commands.into_iter().cloned().collect();
            self.matched_indices = vec![None; self.filtered.len()];
        } else {
            let query = &self.search_query;
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0);

            let mut scored: Vec<(i64, Vec<usize>, Command, bool)> = base_commands
                .into_iter()
                .filter_map(|cmd| {
                    let best_text = self.matcher.fuzzy_indices(&cmd.text, query);
                    let mut best_tag: Option<(i64, Vec<usize>)> = None;

                    for tag in &cmd.tags {
                        if let Some((score, _)) = self.matcher.fuzzy_indices(tag, query) {
                            best_tag = Some(match best_tag {
                                Some((s, _)) => {
                                    if score > s {
                                        (score, self.matcher.fuzzy_indices(tag, query).unwrap().1)
                                    } else {
                                        best_tag.unwrap()
                                    }
                                }
                                None => (score, self.matcher.fuzzy_indices(tag, query).unwrap().1),
                            });
                        }
                    }

                    match (best_text, best_tag) {
                        (Some((text_score, text_indices)), Some((tag_score, _))) => {
                            if text_score >= tag_score {
                                Some((text_score, text_indices, cmd.clone(), true))
                            } else {
                                Some((tag_score, vec![], cmd.clone(), false))
                            }
                        }
                        (Some((score, indices)), None) => Some((score, indices, cmd.clone(), true)),
                        (None, Some((score, _))) => Some((score, vec![], cmd.clone(), false)),
                        (None, None) => None,
                    }
                })
                .collect();

            scored.sort_by(|a, b| {
                let score_a = compute_ranking_score(&a.2, a.0, now);
                let score_b = compute_ranking_score(&b.2, b.0, now);
                score_b
                    .cmp(&score_a)
                    .then_with(|| b.2.use_count.cmp(&a.2.use_count))
            });

            self.filtered = scored.iter().map(|(_, _, cmd, _)| cmd.clone()).collect();
            self.matched_indices = scored
                .into_iter()
                .map(|(_, indices, _, is_text)| {
                    if is_text {
                        Some(indices.into_iter().collect())
                    } else {
                        None
                    }
                })
                .collect();
        }
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

    fn active_command(&self) -> Option<&Command> {
        match self.view_mode {
            ViewMode::Collections => {
                if self.active_pane == ActivePane::CollectionItems {
                    self.collection_commands.get(self.collection_selected_index)
                } else {
                    self.filtered.get(self.selected_index)
                }
            }
            _ => self.filtered.get(self.selected_index),
        }
    }

    fn mark_executed(&mut self) {
        let selected_id = self.filtered.get(self.selected_index).map(|c| c.id.clone());
        let cmd = selected_id.and_then(|id| self.commands.iter_mut().find(|c| c.id == id));
        if let Some(cmd) = cmd {
            cmd.use_count += 1;
            cmd.last_used = Some(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs() as i64)
                    .unwrap_or(0),
            );

            if let Some(ref mut conn) = self.db {
                use storage::commands;
                if let Err(e) = commands::increment_use_count(conn, &cmd.text) {
                    eprintln!("DB error updating use count: {}", e);
                }
            }
        }
    }

    fn mark_executed_for_text(&mut self, text: &str) {
        if let Some(cmd) = self.commands.iter_mut().find(|c| c.text == text) {
            cmd.use_count += 1;
            cmd.last_used = Some(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs() as i64)
                    .unwrap_or(0),
            );

            if let Some(ref mut conn) = self.db {
                use storage::commands;
                if let Err(e) = commands::increment_use_count(conn, &cmd.text) {
                    eprintln!("DB error updating use count: {}", e);
                }
            }
        }
    }

    fn toggle_favorite(&mut self) {
        let selected_id = self.filtered.get(self.selected_index).map(|c| c.id.clone());
        let cmd = selected_id.and_then(|id| self.commands.iter_mut().find(|c| c.id == id));
        if let Some(cmd) = cmd {
            cmd.favorite = !cmd.favorite;

            if let Some(ref mut conn) = self.db {
                use storage::commands;
                if let Err(e) = commands::update_favorite(conn, &cmd.text, cmd.favorite) {
                    eprintln!("DB error updating favorite: {}", e);
                }
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

    fn selected_collection(&self) -> Option<&Collection> {
        self.collections.get(self.selected_collection_index)
    }

    fn load_collections(&mut self) {
        if let Some(ref conn) = self.db {
            match storage::collections::get_all_collections(conn) {
                Ok(cols) => self.collections = cols,
                Err(e) => eprintln!("DB error loading collections: {}", e),
            }
        }
    }

    fn load_collection_commands(&mut self) {
        self.collection_commands.clear();
        let conn = match self.db.as_ref() {
            Some(c) => c,
            None => return,
        };
        let col = match self.selected_collection() {
            Some(c) => c,
            None => return,
        };
        match storage::collections::get_command_ids_in_collection(conn, &col.id) {
            Ok(ids) => {
                for id in ids {
                    if let Some(cmd) = self.commands.iter().find(|c| c.id == id) {
                        self.collection_commands.push(cmd.clone());
                    }
                }
            }
            Err(e) => eprintln!("DB error loading collection commands: {}", e),
        }
    }

    fn create_collection(&mut self, name: String) {
        let name_for_msg = name.clone();
        if let Some(ref conn) = self.db {
            match storage::collections::create_collection(conn, &name) {
                Ok(id) => {
                    self.collections.push(Collection { id, name });
                    self.collections.sort_by(|a, b| a.name.cmp(&b.name));
                    self.status_message = Some(format!("Created collection: {}", name_for_msg));
                    self.status_timestamp = Some(Instant::now());
                }
                Err(e) => eprintln!("DB error creating collection: {}", e),
            }
        }
    }

    fn rename_collection(&mut self, id: &str, new_name: String) {
        if let Some(ref conn) = self.db {
            match storage::collections::rename_collection(conn, id, &new_name) {
                Ok(()) => {
                    if let Some(col) = self.collections.iter_mut().find(|c| c.id == id) {
                        col.name = new_name.clone();
                    }
                    self.collections.sort_by(|a, b| a.name.cmp(&b.name));
                    self.status_message = Some(format!("Renamed to: {}", new_name));
                    self.status_timestamp = Some(Instant::now());
                }
                Err(e) => eprintln!("DB error renaming collection: {}", e),
            }
        }
    }

    fn delete_collection(&mut self) {
        let col_id = self.selected_collection().map(|c| c.id.clone());
        let col_name = self.selected_collection().map(|c| c.name.clone());
        let (id, name) = match (col_id, col_name) {
            (Some(id), Some(name)) => (id, name),
            _ => return,
        };
        let conn = match self.db.as_mut() {
            Some(c) => c,
            None => return,
        };
        match storage::collections::delete_collection(conn, &id) {
            Ok(()) => {
                self.collections.retain(|c| c.id != id);
                if self.selected_collection_index >= self.collections.len() {
                    self.selected_collection_index = self.collections.len().saturating_sub(1);
                }
                self.load_collection_commands();
                self.status_message = Some(format!("Deleted collection: {}", name));
                self.status_timestamp = Some(Instant::now());
            }
            Err(e) => eprintln!("DB error deleting collection: {}", e),
        }
    }

    fn add_command_to_collection(&mut self, cmd_text: &str, collection_id: &str) {
        let col_name = self
            .collections
            .iter()
            .find(|c| c.id == collection_id)
            .map(|c| c.name.clone());
        if let Some(ref conn) = self.db {
            match storage::collections::add_command_to_collection(conn, cmd_text, collection_id) {
                Ok(()) => {
                    if let Some(name) = col_name {
                        self.status_message = Some(format!("Added to {}", name));
                        self.status_timestamp = Some(Instant::now());
                    }
                    self.load_collection_commands();
                }
                Err(e) => eprintln!("DB error adding to collection: {}", e),
            }
        }
    }

    fn remove_command_from_collection(&mut self, cmd_text: &str) {
        let col_name = self.selected_collection().map(|c| c.name.clone());
        let col_id = self.selected_collection().map(|c| c.id.clone());
        let (name, id) = match (col_name, col_id) {
            (Some(name), Some(id)) => (name, id),
            _ => return,
        };
        let conn = match self.db.as_ref() {
            Some(c) => c,
            None => return,
        };
        match storage::collections::remove_command_from_collection(conn, cmd_text, &id) {
            Ok(()) => {
                self.load_collection_commands();
                self.status_message = Some(format!("Removed from {}", name));
                self.status_timestamp = Some(Instant::now());
            }
            Err(e) => eprintln!("DB error removing from collection: {}", e),
        }
    }

    fn navigate_collection_up(&mut self) {
        self.selected_collection_index = self.selected_collection_index.saturating_sub(1);
        self.load_collection_commands();
    }

    fn navigate_collection_down(&mut self) {
        if !self.collections.is_empty() {
            self.selected_collection_index =
                (self.selected_collection_index + 1) % self.collections.len();
            self.load_collection_commands();
        }
    }
}

fn compute_ranking_score(cmd: &Command, fuzzy: i64, now: i64) -> i64 {
    let usage = cmd.use_count as i64 * 2;

    let recency = if let Some(ts) = cmd.last_used {
        let age = now - ts;
        if age < 3600 {
            50
        } else if age < 86400 {
            20
        } else if age < 604800 {
            10
        } else {
            0
        }
    } else {
        0
    };

    let favorite = if cmd.favorite { 100 } else { 0 };

    fuzzy * 10 + usage + recency + favorite
}

fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;
    cli::run()
}

pub fn run_tui(output_file: Option<String>) -> color_eyre::Result<Option<String>> {
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
    list_state.select(Some(0));
    collection_list_state.select(Some(0));
    collection_items_list_state.select(Some(0));

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
                let new_tags: Vec<String> = state
                    .tag_input
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
            return None;
        }
        (KeyCode::Char('c'), KeyModifiers::NONE) => {
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
            return None;
        }
        (KeyCode::Enter, _) => {
            if state.view_mode == ViewMode::Collections {
                match state.active_pane {
                    ActivePane::CollectionsList => {
                        state.load_collection_commands();
                        state.active_pane = ActivePane::CollectionItems;
                        return None;
                    }
                    ActivePane::CollectionItems => {
                        let cmd = state
                            .collection_commands
                            .get(state.collection_selected_index)
                            .cloned();
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
                    if state.collection_selected_index > 0 {
                        state.collection_selected_index -= 1;
                    }
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
                    state.collection_selected_index = (state.collection_selected_index + 1)
                        .min(state.collection_commands.len().saturating_sub(1));
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
            (KeyCode::Char('r'), KeyModifiers::NONE) => {
                if let Some(cmd) = state
                    .collection_commands
                    .get(state.collection_selected_index)
                {
                    let text = cmd.text.clone();
                    state.remove_command_from_collection(&text);
                }
            }
            (KeyCode::Char('j'), KeyModifiers::NONE) => {
                state.collection_selected_index = (state.collection_selected_index + 1)
                    .min(state.collection_commands.len().saturating_sub(1));
            }
            (KeyCode::Char('k'), KeyModifiers::NONE) => {
                if state.collection_selected_index > 0 {
                    state.collection_selected_index -= 1;
                }
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
                match c {
                    'j' => {
                        state.selected_collection_index = (state.selected_collection_index + 1)
                            .min(state.collections.len().saturating_sub(1));
                    }
                    'k' => {
                        if state.selected_collection_index > 0 {
                            state.selected_collection_index -= 1;
                        }
                    }
                    _ => {}
                }
            } else {
                state.collection_input_text.push(c);
            }
        }
        (KeyCode::Backspace, _) => {
            if state.collection_input_mode != CollectionInputMode::AddToCollection {
                state.collection_input_text.pop();
            }
        }
        (KeyCode::Up, _) => {
            if state.collection_input_mode == CollectionInputMode::AddToCollection
                && !state.collections.is_empty()
            {
                state.selected_collection_index = state.selected_collection_index.saturating_sub(1);
            }
        }
        (KeyCode::Down, _) => {
            if state.collection_input_mode == CollectionInputMode::AddToCollection
                && !state.collections.is_empty()
            {
                state.selected_collection_index = (state.selected_collection_index + 1)
                    .min(state.collections.len().saturating_sub(1));
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
                    let col = state
                        .collections
                        .get(state.selected_collection_index)
                        .cloned();
                    let cmd = state.filtered.get(state.selected_index).cloned();
                    if let (Some(col), Some(cmd)) = (col, cmd) {
                        state.add_command_to_collection(&cmd.text, &col.id);
                        state.load_collection_commands();
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
        render_tag_popup(frame, state, area);
    }

    if state.input_mode == InputMode::CollectionInput {
        render_collection_popup(frame, state, area);
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

    let cmd = match state.filtered.get(state.selected_index) {
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
                        " j/k or ↑/↓: Navigate | Enter: Select | c: Add to Collection | r: Remove | Tab: Switch pane ".into()
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
    let sugg_height = if has_suggestions {
        sugg_count as u16 + 2
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
        let sugg_area = Rect::new(chunks[1].x, chunks[1].y, chunks[1].width, sugg_height);

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

fn render_collections_view(
    frame: &mut Frame,
    state: &AppState,
    list_state: &mut ListState,
    items_list_state: &mut ListState,
    area: Rect,
) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
        .split(area);

    render_collection_list(frame, state, list_state, chunks[0]);
    render_collection_commands(frame, state, items_list_state, chunks[1]);
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
        if state.collection_commands.is_empty() {
            (
                &col.name,
                vec![ListItem::new("No commands in this collection")],
            )
        } else {
            (
                &col.name,
                state
                    .collection_commands
                    .iter()
                    .map(|cmd| {
                        let tags = cmd
                            .tags
                            .iter()
                            .map(|t| format!("#{}", t))
                            .collect::<Vec<_>>()
                            .join(" ");
                        let fav = if cmd.favorite { "⭐" } else { " " };
                        ListItem::new(format!("{} {} {}", fav, cmd.text, tags))
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

    list_state.select(Some(state.collection_selected_index));
    frame.render_stateful_widget(list, area, list_state);
}

fn render_collection_popup(frame: &mut Frame, state: &AppState, area: Rect) {
    let popup_height = 5u16;
    let popup_width = 50u16;
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
            "j/k: Navigate | Enter: Add | Esc: Cancel",
        ),
        CollectionInputMode::None => return,
    };

    let content: Vec<ListItem> =
        if state.collection_input_mode == CollectionInputMode::AddToCollection {
            if state.collections.is_empty() {
                vec![ListItem::new("No collections - press n to create one")]
            } else {
                let active_cmd = state.active_command();
                let cmd_col_ids: Vec<&str> = active_cmd
                    .map(|c| c.collection_ids.iter().map(|s| s.as_str()).collect())
                    .unwrap_or_default();
                state
                    .collections
                    .iter()
                    .enumerate()
                    .map(|(idx, col)| {
                        let prefix = if cmd_col_ids.contains(&col.id.as_str()) {
                            "✔ "
                        } else {
                            "  "
                        };
                        if idx == state.selected_collection_index {
                            ListItem::new(format!("> {}{}", prefix, col.name))
                                .style(Style::new().bg(Color::Blue).fg(Color::Black))
                        } else {
                            ListItem::new(format!("  {}{}", prefix, col.name))
                        }
                    })
                    .collect()
            }
        } else {
            let input_text = if state.collection_input_text.is_empty() {
                "▋".to_string()
            } else {
                format!("{}▋", state.collection_input_text)
            };
            vec![ListItem::new(input_text)]
        };

    let list = List::new(content).block(
        Block::bordered()
            .title(title)
            .border_type(BorderType::Rounded)
            .border_style(Style::new().fg(Color::Yellow)),
    );

    frame.render_widget(list, centered);

    let hint_area = Rect::new(centered.x, centered.y + popup_height - 1, popup_width, 1);
    frame.render_widget(
        Paragraph::new(hint)
            .style(Style::new().fg(Color::DarkGray))
            .alignment(Alignment::Center),
        hint_area,
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

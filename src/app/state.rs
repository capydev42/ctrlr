use std::collections::HashSet;
use std::time::Instant;

use fuzzy_matcher::FuzzyMatcher;
use fuzzy_matcher::skim::SkimMatcherV2;
use ratatui::widgets::ListState;

use crate::storage::collections::Collection;

#[derive(Clone, Debug, PartialEq)]
pub enum ActivePane {
    Search,
    History,
    CollectionsList,
    CollectionItems,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ViewMode {
    History,
    Favorites,
    Collections,
}

#[derive(Clone, Debug, PartialEq)]
pub enum InputMode {
    Normal,
    TagInput,
    CollectionInput,
}

#[derive(Clone, Debug, PartialEq)]
pub enum CollectionInputMode {
    None,
    AddToCollection,
    NewCollection,
    EditCollection,
}

#[derive(Clone, Debug)]
pub struct Command {
    pub id: String,
    pub text: String,
    pub tags: Vec<String>,
    pub collection_ids: Vec<String>,
    pub favorite: bool,
    pub _context: Vec<String>,
    pub use_count: i32,
    pub last_used: Option<i64>,
}

pub struct AppState {
    pub commands: Vec<Command>,
    pub filtered: Vec<Command>,
    pub matched_indices: Vec<Option<HashSet<usize>>>,
    pub selected_index: usize,
    pub search_query: String,
    pub status_message: Option<String>,
    pub status_timestamp: Option<Instant>,
    pub active_pane: ActivePane,
    pub view_mode: ViewMode,
    pub show_details: bool,
    pub input_mode: InputMode,
    pub tag_input: String,
    pub tag_selected_index: usize,
    pub tag_cursor_index: Option<usize>,
    pub db: Option<rusqlite::Connection>,
    matcher: SkimMatcherV2,
    pub collections: Vec<Collection>,
    pub selected_collection_index: usize,
    pub collection_popup_index: usize,
    pub collection_commands: Vec<Command>,
    pub collection_input_mode: CollectionInputMode,
    pub collection_input_text: String,
    pub editing_collection_id: Option<String>,
    pub list_state: ListState,
    pub collection_list_state: ListState,
    pub collection_items_list_state: ListState,
    pub tag_popup_list_state: ListState,
    pub collection_popup_list_state: ListState,
}

impl AppState {
    pub fn new(commands: Vec<Command>, db: Option<rusqlite::Connection>) -> Self {
        let filtered = commands.clone();
        let matched_indices = vec![None; filtered.len()];
        let list_state = {
            let mut s = ListState::default();
            s.select(Some(0));
            s
        };
        let collection_list_state = {
            let mut s = ListState::default();
            s.select(Some(0));
            s
        };
        let collection_items_list_state = {
            let mut s = ListState::default();
            s.select(Some(0));
            s
        };
        let tag_popup_list_state = {
            let mut s = ListState::default();
            s.select(Some(0));
            s
        };
        let collection_popup_list_state = {
            let mut s = ListState::default();
            s.select(Some(0));
            s
        };
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
            collection_popup_index: 0,
            collection_commands: Vec::new(),
            collection_input_mode: CollectionInputMode::None,
            collection_input_text: String::new(),
            editing_collection_id: None,
            list_state,
            collection_list_state,
            collection_items_list_state,
            tag_popup_list_state,
            collection_popup_list_state,
        }
    }

    pub fn selected_command_tags(&self) -> Vec<String> {
        self.filtered
            .get(self.selected_index)
            .map(|c| c.tags.clone())
            .unwrap_or_default()
    }

    pub fn current_tag_fragment(&self) -> String {
        self.tag_input
            .split(',')
            .next_back()
            .unwrap_or("")
            .trim()
            .to_string()
    }

    pub fn get_all_tags(&self) -> Vec<String> {
        let mut tags: Vec<String> = self.commands.iter().flat_map(|c| c.tags.clone()).collect();
        tags.sort();
        tags.dedup();
        tags
    }

    pub fn filtered_tags(&self) -> Vec<String> {
        let fragment = self.current_tag_fragment().to_lowercase();
        let tags = self.get_all_tags();

        if fragment.is_empty() {
            return tags;
        }

        tags.into_iter()
            .filter(|t| t.to_lowercase().contains(&fragment))
            .collect()
    }

    pub fn apply_selected_tag(&mut self, tag: String) {
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

    pub fn set_tags(&mut self, tags: Vec<String>) {
        let current_id = self.filtered.get(self.selected_index).map(|c| c.id.clone());

        if let Some(ref id) = current_id {
            let cmd = self.commands.iter_mut().find(|c| &c.id == id);
            if let Some(cmd) = cmd {
                cmd.tags = tags.clone();

                if let Some(ref mut conn) = self.db {
                    use crate::storage::tags;
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

    pub fn switch_pane(&mut self) {
        self.active_pane = match (&self.view_mode, &self.active_pane) {
            (ViewMode::Collections, ActivePane::CollectionsList) => ActivePane::CollectionItems,
            (ViewMode::Collections, ActivePane::CollectionItems) => ActivePane::Search,
            (ViewMode::Collections, ActivePane::Search) => ActivePane::CollectionsList,
            (_, ActivePane::Search) => ActivePane::History,
            (_, ActivePane::History) => ActivePane::Search,
            _ => ActivePane::Search,
        };
    }

    pub fn navigate_up(&mut self) {
        self.selected_index = if self.selected_index == 0 {
            self.filtered.len().saturating_sub(1)
        } else {
            self.selected_index - 1
        };
    }

    pub fn navigate_down(&mut self) {
        self.selected_index = (self.selected_index + 1) % self.filtered.len().max(1);
    }

    pub fn add_to_search(&mut self, c: char) {
        self.search_query.push(c);
        self.filter_commands();
    }

    pub fn remove_from_search(&mut self) {
        self.search_query.pop();
        self.filter_commands();
    }

    pub fn filter_commands(&mut self) {
        let base_commands: Vec<&Command> = match self.view_mode {
            ViewMode::History => self.commands.iter().collect(),
            ViewMode::Favorites => self.commands.iter().filter(|c| c.favorite).collect(),
            ViewMode::Collections => {
                self.collection_commands = if let Some(col) = self.selected_collection() {
                    self.commands
                        .iter()
                        .filter(|c| c.collection_ids.contains(&col.id))
                        .cloned()
                        .collect()
                } else {
                    vec![]
                };
                self.collection_commands.iter().collect()
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

        if !self.search_query.is_empty() {
            self.selected_index = 0;
        }
    }

    pub fn handle_esc(&mut self) -> bool {
        if self.search_query.is_empty() {
            true
        } else {
            self.search_query.clear();
            self.filter_commands();
            false
        }
    }

    pub fn selected_command(&self) -> Option<String> {
        if self.filtered.is_empty() {
            return None;
        }
        self.filtered
            .get(self.selected_index)
            .map(|c| c.text.clone())
    }

    pub fn active_command(&self) -> Option<&Command> {
        self.filtered.get(self.selected_index)
    }

    pub fn mark_executed(&mut self) {
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
                use crate::storage::commands;
                if let Err(e) = commands::increment_use_count(conn, &cmd.text) {
                    eprintln!("DB error updating use count: {}", e);
                }
            }
        }
    }

    pub fn mark_executed_for_text(&mut self, text: &str) {
        if let Some(cmd) = self.commands.iter_mut().find(|c| c.text == text) {
            cmd.use_count += 1;
            cmd.last_used = Some(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs() as i64)
                    .unwrap_or(0),
            );

            if let Some(ref mut conn) = self.db {
                use crate::storage::commands;
                if let Err(e) = commands::increment_use_count(conn, &cmd.text) {
                    eprintln!("DB error updating use count: {}", e);
                }
            }
        }
    }

    pub fn toggle_favorite(&mut self) {
        let selected_id = self.filtered.get(self.selected_index).map(|c| c.id.clone());
        let cmd = selected_id.and_then(|id| self.commands.iter_mut().find(|c| c.id == id));
        if let Some(cmd) = cmd {
            cmd.favorite = !cmd.favorite;

            if let Some(ref mut conn) = self.db {
                use crate::storage::commands;
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

    pub fn selected_collection(&self) -> Option<&Collection> {
        self.collections.get(self.selected_collection_index)
    }

    pub fn load_collections(&mut self) {
        if let Some(ref conn) = self.db {
            match crate::storage::collections::get_all_collections(conn) {
                Ok(cols) => self.collections = cols,
                Err(e) => eprintln!("DB error loading collections: {}", e),
            }
        }
    }

    pub fn load_collection_commands(&mut self) {
        self.collection_commands.clear();
        let conn = match self.db.as_ref() {
            Some(c) => c,
            None => return,
        };
        let col = match self.selected_collection() {
            Some(c) => c,
            None => return,
        };
        match crate::storage::collections::get_command_ids_in_collection(conn, &col.id) {
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

    pub fn create_collection(&mut self, name: String) {
        let name_for_msg = name.clone();
        if let Some(ref conn) = self.db {
            match crate::storage::collections::create_collection(conn, &name) {
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

    pub fn rename_collection(&mut self, id: &str, new_name: String) {
        if let Some(ref conn) = self.db {
            match crate::storage::collections::rename_collection(conn, id, &new_name) {
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

    pub fn delete_collection(&mut self) {
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
        match crate::storage::collections::delete_collection(conn, &id) {
            Ok(()) => {
                for cmd in self.commands.iter_mut() {
                    cmd.collection_ids.retain(|c| c != &id);
                }
                self.collections.retain(|c| c.id != id);
                if self.selected_collection_index >= self.collections.len() {
                    self.selected_collection_index = self.collections.len().saturating_sub(1);
                }
                self.load_collection_commands();
                self.filter_commands();
                self.status_message = Some(format!("Deleted collection: {}", name));
                self.status_timestamp = Some(Instant::now());
            }
            Err(e) => eprintln!("DB error deleting collection: {}", e),
        }
    }

    pub fn add_command_to_collection(&mut self, cmd_text: &str, collection_id: &str) {
        let col_name = self
            .collections
            .iter()
            .find(|c| c.id == collection_id)
            .map(|c| c.name.clone());

        let cmd_id = crate::storage::collections::hash_command(cmd_text);

        if !self.commands.iter().any(|c| c.text == cmd_text) {
            self.commands.push(Command {
                id: cmd_id,
                text: cmd_text.to_string(),
                tags: vec![],
                collection_ids: vec![collection_id.to_string()],
                favorite: false,
                _context: vec![],
                use_count: 0,
                last_used: None,
            });
        }

        let conn = match self.db.as_ref() {
            Some(c) => c,
            None => return,
        };

        let result =
            crate::storage::collections::add_command_to_collection(conn, cmd_text, collection_id);

        if let Err(e) = result {
            eprintln!("DB error adding to collection: {}", e);
            return;
        }

        if let Some(name) = col_name {
            self.status_message = Some(format!("Added to {}", name));
            self.status_timestamp = Some(Instant::now());
        }

        match self.commands.iter_mut().find(|c| c.text == cmd_text) {
            Some(cmd) if !cmd.collection_ids.contains(&collection_id.to_string()) => {
                cmd.collection_ids.push(collection_id.to_string());
            }
            _ => {}
        }

        self.load_collection_commands();
        self.filter_commands();

        if let Some(idx) = self.filtered.iter().position(|c| c.text == cmd_text) {
            self.selected_index = idx;
        }
    }

    pub fn remove_command_from_collection(&mut self, cmd_text: &str) {
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
        match crate::storage::collections::remove_command_from_collection(conn, cmd_text, &id) {
            Ok(()) => {
                if let Some(cmd) = self.commands.iter_mut().find(|c| c.text == cmd_text) {
                    cmd.collection_ids.retain(|c| c != &id);
                }
                self.load_collection_commands();
                self.filter_commands();
                self.status_message = Some(format!("Removed from {}", name));
                self.status_timestamp = Some(Instant::now());
            }
            Err(e) => eprintln!("DB error removing from collection: {}", e),
        }
    }

    pub fn filtered_collections(&self, search: &str) -> Vec<&Collection> {
        if search.is_empty() {
            return self.collections.iter().collect();
        }
        self.collections
            .iter()
            .filter(|c| c.name.to_lowercase().contains(&search.to_lowercase()))
            .collect()
    }

    pub fn navigate_collection_up(&mut self) {
        self.selected_collection_index = self.selected_collection_index.saturating_sub(1);
        self.load_collection_commands();
        self.filter_commands();
    }

    pub fn navigate_collection_down(&mut self) {
        if !self.collections.is_empty() {
            self.selected_collection_index =
                (self.selected_collection_index + 1) % self.collections.len();
            self.load_collection_commands();
            self.filter_commands();
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

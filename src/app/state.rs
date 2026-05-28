use std::collections::HashSet;
use std::time::Instant;

use fuzzy_matcher::FuzzyMatcher;
use fuzzy_matcher::skim::SkimMatcherV2;
use ratatui::widgets::ListState;

use crate::input::help::GroupedShortcut;
use crate::storage::collections::Collection;
use crate::storage::import_export::{ImportMode, ImportPreview};
use crate::ui::theme::{CatppuccinFlavor, Theme};

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
    ImportExport,
}

#[derive(Clone, Debug, PartialEq)]
pub enum CollectionInputMode {
    None,
    AddToCollection,
    NewCollection,
    EditCollection,
    AddToCollectionSearch,
    ConfirmDeleteCollection,
    ConfirmDeleteCommand,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ImportExportMode {
    Export,
    Import,
    ImportPreview,
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
    pub add_command_search_index: usize,
    pub delete_confirm_text: String,
    pub terminal_height: u16,
    pub key_buffer: Option<char>,
    pub key_buffer_timestamp: Option<Instant>,
    pub help_open: bool,
    pub help_search_query: String,
    pub help_filtered_shortcuts: Vec<GroupedShortcut>,
    pub help_selected_index: usize,
    pub help_list_state: ListState,
    pub current_theme: Theme,
    pub theme_popup_open: bool,
    pub theme_popup_index: usize,
    pub theme_popup_list_state: ListState,
    pub saved_theme: Theme,
    pub export_popup_open: bool,
    pub import_popup_open: bool,
    pub import_export_file_path: String,
    pub import_mode_index: usize,
    pub import_preview: Option<ImportPreview>,
    pub import_export_mode: ImportExportMode,
}

impl AppState {
    pub fn bootstrap() -> Self {
        let mut db = match crate::storage::init_db() {
            Ok(conn) => Some(conn),
            Err(e) => {
                eprintln!("Failed to initialize database: {}", e);
                None
            }
        };

        let mut commands = crate::history::load_history();
        commands = crate::history::deduplicate(commands);

        if let Some(ref mut conn) = db {
            let cmd_refs: Vec<(&str, String)> = commands
                .iter()
                .map(|c| (c.text.as_str(), c.id.clone()))
                .collect();
            if let Err(e) = crate::storage::commands::ensure_commands_exist(conn, &cmd_refs) {
                eprintln!("Failed to save commands: {}", e);
            }
            crate::storage::hydrate_commands(conn, &mut commands);

            // Load DB-only commands (manually added to collections, not in shell history)
            #[allow(clippy::collapsible_if)]
            if let Ok(mut stmt) = conn.prepare("SELECT id, text FROM commands") {
                if let Ok(rows) = stmt.query_map([], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                }) {
                    for row in rows.flatten() {
                        let (db_id, db_text) = row;
                        if !commands.iter().any(|c| c.id == db_id) {
                            let mut cmd = Command {
                                id: db_id,
                                text: db_text.clone(),
                                tags: vec![],
                                collection_ids: vec![],
                                favorite: false,
                                _context: vec![],
                                use_count: 0,
                                last_used: None,
                            };
                            if let Some(meta) = crate::storage::load_metadata(conn, &db_text) {
                                cmd.favorite = meta.favorite;
                                if meta.use_count > cmd.use_count {
                                    cmd.use_count = meta.use_count;
                                }
                                cmd.last_used = meta.last_used;
                            }
                            let tags = crate::storage::load_tags(conn, &db_text);
                            if !tags.is_empty() {
                                cmd.tags = tags;
                            }
                            let collection_ids =
                                crate::storage::collections::get_collections_for_command(
                                    conn, &db_text,
                                )
                                .unwrap_or_default();
                            if !collection_ids.is_empty() {
                                cmd.collection_ids = collection_ids;
                            }
                            commands.push(cmd);
                        }
                    }
                }
            }
        }

        let mut state = AppState::new(commands, db);
        state.load_theme_from_db();
        state.load_collections();
        state
    }

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
        let help_list_state = {
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
            add_command_search_index: 0,
            delete_confirm_text: String::new(),
            terminal_height: 24,
            key_buffer: None,
            key_buffer_timestamp: None,
            help_open: false,
            help_search_query: String::new(),
            help_filtered_shortcuts: Vec::new(),
            help_selected_index: 0,
            help_list_state,
            current_theme: Theme::default(),
            theme_popup_open: false,
            theme_popup_index: 0,
            theme_popup_list_state: {
                let mut s = ListState::default();
                s.select(Some(0));
                s
            },
            saved_theme: Theme::default(),
            export_popup_open: false,
            import_popup_open: false,
            import_export_file_path: String::new(),
            import_mode_index: 0,
            import_preview: None,
            import_export_mode: ImportExportMode::Export,
        }
    }

    pub fn set_terminal_height(&mut self, height: u16) {
        self.terminal_height = height;
    }

    pub fn set_status_message(&mut self, msg: String) {
        self.status_message = Some(msg);
        self.status_timestamp = Some(Instant::now());
    }

    pub fn check_key_buffer_timeout(&mut self) {
        let timed_out = self
            .key_buffer_timestamp
            .map(|t| t.elapsed().as_millis() > 500)
            .unwrap_or(false);
        if timed_out {
            self.key_buffer = None;
            self.key_buffer_timestamp = None;
        }
    }

    pub fn set_key_buffer(&mut self, key: char) {
        self.key_buffer = Some(key);
        self.key_buffer_timestamp = Some(Instant::now());
    }

    pub fn clear_key_buffer(&mut self) {
        self.key_buffer = None;
        self.key_buffer_timestamp = None;
    }

    pub fn go_to_top(&mut self) {
        self.selected_index = 0;
        self.list_state.select(Some(0));
    }

    pub fn go_to_bottom(&mut self) {
        if !self.filtered.is_empty() {
            self.selected_index = self.filtered.len() - 1;
            self.list_state.select(Some(self.selected_index));
        }
    }

    pub fn go_to_collection_top(&mut self) {
        self.selected_collection_index = 0;
        self.collection_list_state.select(Some(0));
        self.load_collection_commands();
        self.filter_commands();
    }

    pub fn go_to_collection_bottom(&mut self) {
        if !self.collections.is_empty() {
            self.selected_collection_index = self.collections.len() - 1;
            self.collection_list_state
                .select(Some(self.selected_collection_index));
            self.load_collection_commands();
            self.filter_commands();
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

    pub fn pane_down(&mut self) {
        if self.view_mode == ViewMode::Collections {
            self.active_pane = match self.active_pane {
                ActivePane::Search => ActivePane::CollectionsList,
                ActivePane::CollectionsList => ActivePane::Search,
                ActivePane::CollectionItems => ActivePane::CollectionsList,
                _ => ActivePane::Search,
            };
        } else {
            self.active_pane = match self.active_pane {
                ActivePane::Search => ActivePane::History,
                _ => ActivePane::Search,
            };
        }
    }

    pub fn pane_up(&mut self) {
        if self.view_mode == ViewMode::Collections {
            self.active_pane = match self.active_pane {
                ActivePane::Search => ActivePane::CollectionsList,
                ActivePane::CollectionsList => ActivePane::Search,
                ActivePane::CollectionItems => ActivePane::CollectionsList,
                _ => ActivePane::Search,
            };
        } else {
            self.active_pane = match self.active_pane {
                ActivePane::History => ActivePane::Search,
                _ => ActivePane::History,
            };
        }
    }

    pub fn pane_left(&mut self) {
        if self.view_mode == ViewMode::Collections
            && self.active_pane == ActivePane::CollectionItems
        {
            self.active_pane = ActivePane::CollectionsList;
        }
    }

    pub fn pane_right(&mut self) {
        if self.view_mode == ViewMode::Collections
            && self.active_pane == ActivePane::CollectionsList
        {
            self.active_pane = ActivePane::CollectionItems;
        }
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

    pub fn navigate_page_down(&mut self) {
        if self.filtered.is_empty() {
            return;
        }
        let page_size = (self.terminal_height.saturating_sub(4) / 2).max(5) as usize;
        self.selected_index = (self.selected_index + page_size).min(self.filtered.len() - 1);
    }

    pub fn navigate_page_up(&mut self) {
        let page_size = (self.terminal_height.saturating_sub(4) / 2).max(5) as usize;
        self.selected_index = self.selected_index.saturating_sub(page_size);
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
        let cmd_idx = selected_id.and_then(|id| self.commands.iter().position(|c| c.id == id));
        if let Some(idx) = cmd_idx {
            let cmd = &mut self.commands[idx];
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

            if idx > 0 {
                let cmd = self.commands.remove(idx);
                self.commands.insert(0, cmd);
            }
            self.filter_commands();
            self.selected_index = 0;
            self.list_state.select(Some(0));
        }
    }

    pub fn mark_executed_for_text(&mut self, text: &str) {
        let cmd_idx = self.commands.iter().position(|c| c.text == text);
        if let Some(idx) = cmd_idx {
            let cmd = &mut self.commands[idx];
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

            if idx > 0 {
                let cmd = self.commands.remove(idx);
                self.commands.insert(0, cmd);
            }
            self.filter_commands();
            self.selected_index = 0;
            self.list_state.select(Some(0));
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
                format!("* Favorited: {}", cmd.text)
            } else {
                format!("* Unfavorited: {}", cmd.text)
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
        if let Some(col) = self.selected_collection() {
            self.delete_confirm_text = col.name.clone();
        }
        self.collection_input_mode = CollectionInputMode::ConfirmDeleteCollection;
        self.input_mode = InputMode::CollectionInput;
    }

    pub fn delete_collection_confirmed(&mut self) {
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
        self.input_mode = InputMode::Normal;
        self.collection_input_mode = CollectionInputMode::None;
    }

    pub fn add_command_to_collection(&mut self, cmd_text: &str, collection_id: &str) {
        // DB operation FIRST to keep state consistent on error
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

        // Then update in-memory state
        if !self.commands.iter().any(|c| c.text == cmd_text) {
            let cmd_id = crate::storage::collections::hash_command(cmd_text);
            self.commands.push(Command {
                id: cmd_id,
                text: cmd_text.to_string(),
                tags: vec![],
                collection_ids: vec![],
                favorite: false,
                _context: vec![],
                use_count: 0,
                last_used: None,
            });
        }

        match self.commands.iter_mut().find(|c| c.text == cmd_text) {
            Some(cmd) if !cmd.collection_ids.contains(&collection_id.to_string()) => {
                cmd.collection_ids.push(collection_id.to_string());
            }
            _ => {}
        }

        let col_name = self
            .collections
            .iter()
            .find(|c| c.id == collection_id)
            .map(|c| c.name.clone());

        if let Some(name) = col_name {
            self.status_message = Some(format!("Added to {}", name));
            self.status_timestamp = Some(Instant::now());
        }

        self.load_collection_commands();
        self.filter_commands();

        if let Some(idx) = self.filtered.iter().position(|c| c.text == cmd_text) {
            self.selected_index = idx;
        }
    }

    pub fn remove_command_from_collection(&mut self, cmd_text: &str) {
        self.delete_confirm_text = cmd_text.to_string();
        self.collection_input_mode = CollectionInputMode::ConfirmDeleteCommand;
        self.input_mode = InputMode::CollectionInput;
    }

    pub fn remove_command_from_collection_confirmed(&mut self, cmd_text: &str) {
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
        self.input_mode = InputMode::Normal;
        self.collection_input_mode = CollectionInputMode::None;
    }

    pub fn search_results_for_add_command(&self) -> Vec<&Command> {
        let query = self.collection_input_text.trim();
        let col_id = self.selected_collection().map(|c| c.id.clone());

        if query.is_empty() {
            return self
                .commands
                .iter()
                .filter(|c| {
                    !c.collection_ids
                        .contains(col_id.as_ref().unwrap_or(&"".to_string()))
                })
                .collect();
        }

        let _search_lower = query.to_lowercase();
        let mut scored: Vec<(i64, &Command)> = self
            .commands
            .iter()
            .filter(|c| {
                !c.collection_ids
                    .contains(col_id.as_ref().unwrap_or(&"".to_string()))
            })
            .filter_map(|cmd| {
                self.matcher
                    .fuzzy_indices(&cmd.text, query)
                    .map(|(score, _)| (score, cmd))
            })
            .collect();

        scored.sort_by_key(|b| std::cmp::Reverse(b.0));

        scored.into_iter().map(|(_, cmd)| cmd).collect()
    }

    pub fn add_command_to_collection_by_text(&mut self, cmd_text: &str) {
        let col = match self.selected_collection() {
            Some(c) => c,
            None => return,
        };
        let col_id = col.id.clone();

        let db_result: Result<(), rusqlite::Error> = self.db.as_ref().map_or(Ok(()), |conn| {
            crate::storage::collections::add_command_to_collection(conn, cmd_text, &col_id)
        });
        if let Err(e) = db_result {
            eprintln!("DB error adding to collection: {}", e);
            return;
        }

        if let Some(cmd) = self.commands.iter_mut().find(|c| c.text == cmd_text) {
            if !cmd.collection_ids.contains(&col_id) {
                cmd.collection_ids.push(col_id.clone());
            }
        } else {
            let cmd_id = crate::storage::collections::hash_command(cmd_text);
            self.commands.push(Command {
                id: cmd_id,
                text: cmd_text.to_string(),
                tags: vec![],
                collection_ids: vec![col_id.clone()],
                favorite: false,
                _context: vec![],
                use_count: 0,
                last_used: None,
            });
        }

        self.status_message = Some(format!("Added: {}", cmd_text));
        self.status_timestamp = Some(Instant::now());
        self.load_collection_commands();
        self.filter_commands();
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

    pub fn navigate_collection_page_down(&mut self) {
        if self.collections.is_empty() {
            return;
        }
        let page_size = (self.terminal_height.saturating_sub(4) / 2).max(5) as usize;
        self.selected_collection_index =
            (self.selected_collection_index + page_size).min(self.collections.len() - 1);
        self.load_collection_commands();
        self.filter_commands();
    }

    pub fn navigate_collection_page_up(&mut self) {
        if self.collections.is_empty() {
            return;
        }
        let page_size = (self.terminal_height.saturating_sub(4) / 2).max(5) as usize;
        self.selected_collection_index = self.selected_collection_index.saturating_sub(page_size);
        self.load_collection_commands();
        self.filter_commands();
    }

    pub fn open_theme_popup(&mut self) {
        self.saved_theme = self.current_theme.clone();
        self.theme_popup_open = true;
        for (i, flavor) in CatppuccinFlavor::all().iter().enumerate() {
            let t = flavor.theme();
            if t.focus_border == self.current_theme.focus_border {
                self.theme_popup_index = i;
                break;
            }
        }
        self.theme_popup_list_state
            .select(Some(self.theme_popup_index));
    }

    pub fn load_theme_from_db(&mut self) {
        let Some(ref conn) = self.db else { return };
        let Some(name) = crate::storage::load_theme(conn) else {
            return;
        };
        let theme = match name.as_str() {
            "Latte" => Theme::latte(),
            "Frappe" => Theme::frappe(),
            "Macchiato" => Theme::macchiato(),
            "Mocha" => Theme::mocha(),
            _ => Theme::default(),
        };
        self.current_theme = theme.clone();
        self.saved_theme = theme;
    }

    pub fn close_theme_popup(&mut self) {
        self.current_theme = self.saved_theme.clone();
        self.theme_popup_open = false;
    }

    pub fn apply_theme_and_close(&mut self) {
        let theme_name = self.current_theme.name().to_string();
        let Some(ref conn) = self.db else {
            self.theme_popup_open = false;
            return;
        };
        if let Err(e) = crate::storage::save_theme(conn, &theme_name) {
            eprintln!("Failed to save theme: {}", e);
        }
        self.theme_popup_open = false;
    }

    pub fn navigate_theme_popup_up(&mut self) {
        if self.theme_popup_index > 0 {
            self.theme_popup_index -= 1;
        } else {
            self.theme_popup_index = CatppuccinFlavor::all().len() - 1;
        }
        self.current_theme = CatppuccinFlavor::all()[self.theme_popup_index].theme();
        self.theme_popup_list_state
            .select(Some(self.theme_popup_index));
    }

    pub fn navigate_theme_popup_down(&mut self) {
        let max = CatppuccinFlavor::all().len() - 1;
        if self.theme_popup_index < max {
            self.theme_popup_index += 1;
        } else {
            self.theme_popup_index = 0;
        }
        self.current_theme = CatppuccinFlavor::all()[self.theme_popup_index].theme();
        self.theme_popup_list_state
            .select(Some(self.theme_popup_index));
    }

    pub fn open_export_popup(&mut self) {
        self.export_popup_open = true;
        self.import_popup_open = false;
        self.input_mode = InputMode::ImportExport;
        self.import_export_mode = ImportExportMode::Export;
        self.import_export_file_path.clear();
        self.import_preview = None;
        self.import_mode_index = 0;
    }

    pub fn open_import_popup(&mut self) {
        self.import_popup_open = true;
        self.export_popup_open = false;
        self.input_mode = InputMode::ImportExport;
        self.import_export_mode = ImportExportMode::Import;
        self.import_export_file_path.clear();
        self.import_preview = None;
        self.import_mode_index = 0;
    }

    pub fn close_import_export_popup(&mut self) {
        self.export_popup_open = false;
        self.import_popup_open = false;
        self.input_mode = InputMode::Normal;
        self.import_export_file_path.clear();
        self.import_preview = None;
    }

    pub fn preview_import(&mut self) {
        if self.import_export_file_path.is_empty() {
            return;
        }

        let content = match std::fs::read_to_string(&self.import_export_file_path) {
            Ok(c) => c,
            Err(e) => {
                self.set_status_message(format!("Error: {}", e));
                return;
            }
        };

        let data: crate::storage::import_export::ExportData = match serde_json::from_str(&content) {
            Ok(d) => d,
            Err(e) => {
                self.set_status_message(format!("Invalid JSON: {}", e));
                return;
            }
        };

        if data.version != 1 {
            self.set_status_message(format!("Unsupported version {}", data.version));
            return;
        }

        let Some(ref conn) = self.db else {
            self.set_status_message("No database connection".to_string());
            return;
        };

        match crate::storage::import_export::preview_import(conn, &data) {
            Ok(preview) => {
                self.import_preview = Some(preview);
                self.import_export_mode = ImportExportMode::ImportPreview;
            }
            Err(e) => {
                self.set_status_message(format!("Preview error: {}", e));
            }
        }
    }

    pub fn execute_import(&mut self) {
        if self.import_preview.is_none() {
            return;
        }

        let content = match std::fs::read_to_string(&self.import_export_file_path) {
            Ok(c) => c,
            Err(e) => {
                self.set_status_message(format!("Error: {}", e));
                return;
            }
        };

        let data: crate::storage::import_export::ExportData = match serde_json::from_str(&content) {
            Ok(d) => d,
            Err(e) => {
                self.set_status_message(format!("Invalid JSON: {}", e));
                return;
            }
        };

        let mode = if self.import_mode_index == 1 {
            ImportMode::Replace
        } else {
            ImportMode::Merge
        };

        let Some(ref mut conn) = self.db else {
            self.set_status_message("No database connection".to_string());
            return;
        };

        match crate::storage::import_export::import_data(conn, &data, &mode) {
            Ok(result) => {
                let mut msg = String::new();
                if result.imported_commands > 0 {
                    msg.push_str(&format!("Imported {} commands", result.imported_commands));
                }
                if result.imported_collections > 0 {
                    if !msg.is_empty() {
                        msg.push_str(", ");
                    }
                    msg.push_str(&format!("{} collections", result.imported_collections));
                }
                if result.skipped_commands > 0 {
                    msg.push_str(&format!(", skipped {}", result.skipped_commands));
                }
                self.set_status_message(msg);

                let commands = crate::history::load_history();
                let commands = crate::history::deduplicate(commands);
                if let Some(ref mut c) = self.db {
                    let cmd_refs: Vec<(&str, String)> = commands
                        .iter()
                        .map(|c| (c.text.as_str(), c.id.clone()))
                        .collect();
                    if let Err(e) = crate::storage::commands::ensure_commands_exist(c, &cmd_refs) {
                        eprintln!("Failed to sync commands: {}", e);
                    }
                    self.commands.clear();
                    self.commands.extend(commands.into_iter().map(|cmd| {
                        let mut c2 = cmd;
                        if let Some(meta) = crate::storage::load_metadata(c, &c2.text) {
                            c2.favorite = meta.favorite;
                            c2.use_count = meta.use_count.max(c2.use_count);
                            if meta.last_used > c2.last_used {
                                c2.last_used = meta.last_used;
                            }
                        }
                        let tags = crate::storage::load_tags(c, &c2.text);
                        if !tags.is_empty() {
                            c2.tags = tags;
                        }
                        let cols =
                            crate::storage::collections::get_collections_for_command(c, &c2.text)
                                .unwrap_or_default();
                        if !cols.is_empty() {
                            c2.collection_ids = cols;
                        }
                        c2
                    }));
                }
                self.filter_commands();
            }
            Err(e) => {
                self.set_status_message(format!("Import error: {}", e));
            }
        }

        self.close_import_export_popup();
    }

    pub fn execute_export(&mut self) {
        if self.import_export_file_path.is_empty() {
            return;
        }

        let Some(ref conn) = self.db else {
            self.set_status_message("No database connection".to_string());
            return;
        };

        match crate::storage::import_export::export_data(conn) {
            Ok(data) => {
                let json = match serde_json::to_string_pretty(&data) {
                    Ok(j) => j,
                    Err(e) => {
                        self.set_status_message(format!("JSON error: {}", e));
                        return;
                    }
                };

                if let Err(e) = std::fs::write(&self.import_export_file_path, &json) {
                    self.set_status_message(format!("Write error: {}", e));
                    return;
                }

                self.set_status_message(format!(
                    "Exported {} commands to {}",
                    data.commands.len(),
                    self.import_export_file_path
                ));
            }
            Err(e) => {
                self.set_status_message(format!("Export error: {}", e));
            }
        }

        self.close_import_export_popup();
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

impl AppState {}

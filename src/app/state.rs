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
    /// Times this command was recorded in the directory ctrlr was launched
    /// from. Populated from `command_runs`; 0 when nothing was recorded.
    pub runs_here: i32,
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
    /// Canonical directory ctrlr was launched from, i.e. the shell's. `None`
    /// when it cannot be read.
    pub cwd: Option<String>,
    /// Restricts the list to commands recorded in [`AppState::cwd`].
    pub scope_to_cwd: bool,
    /// Offers to install or update the shell integration on startup.
    pub integration_popup_open: bool,
    pub integration_shell: Option<crate::cli::shells::Shell>,
    pub integration_state: Option<crate::cli::shells::IntegrationState>,
    /// Set once the popup has written the config, switching it to its result
    /// view.
    pub integration_installed: bool,
    pub integration_message: Option<String>,
    /// Whether the selection is handed back through `--output-file`, i.e.
    /// whether ctrlr can put a shell reload on the prompt line.
    pub writes_to_output_file: bool,
    /// Declining now retires the offer for this integration, so the popup says
    /// so instead of quietly never returning.
    pub integration_final_offer: bool,
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

        // Drained before the history is read so this session's commands are
        // already counted, and before `ensure_commands_exist` so a command that
        // ran but has not reached the history file yet still gets a row.
        if let Some(ref mut conn) = db {
            let entries: Vec<crate::history::runs::RunEntry> =
                crate::history::runs::take_run_log(&crate::storage::runs_log_path())
                    .into_iter()
                    .map(|mut entry| {
                        entry.cwd = crate::history::runs::canonical_dir(&entry.cwd);
                        entry
                    })
                    .collect();

            if let Err(e) = crate::storage::runs::record_runs(conn, &entries) {
                eprintln!("Failed to record command runs: {}", e);
            }
        }

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
                                runs_here: 0,
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

        let cwd = crate::history::runs::current_dir();

        // One grouped query for the whole list rather than a lookup per
        // command: this runs on every launch.
        if let (Some(conn), Some(dir)) = (&db, &cwd) {
            let counts = crate::storage::runs::runs_in_dir(conn, dir);
            for cmd in &mut commands {
                cmd.runs_here = counts.get(&cmd.id).copied().unwrap_or(0);
            }
        }

        let mut state = AppState::new(commands, db);
        state.cwd = cwd;
        state.load_theme_from_db();
        state.load_collections();
        state.check_integration();
        state
    }

    /// Key under which a dismissed offer is remembered.
    const INTEGRATION_DISMISSED_KEY: &'static str = "integration_prompt_dismissed";

    /// Decides whether to offer installing or updating the shell integration.
    ///
    /// The stdout warning this backs up is printed before the alternate screen
    /// opens, so the user only sees it after quitting — which is how an
    /// out-of-date integration stays unnoticed.
    fn check_integration(&mut self) {
        use crate::cli::shells::{self, IntegrationState};

        let Some((shell, state)) = shells::detect_integration_state() else {
            return;
        };

        if state == IntegrationState::Current {
            return;
        }

        let stored = self
            .db
            .as_ref()
            .and_then(|conn| crate::storage::load_setting(conn, Self::INTEGRATION_DISMISSED_KEY));

        let decision = integration_offer(stored.as_deref(), &shells::script_fingerprint(shell));

        if let (Some(conn), Some(record)) = (self.db.as_ref(), &decision.record) {
            let _ = crate::storage::save_setting(conn, Self::INTEGRATION_DISMISSED_KEY, record);
        }

        if !decision.show {
            return;
        }

        self.integration_final_offer = decision.final_offer;
        self.integration_shell = Some(shell);
        self.integration_state = Some(state);
        self.integration_popup_open = true;
    }

    /// Writes the integration and switches the popup to its result view.
    ///
    /// Returns the command that reloads the shell when ctrlr can put one on the
    /// prompt line, which is only possible through `--output-file`: ctrlr is a
    /// child process and cannot source anything into the shell that started it.
    pub fn install_integration(&mut self) -> Option<String> {
        let shell = self.integration_shell?;

        match crate::cli::init::install_integration(shell) {
            Ok(outcome) => {
                self.integration_installed = true;
                self.integration_message = Some(match &outcome.backup {
                    Some(backup) => format!(
                        "Wrote {}\nPrevious config saved to {}",
                        outcome.config_path.display(),
                        backup.display()
                    ),
                    None => format!("Wrote {}", outcome.config_path.display()),
                });

                // With a reload on the prompt line ctrlr is about to exit, so
                // the result view is never seen. Without one the popup stays
                // up: "restart your shell" in a status message that expires
                // after two seconds is too easy to miss.
                if self.writes_to_output_file {
                    self.integration_popup_open = false;
                    return Some(crate::cli::shells::reload_command(shell).to_string());
                }
                None
            }
            Err(e) => {
                self.integration_message = Some(format!("Could not update the config: {}", e));
                None
            }
        }
    }

    /// Silences the popup for [`INTEGRATION_REASK_AFTER`] launches.
    pub fn dismiss_integration_popup(&mut self) {
        self.integration_popup_open = false;

        let Some(shell) = self.integration_shell else {
            return;
        };

        let fingerprint = crate::cli::shells::script_fingerprint(shell);
        let previous = self
            .db
            .as_ref()
            .and_then(|conn| crate::storage::load_setting(conn, Self::INTEGRATION_DISMISSED_KEY))
            .and_then(|raw| serde_json::from_str::<Dismissal>(&raw).ok())
            .filter(|d| d.fingerprint == fingerprint);

        let record = Dismissal {
            declines: previous.map(|d| d.declines).unwrap_or(0).saturating_add(1),
            fingerprint,
            launches: 0,
        };

        if let (Some(conn), Ok(encoded)) = (self.db.as_ref(), record.encode()) {
            let _ = crate::storage::save_setting(conn, Self::INTEGRATION_DISMISSED_KEY, &encoded);
        }
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
            cwd: None,
            scope_to_cwd: false,
            integration_popup_open: false,
            integration_shell: None,
            integration_state: None,
            integration_installed: false,
            integration_message: None,
            writes_to_output_file: false,
            integration_final_offer: false,
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

    /// Clears the query and returns the selection to the top.
    ///
    /// `filter_commands` only resets the selection while a query is present, so
    /// without the explicit reset the cursor would sit on whatever index it had
    /// once the full list comes back — an unrelated command.
    pub fn clear_search(&mut self) {
        self.search_query.clear();
        self.filter_commands();
        self.selected_index = 0;
        self.list_state.select(Some(0));
    }

    /// Flips the directory scope and returns the message to show for it.
    ///
    /// Refuses when there is nothing recorded for this directory, rather than
    /// scoping the list down to nothing: the run log only fills going forward,
    /// so an empty result is the normal state right after installing.
    pub fn toggle_cwd_scope(&mut self) -> String {
        if self.cwd.is_none() {
            return "Current directory unknown".to_string();
        }

        if !self.scope_to_cwd && !self.commands.iter().any(|c| c.runs_here > 0) {
            return "Nothing recorded in this directory yet".to_string();
        }

        self.scope_to_cwd = !self.scope_to_cwd;
        self.filter_commands();
        self.selected_index = 0;
        self.list_state.select(Some(0));

        if self.scope_to_cwd {
            format!("Scoped to {}", self.cwd_display())
        } else {
            "Showing all directories".to_string()
        }
    }

    /// The current directory shortened for display: `~` for home, and only the
    /// last two components of anything longer.
    pub fn cwd_display(&self) -> String {
        let Some(cwd) = &self.cwd else {
            return "?".to_string();
        };

        let home = dirs::home_dir().map(|h| h.to_string_lossy().into_owned());
        let shortened = match &home {
            Some(home) if cwd == home => return "~".to_string(),
            Some(home) => cwd.strip_prefix(home).map(|rest| format!("~{}", rest)),
            None => None,
        };
        let path = shortened.unwrap_or_else(|| cwd.clone());

        let parts: Vec<&str> = path.split('/').filter(|p| !p.is_empty()).collect();
        if parts.len() > 2 {
            format!(".../{}", parts[parts.len() - 2..].join("/"))
        } else {
            path
        }
    }

    pub fn filter_commands(&mut self) {
        let scope_to_cwd = self.scope_to_cwd;
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

        // Applied to the base list, so it holds with an empty query too — the
        // branch below only sorts when there is something to score.
        let base_commands: Vec<&Command> = if scope_to_cwd {
            base_commands
                .into_iter()
                .filter(|c| c.runs_here > 0)
                .collect()
        } else {
            base_commands
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

    /// Returns true when the caller should quit — i.e. there was no query to clear.
    pub fn handle_esc(&mut self) -> bool {
        if self.search_query.is_empty() {
            true
        } else {
            self.clear_search();
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
            let cmd_id = crate::hash::hash_command(cmd_text);
            self.commands.push(Command {
                id: cmd_id,
                text: cmd_text.to_string(),
                tags: vec![],
                collection_ids: vec![],
                favorite: false,
                _context: vec![],
                use_count: 0,
                last_used: None,
                runs_here: 0,
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
            let cmd_id = crate::hash::hash_command(cmd_text);
            self.commands.push(Command {
                id: cmd_id,
                text: cmd_text.to_string(),
                tags: vec![],
                collection_ids: vec![col_id.clone()],
                favorite: false,
                _context: vec![],
                use_count: 0,
                last_used: None,
                runs_here: 0,
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

/// Launches to stay quiet for after the integration offer is declined.
///
/// Counted in launches rather than days because ctrlr has no scheduler of its
/// own, and kept high because the popup lands on the `Ctrl+R` hot path: a user
/// who opened ctrlr to run a command does not want to answer a question first.
const INTEGRATION_REASK_AFTER: u32 = 40;

/// Declines after which the offer is dropped for this script. Three no's are an
/// answer; only a changed integration is a new question worth asking.
const INTEGRATION_MAX_DECLINES: u32 = 3;

/// A declined integration offer, as stored in `settings`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct Dismissal {
    /// The script that was declined. A different one is a different offer.
    fingerprint: String,
    /// Launches counted since the dismissal.
    #[serde(default)]
    launches: u32,
    /// How often this script has been declined.
    #[serde(default)]
    declines: u32,
}

impl Dismissal {
    fn encode(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }
}

/// What to do about the integration offer this launch.
struct IntegrationOffer {
    show: bool,
    /// The record to persist, when it changed.
    record: Option<String>,
    /// Whether declining now is the last decline this script gets, so the
    /// popup can say so rather than going quiet without explanation.
    final_offer: bool,
}

/// Pure policy behind [`AppState::check_integration`], so the counting is
/// testable without a shell config or a database.
///
/// A stored record for a different script is stale: that offer was never
/// declined, so it is made again.
fn integration_offer(stored: Option<&str>, fingerprint: &str) -> IntegrationOffer {
    let dismissal = stored
        .and_then(|raw| serde_json::from_str::<Dismissal>(raw).ok())
        .filter(|d| d.fingerprint == fingerprint);

    let Some(dismissal) = dismissal else {
        return IntegrationOffer {
            show: true,
            record: None,
            final_offer: INTEGRATION_MAX_DECLINES <= 1,
        };
    };

    if dismissal.declines >= INTEGRATION_MAX_DECLINES {
        // Answered often enough. No counting either: there is nothing left to
        // count towards.
        return IntegrationOffer {
            show: false,
            record: None,
            final_offer: false,
        };
    }

    let launches = dismissal.launches.saturating_add(1);
    if launches >= INTEGRATION_REASK_AFTER {
        // Asked again, and the count only resets once it is declined again.
        return IntegrationOffer {
            show: true,
            record: None,
            final_offer: dismissal.declines + 1 >= INTEGRATION_MAX_DECLINES,
        };
    }

    IntegrationOffer {
        show: false,
        record: Dismissal {
            fingerprint: fingerprint.to_string(),
            launches,
            declines: dismissal.declines,
        }
        .encode()
        .ok(),
        final_offer: false,
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

    // Weighted above recency and below an explicit favourite: what you ran in
    // this directory is usually what you want here, but not more than what you
    // deliberately starred. Capped so one hot command cannot dominate.
    let here = if cmd.runs_here > 0 {
        60 + cmd.runs_here.min(20) as i64
    } else {
        0
    };

    fuzzy * 10 + usage + recency + favorite + here
}

impl AppState {}

#[cfg(test)]
mod tests {
    use super::*;

    fn cmd(text: &str) -> Command {
        Command {
            id: text.to_string(),
            text: text.to_string(),
            tags: vec![],
            collection_ids: vec![],
            favorite: false,
            _context: vec![],
            use_count: 0,
            last_used: None,
            runs_here: 0,
        }
    }

    fn state_with(texts: &[&str]) -> AppState {
        AppState::new(texts.iter().map(|t| cmd(t)).collect(), None)
    }

    /// A state where `here` was recorded in the current directory and the rest
    /// were not.
    fn state_with_runs_here(texts: &[&str], here: &[&str]) -> AppState {
        let commands = texts
            .iter()
            .map(|t| {
                let mut c = cmd(t);
                if here.contains(t) {
                    c.runs_here = 3;
                }
                c
            })
            .collect();
        let mut state = AppState::new(commands, None);
        state.cwd = Some("/work/repo".to_string());
        state
    }

    fn dismissed(fingerprint: &str, launches: u32) -> String {
        declined(fingerprint, launches, 1)
    }

    fn declined(fingerprint: &str, launches: u32, declines: u32) -> String {
        Dismissal {
            fingerprint: fingerprint.to_string(),
            launches,
            declines,
        }
        .encode()
        .unwrap()
    }

    #[test]
    fn test_integration_offered_when_never_dismissed() {
        let offer = integration_offer(None, "abc");
        assert!(offer.show);
        assert!(offer.record.is_none(), "nothing to count yet");
    }

    #[test]
    fn test_integration_silent_right_after_dismissal() {
        let offer = integration_offer(Some(&dismissed("abc", 0)), "abc");
        assert!(!offer.show);
        assert_eq!(offer.record, Some(dismissed("abc", 1)));
    }

    #[test]
    fn test_integration_counts_launches_until_the_threshold() {
        let mut stored = dismissed("abc", 0);
        for _ in 0..INTEGRATION_REASK_AFTER - 1 {
            let offer = integration_offer(Some(&stored), "abc");
            assert!(!offer.show);
            stored = offer.record.unwrap();
        }

        let offer = integration_offer(Some(&stored), "abc");
        assert!(offer.show, "asked again after {}", INTEGRATION_REASK_AFTER);
    }

    #[test]
    fn test_integration_keeps_asking_until_dismissed_again() {
        // The count is only reset by a dismissal, so quitting without
        // answering does not buy another 40 launches of silence.
        let stored = dismissed("abc", INTEGRATION_REASK_AFTER);
        assert!(integration_offer(Some(&stored), "abc").show);
        assert!(integration_offer(Some(&stored), "abc").show);
    }

    #[test]
    fn test_integration_stops_asking_after_the_decline_cap() {
        // Three no's are an answer; only a changed script asks again.
        let stored = declined("abc", INTEGRATION_REASK_AFTER, INTEGRATION_MAX_DECLINES);
        let offer = integration_offer(Some(&stored), "abc");

        assert!(!offer.show);
        assert!(offer.record.is_none(), "nothing left to count");
        assert!(integration_offer(Some(&stored), "def").show);
    }

    #[test]
    fn test_integration_announces_the_last_offer() {
        let last = declined("abc", INTEGRATION_REASK_AFTER, INTEGRATION_MAX_DECLINES - 1);
        assert!(integration_offer(Some(&last), "abc").final_offer);

        let earlier = declined("abc", INTEGRATION_REASK_AFTER, 1);
        assert!(!integration_offer(Some(&earlier), "abc").final_offer);
    }

    #[test]
    fn test_integration_counting_preserves_the_decline_count() {
        let offer = integration_offer(Some(&declined("abc", 0, 2)), "abc");
        assert_eq!(offer.record, Some(declined("abc", 1, 2)));
    }

    #[test]
    fn test_integration_offered_again_when_the_script_changes() {
        let offer = integration_offer(Some(&dismissed("abc", 1)), "def");
        assert!(offer.show, "a different script is a different offer");
    }

    #[test]
    fn test_integration_tolerates_a_corrupt_record() {
        assert!(integration_offer(Some("not json"), "abc").show);
        assert!(integration_offer(Some(""), "abc").show);
    }

    #[test]
    fn test_scope_filters_with_empty_query() {
        // The empty-query branch does no scoring, so the scope has to be
        // applied to the base list or it looks broken until you type.
        let mut state = state_with_runs_here(&["cargo build", "ssh box", "ls"], &["cargo build"]);
        state.toggle_cwd_scope();

        assert_eq!(state.filtered.len(), 1);
        assert_eq!(state.filtered[0].text, "cargo build");
    }

    #[test]
    fn test_scope_filters_with_query() {
        let mut state =
            state_with_runs_here(&["cargo build", "cargo test", "cargo fmt"], &["cargo test"]);
        state.toggle_cwd_scope();
        state.search_query = "cargo".to_string();
        state.filter_commands();

        assert_eq!(state.filtered.len(), 1);
        assert_eq!(state.filtered[0].text, "cargo test");
    }

    #[test]
    fn test_toggle_scope_off_restores_the_full_list() {
        let mut state = state_with_runs_here(&["cargo build", "ssh box"], &["cargo build"]);
        state.toggle_cwd_scope();
        assert_eq!(state.filtered.len(), 1);

        state.toggle_cwd_scope();
        assert!(!state.scope_to_cwd);
        assert_eq!(state.filtered.len(), 2);
    }

    #[test]
    fn test_toggle_scope_refuses_when_nothing_recorded_here() {
        // Right after installing the integration nothing is recorded yet;
        // scoping to an empty list would just look broken.
        let mut state = state_with_runs_here(&["cargo build", "ssh box"], &[]);
        let message = state.toggle_cwd_scope();

        assert!(!state.scope_to_cwd);
        assert_eq!(state.filtered.len(), 2);
        assert!(message.contains("Nothing recorded"));
    }

    #[test]
    fn test_toggle_scope_without_a_known_cwd() {
        let mut state = state_with(&["cargo build"]);
        let message = state.toggle_cwd_scope();

        assert!(!state.scope_to_cwd);
        assert!(message.contains("unknown"));
    }

    #[test]
    fn test_ranking_prefers_commands_run_here() {
        let mut state = state_with_runs_here(&["cargo build", "cargo test"], &["cargo test"]);
        state.search_query = "cargo".to_string();
        state.filter_commands();

        assert_eq!(
            state.filtered[0].text, "cargo test",
            "a command recorded in this directory outranks an equal one that was not"
        );
    }

    #[test]
    fn test_favorite_still_outranks_the_directory() {
        let mut commands = vec![cmd("cargo build"), cmd("cargo test")];
        commands[0].favorite = true;
        commands[1].runs_here = 20;
        let mut state = AppState::new(commands, None);
        state.search_query = "cargo".to_string();
        state.filter_commands();

        assert_eq!(state.filtered[0].text, "cargo build");
    }

    #[test]
    fn test_cwd_display_shortens_long_paths() {
        let mut state = state_with(&[]);
        state.cwd = Some("/home/u/dev/rust/ctrlr".to_string());
        assert_eq!(state.cwd_display(), ".../rust/ctrlr");

        state.cwd = Some("/tmp".to_string());
        assert_eq!(state.cwd_display(), "/tmp");

        state.cwd = None;
        assert_eq!(state.cwd_display(), "?");
    }

    #[test]
    fn test_clear_search_resets_query_and_list() {
        let mut state = state_with(&["git status", "cargo build", "ls -la"]);
        state.search_query = "git".to_string();
        state.filter_commands();
        assert_eq!(state.filtered.len(), 1);

        state.clear_search();

        assert!(state.search_query.is_empty());
        assert_eq!(state.filtered.len(), 3, "full list must come back");
    }

    #[test]
    fn test_clear_search_resets_selection_to_top() {
        let mut state = state_with(&["git status", "cargo build", "ls -la"]);
        state.search_query = "a".to_string();
        state.filter_commands();
        // Walked down the filtered results before clearing.
        state.selected_index = 1;
        state.list_state.select(Some(1));

        state.clear_search();

        // filter_commands alone would leave these at 1, pointing at an
        // unrelated command once the full list returns.
        assert_eq!(state.selected_index, 0);
        assert_eq!(state.list_state.selected(), Some(0));
    }

    #[test]
    fn test_clear_search_on_empty_query_is_noop() {
        let mut state = state_with(&["git status", "cargo build"]);
        state.clear_search();
        state.clear_search();

        assert!(state.search_query.is_empty());
        assert_eq!(state.filtered.len(), 2);
        assert_eq!(state.selected_index, 0);
    }

    #[test]
    fn test_handle_esc_clears_query_without_quitting() {
        let mut state = state_with(&["git status", "cargo build"]);
        state.search_query = "git".to_string();
        state.filter_commands();

        let should_quit = state.handle_esc();

        assert!(!should_quit, "Esc with a query must not quit");
        assert!(state.search_query.is_empty());
        assert_eq!(state.filtered.len(), 2);
    }

    #[test]
    fn test_handle_esc_on_empty_query_signals_quit() {
        let mut state = state_with(&["git status"]);
        assert!(state.handle_esc(), "Esc with no query must quit");
    }
}

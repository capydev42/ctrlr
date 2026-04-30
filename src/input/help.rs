use crate::app::{Action, ActivePane, AppState, InputMode, ViewMode};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use fuzzy_matcher::FuzzyMatcher;
use fuzzy_matcher::skim::SkimMatcherV2;
use std::time::Instant;

#[derive(Clone, Debug)]
pub struct GroupedShortcut {
    pub action_id: &'static str,
    pub action_name: &'static str,
    pub description: &'static str,
    pub keys: Vec<&'static str>,
    pub category: &'static str,
}

pub fn get_all_shortcuts() -> Vec<GroupedShortcut> {
    vec![
        GroupedShortcut {
            action_id: "navigate_down",
            action_name: "Navigate Down",
            description: "Move selection one item down",
            keys: vec!["j", "↓"],
            category: "Navigation",
        },
        GroupedShortcut {
            action_id: "navigate_up",
            action_name: "Navigate Up",
            description: "Move selection one item up",
            keys: vec!["k", "↑"],
            category: "Navigation",
        },
        GroupedShortcut {
            action_id: "page_down",
            action_name: "Page Down",
            description: "Scroll down ~50% of list",
            keys: vec!["Ctrl+d", "PageDown"],
            category: "Navigation",
        },
        GroupedShortcut {
            action_id: "page_up",
            action_name: "Page Up",
            description: "Scroll up ~50% of list",
            keys: vec!["Ctrl+u", "PageUp"],
            category: "Navigation",
        },
        GroupedShortcut {
            action_id: "go_to_top",
            action_name: "Go to Top",
            description: "Jump to first item (press gg)",
            keys: vec!["g", "gg"],
            category: "Navigation",
        },
        GroupedShortcut {
            action_id: "go_to_bottom",
            action_name: "Go to Bottom",
            description: "Jump to last item",
            keys: vec!["G"],
            category: "Navigation",
        },
        GroupedShortcut {
            action_id: "execute",
            action_name: "Execute Command",
            description: "Runs selected command in terminal",
            keys: vec!["Enter"],
            category: "Actions",
        },
        GroupedShortcut {
            action_id: "toggle_favorite",
            action_name: "Toggle Favorite",
            description: "Mark/unmark as favorite",
            keys: vec!["f"],
            category: "Actions",
        },
        GroupedShortcut {
            action_id: "copy_to_clipboard",
            action_name: "Copy to Clipboard",
            description: "Copy command to clipboard",
            keys: vec!["y"],
            category: "Actions",
        },
        GroupedShortcut {
            action_id: "edit_tags",
            action_name: "Edit Tags",
            description: "Add/remove tags from command",
            keys: vec!["t"],
            category: "Actions",
        },
        GroupedShortcut {
            action_id: "add_to_collection",
            action_name: "Add to Collection",
            description: "Add command to collection",
            keys: vec!["c"],
            category: "Actions",
        },
        GroupedShortcut {
            action_id: "toggle_details",
            action_name: "Toggle Details",
            description: "Show/hide details panel",
            keys: vec!["d"],
            category: "Actions",
        },
        GroupedShortcut {
            action_id: "change_theme",
            action_name: "Change Theme",
            description: "Open theme selector popup",
            keys: vec!["Ctrl+t"],
            category: "Actions",
        },
        GroupedShortcut {
            action_id: "focus_search",
            action_name: "Focus Search",
            description: "Move cursor to search field",
            keys: vec!["/"],
            category: "Actions",
        },
        GroupedShortcut {
            action_id: "switch_pane",
            action_name: "Switch Pane",
            description: "Cycle through panes",
            keys: vec!["Tab"],
            category: "Panels",
        },
        GroupedShortcut {
            action_id: "pane_down",
            action_name: "Pane Down",
            description: "Focus pane below",
            keys: vec!["Ctrl+j"],
            category: "Panels",
        },
        GroupedShortcut {
            action_id: "pane_up",
            action_name: "Pane Up",
            description: "Focus pane above",
            keys: vec!["Ctrl+k"],
            category: "Panels",
        },
        GroupedShortcut {
            action_id: "pane_left",
            action_name: "Pane Left",
            description: "Focus pane on left",
            keys: vec!["Ctrl+h"],
            category: "Panels",
        },
        GroupedShortcut {
            action_id: "pane_right",
            action_name: "Pane Right",
            description: "Focus pane on right",
            keys: vec!["Ctrl+l"],
            category: "Panels",
        },
        GroupedShortcut {
            action_id: "view_history",
            action_name: "History View",
            description: "Show all commands",
            keys: vec!["1"],
            category: "Views",
        },
        GroupedShortcut {
            action_id: "view_favorites",
            action_name: "Favorites View",
            description: "Show favorites only",
            keys: vec!["2"],
            category: "Views",
        },
        GroupedShortcut {
            action_id: "view_collections",
            action_name: "Collections View",
            description: "Show collections",
            keys: vec!["3"],
            category: "Views",
        },
        GroupedShortcut {
            action_id: "new_collection",
            action_name: "New Collection",
            description: "Create new collection",
            keys: vec!["n"],
            category: "Collections",
        },
        GroupedShortcut {
            action_id: "edit_collection",
            action_name: "Edit Collection",
            description: "Rename collection",
            keys: vec!["e"],
            category: "Collections",
        },
        GroupedShortcut {
            action_id: "delete_collection",
            action_name: "Delete Collection",
            description: "Delete selected collection",
            keys: vec!["d"],
            category: "Collections",
        },
        GroupedShortcut {
            action_id: "search_collection",
            action_name: "Search to Add",
            description: "Search commands to add",
            keys: vec!["a"],
            category: "Collections",
        },
        GroupedShortcut {
            action_id: "remove_from_collection",
            action_name: "Remove from Collection",
            description: "Remove command from collection",
            keys: vec!["r"],
            category: "Collections",
        },
    ]
}

pub fn filter_shortcuts(shortcuts: &[GroupedShortcut], query: &str) -> Vec<GroupedShortcut> {
    if query.is_empty() {
        return shortcuts.to_vec();
    }

    let matcher = SkimMatcherV2::default();
    let query_lower = query.to_lowercase();

    let mut scored: Vec<(i64, GroupedShortcut)> = shortcuts
        .iter()
        .filter_map(|sc| {
            let name_score = matcher
                .fuzzy_indices(sc.action_name, &query_lower)
                .map(|(s, _)| s);
            let desc_score = matcher
                .fuzzy_indices(sc.description, &query_lower)
                .map(|(s, _)| s / 2);
            let keys_match = sc
                .keys
                .iter()
                .any(|k| k.to_lowercase().contains(&query_lower));
            let key_score: i64 = if keys_match { 1000 } else { 0 };

            let best_name = name_score.unwrap_or(0);
            let best_desc = desc_score.unwrap_or(0);
            let total_score = best_name.max(best_desc) + key_score;

            if total_score > 0 {
                Some((total_score, sc.clone()))
            } else {
                None
            }
        })
        .collect();

    scored.sort_by_key(|(score, _)| std::cmp::Reverse(*score));
    scored.into_iter().map(|(_, sc)| sc).collect()
}

pub fn get_shortcuts_for_context(state: &AppState) -> Vec<GroupedShortcut> {
    let all = get_all_shortcuts();

    match (&state.view_mode, &state.active_pane, &state.input_mode) {
        (ViewMode::History, ActivePane::Search, InputMode::Normal) => all
            .into_iter()
            .filter(|sc| {
                matches!(
                    sc.action_id,
                    "execute"
                        | "navigate_down"
                        | "navigate_up"
                        | "page_down"
                        | "page_up"
                        | "go_to_top"
                        | "go_to_bottom"
                        | "focus_search"
                        | "view_favorites"
                        | "view_collections"
                        | "change_theme"
                        | "pane_down"
                        | "pane_up"
                )
            })
            .collect(),
        (ViewMode::History, ActivePane::History, InputMode::Normal) => all
            .into_iter()
            .filter(|sc| {
                matches!(sc.action_id, |"execute"| "navigate_down"
                    | "navigate_up"
                    | "page_down"
                    | "page_up"
                    | "go_to_top"
                    | "go_to_bottom"
                    | "toggle_favorite"
                    | "copy_to_clipboard"
                    | "edit_tags"
                    | "add_to_collection"
                    | "toggle_details"
                    | "focus_search"
                    | "switch_pane"
                    | "view_favorites"
                    | "view_collections"
                    | "change_theme"
                    | "pane_down"
                    | "pane_up")
            })
            .collect(),
        (ViewMode::Favorites, ActivePane::Search, InputMode::Normal) => all
            .into_iter()
            .filter(|sc| {
                matches!(
                    sc.action_id,
                    "execute"
                        | "navigate_down"
                        | "navigate_up"
                        | "page_down"
                        | "page_up"
                        | "go_to_top"
                        | "go_to_bottom"
                        | "focus_search"
                        | "view_history"
                        | "view_collections"
                        | "change_theme"
                        | "pane_down"
                        | "pane_up"
                )
            })
            .collect(),
        (ViewMode::Favorites, ActivePane::History, InputMode::Normal) => all
            .into_iter()
            .filter(|sc| {
                matches!(sc.action_id, |"execute"| "navigate_down"
                    | "navigate_up"
                    | "page_down"
                    | "page_up"
                    | "go_to_top"
                    | "go_to_bottom"
                    | "toggle_favorite"
                    | "copy_to_clipboard"
                    | "edit_tags"
                    | "add_to_collection"
                    | "toggle_details"
                    | "focus_search"
                    | "switch_pane"
                    | "view_history"
                    | "view_collections"
                    | "change_theme"
                    | "pane_down"
                    | "pane_up")
            })
            .collect(),
        (ViewMode::Collections, ActivePane::Search, InputMode::Normal) => all
            .into_iter()
            .filter(|sc| {
                matches!(
                    sc.action_id,
                    "focus_search"
                        | "view_history"
                        | "view_favorites"
                        | "new_collection"
                        | "pane_down"
                        | "pane_up"
                )
            })
            .collect(),
        (ViewMode::Collections, ActivePane::CollectionsList, InputMode::Normal) => all
            .into_iter()
            .filter(|sc| {
                matches!(
                    sc.action_id,
                    "execute"
                        | "navigate_down"
                        | "navigate_up"
                        | "page_down"
                        | "page_up"
                        | "go_to_top"
                        | "go_to_bottom"
                        | "new_collection"
                        | "edit_collection"
                        | "delete_collection"
                        | "focus_search"
                        | "switch_pane"
                        | "pane_right"
                        | "change_theme"
                        | "view_history"
                        | "view_favorites"
                )
            })
            .collect(),
        (ViewMode::Collections, ActivePane::CollectionItems, InputMode::Normal) => all
            .into_iter()
            .filter(|sc| {
                matches!(sc.action_id, |"execute"| "navigate_down"
                    | "navigate_up"
                    | "page_down"
                    | "page_up"
                    | "go_to_top"
                    | "go_to_bottom"
                    | "copy_to_clipboard"
                    | "toggle_details"
                    | "search_collection"
                    | "remove_from_collection"
                    | "focus_search"
                    | "switch_pane"
                    | "pane_left"
                    | "change_theme"
                    | "view_history"
                    | "view_favorites")
            })
            .collect(),
        _ => all,
    }
}

pub fn handle(state: &mut AppState, key: KeyEvent) -> Action {
    match (key.code, key.modifiers) {
        (KeyCode::Esc, _) => {
            state.help_open = false;
            state.help_search_query.clear();
            return Action::CloseHelp;
        }
        (KeyCode::Up, _) => {
            if state.help_selected_index > 0 {
                state.help_selected_index -= 1;
            }
            state
                .help_list_state
                .select(Some(state.help_selected_index));
            return Action::None;
        }
        (KeyCode::Down, _) => {
            if state.help_filtered_shortcuts.is_empty() {
                return Action::None;
            }
            let max = state.help_filtered_shortcuts.len() - 1;
            if state.help_selected_index < max {
                state.help_selected_index += 1;
            }
            state
                .help_list_state
                .select(Some(state.help_selected_index));
            return Action::None;
        }
        (KeyCode::PageUp, _) | (KeyCode::Char('u'), KeyModifiers::CONTROL) => {
            let page_size = 5usize;
            state.help_selected_index = state.help_selected_index.saturating_sub(page_size);
            state
                .help_list_state
                .select(Some(state.help_selected_index));
            return Action::None;
        }
        (KeyCode::PageDown, _) | (KeyCode::Char('d'), KeyModifiers::CONTROL) => {
            let page_size = 5usize;
            let max = state.help_filtered_shortcuts.len().saturating_sub(1);
            state.help_selected_index = (state.help_selected_index + page_size).min(max);
            state
                .help_list_state
                .select(Some(state.help_selected_index));
            return Action::None;
        }
        (KeyCode::Char('p'), KeyModifiers::CONTROL) => {
            state.help_selected_index = state.help_selected_index.saturating_sub(1);
            state
                .help_list_state
                .select(Some(state.help_selected_index));
            return Action::None;
        }
        (KeyCode::Char('n'), KeyModifiers::CONTROL) => {
            let max = state.help_filtered_shortcuts.len().saturating_sub(1);
            state.help_selected_index = (state.help_selected_index + 1).min(max);
            state
                .help_list_state
                .select(Some(state.help_selected_index));
            return Action::None;
        }
        (KeyCode::Enter, _) => {
            if let Some(sc) = state.help_filtered_shortcuts.get(state.help_selected_index) {
                state.help_open = false;
                return Action::ExecuteHelpShortcut(sc.action_id.to_string());
            }
            return Action::None;
        }
        (KeyCode::Backspace, _) => {
            state.help_search_query.pop();
            state.help_filtered_shortcuts =
                filter_shortcuts(&get_shortcuts_for_context(state), &state.help_search_query);
            state.help_selected_index = 0;
            state.help_list_state.select(Some(0));
            return Action::None;
        }
        (KeyCode::Char(c), KeyModifiers::NONE) => {
            state.help_search_query.push(c);
            state.help_filtered_shortcuts =
                filter_shortcuts(&get_shortcuts_for_context(state), &state.help_search_query);
            state.help_selected_index = 0;
            state.help_list_state.select(Some(0));
            return Action::None;
        }
        _ => {}
    }

    Action::None
}

pub fn execute_help_action(state: &mut AppState, action_id: &str) -> Action {
    match action_id {
        "execute" => {
            let cmd = state.selected_command();
            state.mark_executed();
            return cmd.map(Action::Execute).unwrap_or(Action::None);
        }
        "navigate_down" => {
            state.navigate_down();
            state.list_state.select(Some(state.selected_index));
        }
        "navigate_up" => {
            state.navigate_up();
            state.list_state.select(Some(state.selected_index));
        }
        "page_down" => {
            state.navigate_page_down();
            state.list_state.select(Some(state.selected_index));
        }
        "page_up" => {
            state.navigate_page_up();
            state.list_state.select(Some(state.selected_index));
        }
        "go_to_top" => {
            state.go_to_top();
            state.list_state.select(Some(0));
        }
        "go_to_bottom" => {
            state.go_to_bottom();
            state.list_state.select(Some(state.selected_index));
        }
        "toggle_favorite" => {
            state.toggle_favorite();
        }
        "copy_to_clipboard" => {
            if let Some(text) = state
                .filtered
                .get(state.selected_index)
                .map(|c| c.text.clone())
            {
                let (success, msg) = crate::app::clipboard::copy_to_clipboard(&text);
                if success {
                    state.status_message = Some("📋 Copied to clipboard".into());
                } else if let Some(msg) = msg {
                    state.status_message = Some(msg);
                }
                state.status_timestamp = Some(Instant::now());
            }
        }
        "edit_tags" => {
            state.input_mode = InputMode::TagInput;
            state.tag_input = String::new();
            state.tag_selected_index = 0;
            state.tag_cursor_index = None;
        }
        "add_to_collection" if !state.filtered.is_empty() => {
            state.collection_input_mode = crate::app::CollectionInputMode::AddToCollection;
            state.input_mode = InputMode::CollectionInput;
        }
        "toggle_details" => {
            state.show_details = !state.show_details;
        }
        "focus_search" => {
            state.active_pane = ActivePane::Search;
        }
        "change_theme" => {
            state.open_theme_popup();
        }
        "switch_pane" => {
            state.switch_pane();
        }
        "pane_down" => {
            state.pane_down();
        }
        "pane_up" => {
            state.pane_up();
        }
        "pane_left" => {
            state.pane_left();
        }
        "pane_right" => {
            state.pane_right();
        }
        "view_history" => {
            state.view_mode = ViewMode::History;
            state.active_pane = ActivePane::History;
            state.filter_commands();
        }
        "view_favorites" => {
            state.view_mode = ViewMode::Favorites;
            state.active_pane = ActivePane::History;
            state.filter_commands();
        }
        "view_collections" => {
            state.view_mode = ViewMode::Collections;
            state.active_pane = ActivePane::CollectionsList;
            state.load_collection_commands();
            state.filter_commands();
        }
        "new_collection" => {
            state.collection_input_mode = crate::app::CollectionInputMode::NewCollection;
            state.collection_input_text.clear();
            state.input_mode = InputMode::CollectionInput;
        }
        "edit_collection" => {
            if let Some(col) = state.selected_collection() {
                let col_id = col.id.clone();
                let col_name = col.name.clone();
                state.editing_collection_id = Some(col_id);
                state.collection_input_text = col_name;
                state.collection_input_mode = crate::app::CollectionInputMode::EditCollection;
                state.input_mode = InputMode::CollectionInput;
            }
        }
        "delete_collection" => {
            state.delete_collection();
        }
        "search_collection" => {
            state.collection_input_mode = crate::app::CollectionInputMode::AddToCollectionSearch;
            state.collection_input_text.clear();
            state.input_mode = InputMode::CollectionInput;
            state.add_command_search_index = 0;
        }
        "remove_from_collection" => {
            if let Some(cmd) = state.filtered.get(state.selected_index) {
                let text = cmd.text.clone();
                state.remove_command_from_collection(&text);
            }
        }
        _ => {}
    }

    Action::None
}

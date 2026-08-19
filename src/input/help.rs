use fuzzy_matcher::FuzzyMatcher;
use fuzzy_matcher::skim::SkimMatcherV2;

use crate::app::{Action, AppState};
use crate::keymap::{KeyAction as A, KeyContext, Keymap, format_binding};

/// One row of the help popup.
///
/// `keys` is filled in from the live keymap rather than typed by hand, so a
/// rebound key shows up here without anyone remembering to update a second
/// table. That is also why `drop_ctrl_u_from_page_up` no longer exists: the
/// lookup is per-context, so the search bar simply never lists Ctrl+u under
/// Page Up because Ctrl+u belongs to Clear Search there.
#[derive(Clone, Debug)]
pub struct GroupedShortcut {
    /// `None` for the mouse rows, which document gestures with no binding.
    pub action: Option<A>,
    pub action_name: String,
    pub description: String,
    pub keys: Vec<String>,
    pub category: &'static str,
}

/// What each action is called and where it belongs. Keys deliberately absent —
/// those come from the keymap.
#[rustfmt::skip]
const META: &[(A, &str, &str, &str)] = &[
    (A::NavigateDown, "Navigate Down", "Move selection one item down", "Navigation"),
    (A::NavigateUp, "Navigate Up", "Move selection one item up", "Navigation"),
    (A::PageDown, "Page Down", "Scroll down ~50% of list", "Navigation"),
    (A::PageUp, "Page Up", "Scroll up ~50% of list", "Navigation"),
    (A::GoToTop, "Go to Top", "Jump to first item (press gg)", "Navigation"),
    (A::GoToBottom, "Go to Bottom", "Jump to last item", "Navigation"),
    (A::Execute, "Execute Command", "Runs selected command in terminal", "Actions"),
    (A::ToggleFavorite, "Toggle Favorite", "Mark/unmark as favorite", "Actions"),
    (A::CopyToClipboard, "Copy to Clipboard", "Copy command to clipboard", "Actions"),
    (A::EditTags, "Edit Tags", "Add/remove tags from command", "Actions"),
    (A::AddToCollection, "Add to Collection", "Add command to collection", "Actions"),
    (A::ToggleDetails, "Toggle Details", "Show/hide details panel", "Actions"),
    (A::EditCommand, "Edit Command", "Edit the command before running it (Ctrl+x again for $EDITOR)", "Actions"),
    (A::ChangeTheme, "Change Theme", "Open theme selector popup", "Actions"),
    (A::ExportData, "Export Data", "Open export popup (type path, Enter to export)", "Actions"),
    (A::ImportData, "Import Data", "Open import popup (type path, Enter to preview)", "Actions"),
    (A::FocusSearch, "Focus Search", "Move cursor to search field", "Actions"),
    (A::ClearSearch, "Clear Search", "Clear the search query", "Actions"),
    (A::ShowHelp, "Show Help", "Open this help popup", "Actions"),
    (A::SwitchPane, "Switch Pane", "Cycle through panes", "Panels"),
    (A::PaneDown, "Pane Down", "Focus pane below", "Panels"),
    (A::PaneUp, "Pane Up", "Focus pane above", "Panels"),
    (A::PaneLeft, "Pane Left", "Focus pane on left", "Panels"),
    (A::PaneRight, "Pane Right", "Focus pane on right", "Panels"),
    (A::ViewHistory, "History View", "Show all commands", "Views"),
    (A::ViewFavorites, "Favorites View", "Show favorites only", "Views"),
    (A::ViewCollections, "Collections View", "Show collections", "Views"),
    (A::ScopeCwd, "Scope to Directory", "Show only commands run in the current directory", "Views"),
    (A::NewCollection, "New Collection", "Create new collection", "Collections"),
    (A::EditCollection, "Edit Collection", "Rename collection", "Collections"),
    (A::DeleteCollection, "Delete Collection", "Delete selected collection", "Collections"),
    (A::SearchCollection, "Search to Add", "Search commands to add", "Collections"),
    (A::RemoveFromCollection, "Remove from Collection", "Remove command from collection", "Collections"),
    (A::ShrinkPane, "Narrow Pane", "Narrow the details or collections pane", "Panels"),
    (A::GrowPane, "Widen Pane", "Widen the details or collections pane", "Panels"),
];

/// Gestures, appended to every context. They have no `KeyAction`, so Enter on
/// one is a no-op by construction rather than by a missing match arm.
const MOUSE: &[(&str, &str, &str)] = &[
    ("Select", "Click a row, a tab, or the search bar", "Click"),
    (
        "Run Command",
        "Double-click a command to run it",
        "Double-click",
    ),
    (
        "Context Menu",
        "Right-click a command, or the collections pane",
        "Right-click",
    ),
    (
        "Resize Pane",
        "Drag a pane border; past the minimum hides details",
        "Drag border",
    ),
    (
        "Scroll",
        "Wheel scrolls the list under the pointer",
        "Wheel",
    ),
    (
        "Close Popup",
        "Click outside a popup to close it",
        "Click outside",
    ),
    (
        "Select Text",
        "Hold Shift to select text with the mouse",
        "Shift+drag",
    ),
];

/// Render order. A category split into two runs would draw its header twice.
const CATEGORIES: &[&str] = &[
    "Navigation",
    "Actions",
    "Panels",
    "Views",
    "Collections",
    "Mouse",
];

/// Where config problems are listed. The startup status message names the first
/// one and then sits there until the next keypress; this is the copy that stays
/// put and that a user can actually read through.
pub const CONFIG_CATEGORY: &str = "Config";

/// Every action bound in `context`, plus the ones it inherits from `Global`.
///
/// This replaces six hand-maintained allow-lists of action ids. Those were
/// filters over a separate master list, so an action missing from all of them
/// was invisible in the help popup no matter how well it was described.
pub fn shortcuts_for(
    keymap: &Keymap,
    context: KeyContext,
    problems: &[crate::config::ConfigProblem],
) -> Vec<GroupedShortcut> {
    let mut contexts = vec![context];
    if context.falls_through_to_global() && context != KeyContext::Global {
        contexts.push(KeyContext::Global);
    }

    // First, not last: the popup opens at the top and scrolls, so a problem
    // appended after forty shortcuts is one nobody ever sees. The startup
    // status message points here, and that pointer has to be honest.
    let mut out: Vec<GroupedShortcut> = problems
        .iter()
        .map(|problem| GroupedShortcut {
            action: None,
            action_name: "Not applied".to_owned(),
            description: problem.to_string(),
            keys: vec!["config.toml".to_owned()],
            category: CONFIG_CATEGORY,
        })
        .collect();
    for &category in CATEGORIES {
        if category == "Mouse" {
            out.extend(MOUSE.iter().map(|(name, desc, key)| GroupedShortcut {
                action: None,
                action_name: (*name).to_owned(),
                description: (*desc).to_owned(),
                keys: vec![(*key).to_owned()],
                category: "Mouse",
            }));
            continue;
        }
        for (action, name, description, cat) in META {
            if *cat != category {
                continue;
            }
            // The nearer context wins, so an action bound in both is listed
            // once, with the keys that actually reach it here.
            let Some((owner, mut keys)) = contexts
                .iter()
                .map(|&c| (c, keymap.keys_for(c, *action)))
                .find(|(_, k)| !k.is_empty())
            else {
                continue;
            };
            // Shadowing: a key the nearer context has already claimed for
            // something else never reaches this action, so advertising it would
            // be a lie. This is why Page Up drops Ctrl+u in the search bar,
            // where it clears the query — a case that used to need its own
            // special-cased `drop_ctrl_u_from_page_up`.
            if owner != context {
                let claimed = keymap.entries(context);
                keys.retain(|key| {
                    !claimed
                        .iter()
                        .any(|(b, a)| a != action && format_binding(b) == *key)
                });
            }
            if keys.is_empty() {
                continue;
            }
            out.push(GroupedShortcut {
                action: Some(*action),
                action_name: (*name).to_owned(),
                description: (*description).to_owned(),
                keys,
                category,
            });
        }
    }
    out
}

/// The shortcuts for whatever pane is behind the help popup — not the popup's
/// own context, which is what `input::active_context` would report while it is
/// open.
pub fn get_shortcuts_for_context(state: &AppState) -> Vec<GroupedShortcut> {
    shortcuts_for(
        &state.keymap,
        crate::input::pane_context(state),
        &state.config_problems,
    )
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
                .fuzzy_indices(&sc.action_name, &query_lower)
                .map(|(s, _)| s);
            let desc_score = matcher
                .fuzzy_indices(&sc.description, &query_lower)
                .map(|(s, _)| s / 2);
            let keys_match = sc
                .keys
                .iter()
                .any(|k| k.to_lowercase().contains(&query_lower));
            let key_score: i64 = if keys_match { 1000 } else { 0 };

            let total_score = name_score.unwrap_or(0).max(desc_score.unwrap_or(0)) + key_score;

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

const PAGE: usize = 5;

fn select(state: &mut AppState, index: usize) {
    state.help_selected_index = index;
    state.help_list_state.select(Some(index));
}

fn max_index(state: &AppState) -> usize {
    state.help_filtered_shortcuts.len().saturating_sub(1)
}

fn refilter(state: &mut AppState) {
    state.help_filtered_shortcuts =
        filter_shortcuts(&get_shortcuts_for_context(state), &state.help_search_query);
    select(state, 0);
}

pub fn insert_char(state: &mut AppState, c: char) {
    state.help_search_query.push(c);
    refilter(state);
}

pub fn dispatch(state: &mut AppState, action: A) -> Action {
    match action {
        A::NavigateUp => select(state, state.help_selected_index.saturating_sub(1)),
        A::NavigateDown => select(state, (state.help_selected_index + 1).min(max_index(state))),
        A::PageUp => select(state, state.help_selected_index.saturating_sub(PAGE)),
        A::PageDown => select(
            state,
            (state.help_selected_index + PAGE).min(max_index(state)),
        ),
        A::DeleteCharBackward => {
            state.help_search_query.pop();
            refilter(state);
        }
        A::Confirm => {
            // Runs the chosen action in the pane behind the popup, through the
            // same dispatcher a key press uses — so the help popup can never
            // implement an action differently from its keybinding, which is
            // exactly what had already happened before this existed.
            let picked = state
                .help_filtered_shortcuts
                .get(state.help_selected_index)
                .and_then(|sc| sc.action);
            if let Some(picked) = picked {
                state.help_open = false;
                state.help_search_query.clear();
                return crate::input::dispatch_in_pane(state, picked);
            }
        }
        _ => {}
    }
    Action::None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ConfigProblem;
    use crate::keymap::defaults;

    #[test]
    fn test_config_problems_are_listed_first() {
        let problems = vec![ConfigProblem {
            entry: "[keys.history] launch_rocket".into(),
            message: "unknown action".into(),
        }];
        let rows = shortcuts_for(&defaults::keymap(), KeyContext::History, &problems);
        assert_eq!(rows[0].category, CONFIG_CATEGORY);
        assert!(rows[0].description.contains("unknown action"));
        assert!(rows.len() > 1, "the real shortcuts follow");
    }

    #[test]
    fn test_no_config_section_when_the_file_is_clean() {
        let rows = shortcuts_for(&defaults::keymap(), KeyContext::History, &[]);
        assert!(rows.iter().all(|r| r.category != CONFIG_CATEGORY));
    }

    /// The popup opens a new header each time the category changes as it walks
    /// the list, so a category split into two runs renders its heading twice.
    #[test]
    fn test_categories_are_contiguous() {
        for context in [KeyContext::History, KeyContext::CollectionItems] {
            let rows = shortcuts_for(&defaults::keymap(), context, &[]);
            let mut seen: Vec<&str> = Vec::new();
            let mut previous: Option<&str> = None;
            for row in &rows {
                if previous != Some(row.category) {
                    assert!(
                        !seen.contains(&row.category),
                        "{} appears in two separate runs",
                        row.category
                    );
                    seen.push(row.category);
                    previous = Some(row.category);
                }
            }
        }
    }

    /// Mouse rows document gestures and carry no action, so Enter on one is a
    /// no-op by construction rather than by a missing match arm.
    #[test]
    fn test_mouse_rows_carry_no_action() {
        let rows = shortcuts_for(&defaults::keymap(), KeyContext::History, &[]);
        let mouse: Vec<_> = rows.iter().filter(|r| r.category == "Mouse").collect();
        assert_eq!(mouse.len(), MOUSE.len());
        assert!(mouse.iter().all(|r| r.action.is_none()));
    }

    /// Every listed shortcut shows the keys that actually reach it here.
    #[test]
    fn test_listed_keys_come_from_the_keymap() {
        let mut keymap = defaults::keymap();
        keymap.clear_action(KeyContext::History, A::ToggleFavorite);
        keymap.bind(
            KeyContext::History,
            crate::keymap::parse_binding("v").unwrap(),
            A::ToggleFavorite,
        );
        let rows = shortcuts_for(&keymap, KeyContext::History, &[]);
        let favourite = rows
            .iter()
            .find(|r| r.action == Some(A::ToggleFavorite))
            .expect("toggle_favorite is listed");
        assert_eq!(favourite.keys, vec!["v"]);
    }

    /// In the search bar Ctrl+u clears the query, so Page Up must not advertise
    /// it there. This used to need a special case (`drop_ctrl_u_from_page_up`);
    /// the per-context lookup makes it fall out.
    #[test]
    fn test_page_up_does_not_advertise_ctrl_u_in_the_search_bar() {
        let rows = shortcuts_for(&defaults::keymap(), KeyContext::Search, &[]);
        let page_up = rows.iter().find(|r| r.action == Some(A::PageUp));
        if let Some(page_up) = page_up {
            assert!(
                !page_up.keys.iter().any(|k| k == "Ctrl+u"),
                "Ctrl+u belongs to Clear Search here, not Page Up"
            );
        }
        let history = shortcuts_for(&defaults::keymap(), KeyContext::History, &[]);
        let page_up = history
            .iter()
            .find(|r| r.action == Some(A::PageUp))
            .expect("page_up is listed outside the search bar");
        assert!(page_up.keys.iter().any(|k| k == "Ctrl+u"));
    }
}

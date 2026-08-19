//! Every thing a key can do, as one enum.
//!
//! Before this existed the same action was implemented twice — once as a match
//! arm in a handler, once as a string arm in `help::execute_help_action` — and
//! the two had already drifted. A single enum makes the compiler refuse a new
//! action until every dispatcher has considered it, and `as_str`/`from_str`
//! give the config file and the help popup a stable name to key off.

macro_rules! key_actions {
    ($($variant:ident => $name:literal),* $(,)?) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub enum KeyAction {
            $($variant),*
        }

        impl KeyAction {
            pub const ALL: &'static [KeyAction] = &[$(KeyAction::$variant),*];

            /// The name used in `config.toml` and by the help popup.
            pub const fn as_str(self) -> &'static str {
                match self {
                    $(KeyAction::$variant => $name),*
                }
            }

            pub fn from_str(s: &str) -> Option<Self> {
                match s {
                    $($name => Some(KeyAction::$variant),)*
                    _ => None,
                }
            }
        }
    };
}

key_actions! {
    // Movement
    NavigateUp => "navigate_up",
    NavigateDown => "navigate_down",
    PageUp => "page_up",
    PageDown => "page_down",
    GoToTop => "go_to_top",
    GoToBottom => "go_to_bottom",

    // The selection
    Execute => "execute",
    EditCommand => "edit_command",
    ToggleFavorite => "toggle_favorite",
    CopyToClipboard => "copy_to_clipboard",
    EditTags => "edit_tags",
    AddToCollection => "add_to_collection",
    ToggleDetails => "toggle_details",

    // Search and panes
    FocusSearch => "focus_search",
    ClearSearch => "clear_search",
    SwitchPane => "switch_pane",
    PaneUp => "pane_up",
    PaneDown => "pane_down",
    PaneLeft => "pane_left",
    PaneRight => "pane_right",
    ShrinkPane => "shrink_pane",
    GrowPane => "grow_pane",

    // Views
    ViewHistory => "view_history",
    ViewFavorites => "view_favorites",
    ViewCollections => "view_collections",
    ScopeCwd => "scope_cwd",

    // Collections
    NewCollection => "new_collection",
    EditCollection => "edit_collection",
    DeleteCollection => "delete_collection",
    SearchCollection => "search_collection",
    RemoveFromCollection => "remove_from_collection",

    // Overlays
    ShowHelp => "show_help",
    EditKeybindings => "edit_keybindings",
    ResetKeybindings => "reset_keybindings",
    ChangeTheme => "change_theme",
    ExportData => "export_data",
    ImportData => "import_data",

    // Line editing
    CursorLeft => "cursor_left",
    CursorRight => "cursor_right",
    CursorHome => "cursor_home",
    CursorEnd => "cursor_end",
    DeleteCharBackward => "delete_char_backward",
    DeleteCharForward => "delete_char_forward",
    KillLine => "kill_line",
    AcceptSuggestion => "accept_suggestion",
    OpenExternalEditor => "open_external_editor",

    // Universal
    Confirm => "confirm",
    Cancel => "cancel",
    Decline => "decline",
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_action_names_round_trip() {
        for &action in KeyAction::ALL {
            assert_eq!(
                KeyAction::from_str(action.as_str()),
                Some(action),
                "{:?} does not round-trip",
                action
            );
        }
    }

    #[test]
    fn test_key_action_names_are_unique() {
        let mut names: Vec<&str> = KeyAction::ALL.iter().map(|a| a.as_str()).collect();
        let total = names.len();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), total, "two actions share a name");
    }

    #[test]
    fn test_key_action_names_are_snake_case() {
        for &action in KeyAction::ALL {
            let name = action.as_str();
            assert!(
                name.chars().all(|c| c.is_ascii_lowercase() || c == '_'),
                "{} is not snake_case",
                name
            );
        }
    }
}

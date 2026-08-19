//! The built-in keymap. `config.toml` is applied on top of this, so anything
//! the user does not mention keeps the binding here.

use super::{Binding, KeyAction as A, KeyContext as C, Keymap, parse_binding};

/// Panics on a malformed key string, which can only be a typo in the table
/// below — a test walks every entry.
fn binding(s: &str) -> Binding {
    parse_binding(s).unwrap_or_else(|e| panic!("bad default binding `{}`: {}", s, e))
}

pub fn keymap() -> Keymap {
    let mut km = Keymap::default();
    for (context, keys, action) in TABLE {
        for key in keys.split(", ") {
            km.bind(*context, binding(key), *action);
        }
    }
    km
}

/// `(context, comma-separated keys, action)`. The order within a context is the
/// order the help popup and the footer advertise them in, so the primary key
/// comes first.
#[rustfmt::skip]
pub const TABLE: &[(C, &str, A)] = &[
    // ── Global ──────────────────────────────────────────────────────────────
    // Reached from any pane, but never from an overlay. A plain character here
    // still types when the search bar has focus: `Search` absorbs text before
    // the chain reaches this table.
    (C::Global, "Enter",            A::Execute),
    (C::Global, "Up",               A::NavigateUp),
    (C::Global, "Down",             A::NavigateDown),
    (C::Global, "PageDown, ctrl+d", A::PageDown),
    (C::Global, "PageUp, ctrl+u",   A::PageUp),
    (C::Global, "g g",              A::GoToTop),
    (C::Global, "G",                A::GoToBottom),
    (C::Global, "Tab",              A::SwitchPane),
    (C::Global, "ctrl+j",           A::PaneDown),
    (C::Global, "ctrl+k",           A::PaneUp),
    (C::Global, "ctrl+h",           A::PaneLeft),
    (C::Global, "ctrl+l",           A::PaneRight),
    (C::Global, "1, alt+1",         A::ViewHistory),
    (C::Global, "2, alt+2",         A::ViewFavorites),
    (C::Global, "3, alt+3",         A::ViewCollections),
    (C::Global, "., alt+.",         A::ScopeCwd),
    (C::Global, "<, alt+<",         A::ShrinkPane),
    (C::Global, ">, alt+>",         A::GrowPane),
    (C::Global, "?, F1",            A::ShowHelp),
    (C::Global, "ctrl+t",           A::ChangeTheme),
    (C::Global, "ctrl+e",           A::ExportData),
    (C::Global, "ctrl+o",           A::ImportData),
    (C::Global, "ctrl+x",           A::OpenExternalEditor),
    (C::Global, "c",                A::AddToCollection),
    (C::Global, "esc, ctrl+c",      A::Cancel),

    // ── Panes ───────────────────────────────────────────────────────────────
    (C::Search, "ctrl+u",    A::ClearSearch),
    (C::Search, "Backspace", A::DeleteCharBackward),

    (C::History, "/",         A::FocusSearch),
    (C::History, "j",         A::NavigateDown),
    (C::History, "k",         A::NavigateUp),
    (C::History, "f",         A::ToggleFavorite),
    (C::History, "y",         A::CopyToClipboard),
    (C::History, "t",         A::EditTags),
    (C::History, "d",         A::ToggleDetails),
    (C::History, "e",         A::EditCommand),
    (C::History, "Backspace", A::DeleteCharBackward),

    (C::CollectionsList, "/",         A::FocusSearch),
    (C::CollectionsList, "j",         A::NavigateDown),
    (C::CollectionsList, "k",         A::NavigateUp),
    (C::CollectionsList, "n",         A::NewCollection),
    (C::CollectionsList, "e",         A::EditCollection),
    (C::CollectionsList, "d",         A::DeleteCollection),
    (C::CollectionsList, "Backspace", A::DeleteCharBackward),

    (C::CollectionItems, "/",         A::FocusSearch),
    (C::CollectionItems, "j",         A::NavigateDown),
    (C::CollectionItems, "k",         A::NavigateUp),
    (C::CollectionItems, "y",         A::CopyToClipboard),
    (C::CollectionItems, "d",         A::ToggleDetails),
    (C::CollectionItems, "a",         A::SearchCollection),
    (C::CollectionItems, "r",         A::RemoveFromCollection),
    (C::CollectionItems, "e",         A::EditCommand),
    (C::CollectionItems, "Backspace", A::DeleteCharBackward),

    // ── Overlays ────────────────────────────────────────────────────────────
    // Exclusive: no fall-through to Global, so each needs its own cancel key.
    (C::Help, "Enter",           A::Confirm),
    (C::Help, "Up, ctrl+p",      A::NavigateUp),
    (C::Help, "Down, ctrl+n",    A::NavigateDown),
    (C::Help, "PageUp, ctrl+u",  A::PageUp),
    (C::Help, "PageDown, ctrl+d",A::PageDown),
    (C::Help, "Backspace",       A::DeleteCharBackward),
    (C::Help, "esc, ctrl+c",     A::Cancel),

    (C::TagInput, "Enter",        A::Confirm),
    (C::TagInput, "Tab",          A::AcceptSuggestion),
    (C::TagInput, "Left",         A::CursorLeft),
    (C::TagInput, "Right",        A::CursorRight),
    (C::TagInput, "Up, ctrl+p",   A::NavigateUp),
    (C::TagInput, "Down, ctrl+n", A::NavigateDown),
    (C::TagInput, "Backspace",    A::DeleteCharBackward),
    (C::TagInput, "ctrl+u",       A::KillLine),
    (C::TagInput, "esc, ctrl+c",  A::Cancel),

    (C::CollectionInput, "Enter",        A::Confirm),
    (C::CollectionInput, "Up, ctrl+p",   A::NavigateUp),
    (C::CollectionInput, "Down, ctrl+n", A::NavigateDown),
    (C::CollectionInput, "Backspace",    A::DeleteCharBackward),
    (C::CollectionInput, "ctrl+u",       A::KillLine),
    (C::CollectionInput, "esc, ctrl+c",  A::Cancel),

    (C::ImportExport, "Enter",        A::Confirm),
    (C::ImportExport, "Up, ctrl+p",   A::NavigateUp),
    (C::ImportExport, "Down, ctrl+n", A::NavigateDown),
    (C::ImportExport, "Backspace",    A::DeleteCharBackward),
    (C::ImportExport, "ctrl+u",       A::KillLine),
    (C::ImportExport, "esc, ctrl+c",  A::Cancel),

    (C::ThemePopup, "Enter",       A::Confirm),
    (C::ThemePopup, "j, Down",     A::NavigateDown),
    (C::ThemePopup, "k, Up",       A::NavigateUp),
    (C::ThemePopup, "esc, ctrl+c", A::Cancel),

    (C::ContextMenu, "Enter",          A::Confirm),
    (C::ContextMenu, "j, Down",        A::NavigateDown),
    (C::ContextMenu, "k, Up",          A::NavigateUp),
    (C::ContextMenu, "esc, ctrl+c, q", A::Cancel),

    (C::IntegrationPopup, "Enter, u, y",    A::Confirm),
    (C::IntegrationPopup, "esc, ctrl+c, n, q", A::Decline),

    (C::EditCommand, "Enter",       A::Confirm),
    (C::EditCommand, "Left",        A::CursorLeft),
    (C::EditCommand, "Right",       A::CursorRight),
    (C::EditCommand, "Home",        A::CursorHome),
    (C::EditCommand, "End",         A::CursorEnd),
    (C::EditCommand, "Backspace",   A::DeleteCharBackward),
    (C::EditCommand, "Delete",      A::DeleteCharForward),
    (C::EditCommand, "ctrl+u",      A::KillLine),
    (C::EditCommand, "ctrl+x",      A::OpenExternalEditor),
    (C::EditCommand, "esc, ctrl+c", A::Cancel),
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keymap::keys::Binding;

    #[test]
    fn test_every_default_key_string_parses() {
        for (_, keys, _) in TABLE {
            for key in keys.split(", ") {
                assert!(
                    super::super::parse_binding(key).is_ok(),
                    "`{}` does not parse",
                    key
                );
            }
        }
    }

    /// Two actions on one key in one context means the second is unreachable.
    #[test]
    fn test_no_key_is_bound_twice_within_a_context() {
        let km = keymap();
        for &context in C::ALL {
            let mut seen: Vec<&Binding> = vec![];
            for (binding, action) in km.entries(context) {
                assert!(
                    !seen.contains(&binding),
                    "{:?} binds {} twice (second is {:?})",
                    context,
                    super::super::format_binding(binding),
                    action
                );
                seen.push(binding);
            }
        }
    }

    /// A single binding on `X` fires before the chord machine can ever collect
    /// `X Y`, so the chord would be dead.
    #[test]
    fn test_no_single_binding_shadows_a_chord_prefix() {
        let km = keymap();
        for &context in C::ALL {
            let entries = km.entries(context);
            for (binding, _) in entries {
                let Binding::Chord(first, _) = binding else {
                    continue;
                };
                for (other, action) in entries {
                    if let Binding::Single(single) = other {
                        assert_ne!(
                            single, first,
                            "{:?}: single {:?} shadows a chord starting with the same key",
                            context, action
                        );
                    }
                }
            }
        }
    }

    /// An action nothing is bound to cannot be reached by keyboard.
    #[test]
    fn test_every_action_is_bound_somewhere() {
        let km = keymap();
        for &action in A::ALL {
            let bound = C::ALL
                .iter()
                .any(|&c| km.entries(c).iter().any(|(_, a)| *a == action));
            assert!(bound, "{:?} is not bound in any context", action);
        }
    }

    /// Overlays do not fall through to Global, so each has to carry its own way
    /// out or it would trap the user.
    #[test]
    fn test_every_exclusive_context_can_be_left() {
        let km = keymap();
        for &context in C::ALL {
            if context == C::Global || context.falls_through_to_global() {
                continue;
            }
            let has_exit = km
                .entries(context)
                .iter()
                .any(|(_, a)| matches!(a, A::Cancel | A::Decline));
            assert!(has_exit, "{:?} has no cancel key", context);
        }
    }
}

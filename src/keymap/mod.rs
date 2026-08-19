//! The keymap: which key does what, in which context.
//!
//! Before this, every binding was a literal `match (key.code, key.modifiers)`
//! arm spread over six files, and the help popup advertised a second, hand-typed
//! copy that nothing kept in sync. Here the table is the only copy: dispatch
//! reads it forwards, the help popup and the footer read it backwards.

pub mod action;
pub mod defaults;
pub mod keys;

use std::collections::HashMap;

pub use action::KeyAction;
pub use keys::{Binding, KeyChord, format_binding, parse_binding};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// Where a key press lands. Finer-grained than `InputMode` because the panes
/// disagree about what the same letter means — `d` hides the details pane in
/// History and deletes a collection in the collections list.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum KeyContext {
    /// Reached from any pane context, never from an overlay.
    Global,
    Search,
    History,
    CollectionsList,
    CollectionItems,
    Help,
    TagInput,
    CollectionInput,
    ImportExport,
    ThemePopup,
    ContextMenu,
    IntegrationPopup,
    EditCommand,
}

impl KeyContext {
    /// Contexts with a text field swallow unmodified printable characters on a
    /// lookup miss, before anything falls through to [`KeyContext::Global`].
    ///
    /// This one rule replaces every `if state.active_pane != ActivePane::Search`
    /// guard the old handlers carried: those guards existed so that `1`, `?`,
    /// `<`, `.`, `c` and `g` would type into the search bar instead of firing
    /// their global action. Stating it once beats repeating it per binding.
    pub fn absorbs_text(self) -> bool {
        matches!(
            self,
            KeyContext::Search
                | KeyContext::Help
                | KeyContext::TagInput
                | KeyContext::CollectionInput
                | KeyContext::ImportExport
                | KeyContext::EditCommand
        )
    }

    /// Overlays are exclusive: they take the key or drop it, and never fall
    /// through to `Global`. Otherwise Ctrl+T inside the help popup would open
    /// the theme picker behind it.
    pub fn falls_through_to_global(self) -> bool {
        matches!(
            self,
            KeyContext::Search
                | KeyContext::History
                | KeyContext::CollectionsList
                | KeyContext::CollectionItems
        )
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            KeyContext::Global => "global",
            KeyContext::Search => "search",
            KeyContext::History => "history",
            KeyContext::CollectionsList => "collections_list",
            KeyContext::CollectionItems => "collection_items",
            KeyContext::Help => "help",
            KeyContext::TagInput => "tag_input",
            KeyContext::CollectionInput => "collection_input",
            KeyContext::ImportExport => "import_export",
            KeyContext::ThemePopup => "theme_popup",
            KeyContext::ContextMenu => "context_menu",
            KeyContext::IntegrationPopup => "integration_popup",
            KeyContext::EditCommand => "edit_command",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|c| c.as_str() == s)
    }

    pub const ALL: &'static [KeyContext] = &[
        KeyContext::Global,
        KeyContext::Search,
        KeyContext::History,
        KeyContext::CollectionsList,
        KeyContext::CollectionItems,
        KeyContext::Help,
        KeyContext::TagInput,
        KeyContext::CollectionInput,
        KeyContext::ImportExport,
        KeyContext::ThemePopup,
        KeyContext::ContextMenu,
        KeyContext::IntegrationPopup,
        KeyContext::EditCommand,
    ];
}

/// What a key press resolved to.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Resolved {
    /// Run this action in this context.
    Action(KeyContext, KeyAction),
    /// The first half of a chord; remember it and wait for the second.
    Pending,
    /// An unbound printable character in a context that takes text.
    Text(KeyContext, char),
    /// Nothing is bound to it.
    Unbound,
}

#[derive(Clone, Debug, Default)]
pub struct Keymap {
    bindings: HashMap<KeyContext, Vec<(Binding, KeyAction)>>,
}

impl Keymap {
    pub fn bind(&mut self, context: KeyContext, binding: Binding, action: KeyAction) {
        self.bindings
            .entry(context)
            .or_default()
            .push((binding, action));
    }

    /// Drops every binding for `action` in `context`. A config entry replaces
    /// the defaults rather than adding to them — otherwise nobody could get rid
    /// of a default key they dislike.
    pub fn clear_action(&mut self, context: KeyContext, action: KeyAction) {
        if let Some(list) = self.bindings.get_mut(&context) {
            list.retain(|(_, a)| *a != action);
        }
    }

    /// Drops whatever currently owns `binding` in `context`, so a rebind onto an
    /// occupied key wins instead of being shadowed by the default.
    pub fn evict(&mut self, context: KeyContext, binding: &Binding) -> Option<KeyAction> {
        let list = self.bindings.get_mut(&context)?;
        let idx = list.iter().position(|(b, _)| b == binding)?;
        Some(list.remove(idx).1)
    }

    pub fn entries(&self, context: KeyContext) -> &[(Binding, KeyAction)] {
        self.bindings
            .get(&context)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    /// Keys to advertise for `action`, as display strings.
    pub fn keys_for(&self, context: KeyContext, action: KeyAction) -> Vec<String> {
        self.entries(context)
            .iter()
            .filter(|(_, a)| *a == action)
            .map(|(b, _)| format_binding(b))
            .collect()
    }

    fn lookup_single(&self, context: KeyContext, key: &KeyEvent) -> Option<KeyAction> {
        self.entries(context).iter().find_map(|(b, a)| match b {
            Binding::Single(c) if c.matches(key) => Some(*a),
            _ => None,
        })
    }

    fn lookup_chord(
        &self,
        context: KeyContext,
        pending: &KeyChord,
        key: &KeyEvent,
    ) -> Option<KeyAction> {
        self.entries(context).iter().find_map(|(b, a)| match b {
            Binding::Chord(first, second) if first == pending && second.matches(key) => Some(*a),
            _ => None,
        })
    }

    fn is_chord_prefix(&self, context: KeyContext, key: &KeyEvent) -> bool {
        self.entries(context)
            .iter()
            .any(|(b, _)| matches!(b, Binding::Chord(first, _) if first.matches(key)))
    }

    /// The chain: the specific context first, then `Global`, then the absorb
    /// rule. Inverting the old order — global first, rescued by per-arm pane
    /// guards — is what lets those guards disappear.
    pub fn resolve(
        &self,
        context: KeyContext,
        pending: Option<KeyChord>,
        key: &KeyEvent,
    ) -> Resolved {
        let both = [context, KeyContext::Global];
        let chain = if context.falls_through_to_global() {
            &both[..]
        } else {
            &both[..1]
        };

        for &ctx in chain {
            if let Some(pending) = pending
                && let Some(action) = self.lookup_chord(ctx, &pending, key)
            {
                return Resolved::Action(ctx, action);
            }
            if self.is_chord_prefix(ctx, key) {
                return Resolved::Pending;
            }
            if let Some(action) = self.lookup_single(ctx, key) {
                return Resolved::Action(ctx, action);
            }
            // Absorbing happens here, between the pane and `Global`, not after
            // both. Otherwise `1` in the search bar would switch views instead
            // of typing, which is what the old per-arm
            // `active_pane != ActivePane::Search` guards were there to prevent.
            if ctx == context
                && context.absorbs_text()
                && let KeyCode::Char(c) = key.code
                && (key.modifiers - KeyModifiers::SHIFT).is_empty()
            {
                return Resolved::Text(context, c);
            }
        }
        Resolved::Unbound
    }
}

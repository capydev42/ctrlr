# Changelog

All notable changes to ctrlr will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [Unreleased]

### Added
- Catppuccin theme support with 4 flavors (Latte, Frappe, Macchiato, Mocha)
  - Theme selector popup with color swatches and live preview (`Ctrl+T`)
  - j/k or ↑/↓ navigation with instant theme switching
  - Esc to cancel and revert to previous theme
  - Current theme name displayed in footer (e.g. `Ctrl+T: Theme (Mocha)`)
  - `Ctrl+t` shortcut added to help popup
- Theme persistence in SQLite `settings` table
  - Selected theme is saved and restored on startup

### Changed
- Each theme now has distinct selection highlight colors (Mocha: mauve, Macchiato: sky, Frappe: teal, Latte: blue)
- Unfocus border colors use theme-specific subtle colors instead of generic `DarkGray`
  - Esc to cancel and revert to previous theme
  - Current theme name displayed in footer (e.g. `Ctrl+T: Theme (Mocha)`)

---

## [0.4.0] - 2026-04-28

### Added
- `y` shortcut to copy command to clipboard (shown in help popup)

### Changed
- Panel borders with focus indicators
  - Focused: purple border + [PanelName] title
  - Unfocused: dark gray border + PanelName title
  - Affected: Search, Commands, Details, Collections, Collection Items
- Tab display with counter showing item counts per view
  - Format: "1 History (124)" with spacing between tabs
  - Active tab: lilac color + bold + underlined
  - Inactive tab: muted gray
- Tag display with chip style `[tag]` and right-alignment
  - Max 3 tags visible, overflow shown as "+N more"
  - Consistent colors (dark gray bg, light gray fg)
  - Command text truncated when needed to make room for tags
- Favorites use `*` instead of ⭐ emoji for consistency

---

## [0.3.0] - 2026-04-23

### Added
- Unit tests for shell history parsers (bash, zsh, fish)
- Unit tests for storage modules (commands, tags, collections)
- Copy to clipboard with `y` key (History, Favorites, CollectionItems)
  - Cross-platform support: xclip → wl-copy → arboard fallback
  - Toast notification "📋 Copied to clipboard"
  - Error message if clipboard tools unavailable

### Changed
- Backspace in History/Favorites/Collections switches to Search and removes character

### Fixed
- History now displays newest commands at top (chronological order)
- Executed commands move to top of list for better UX
- use_count populated from shell history instead of starting at 0

---

## [0.2.0] - 2026-04-20

### Added
- Help panel with fuzzy search (press `?` to open)
  - Context-aware shortcuts filtered by current view/pane
  - Shortcut descriptions explain what each key does
  - Execute shortcuts directly with Enter key
  - Version displayed in title bar
  - Key formatting as chips [Key] for better UX
  - Category grouping (Navigation, Actions, Views, Panels, Collections)
  - `? Help` shortcut in main footer hints

### Fixed
- Search field character input in help popup
- Category selection highlighting
- Favorite marker border alignment (use ASCII instead of emoji)
- Search bar duplicate "Search" label

---

## [0.1.4] - 2026-04-18

### Added
- Vim-style panel navigation with Ctrl+j/k/h/l (in addition to Tab)
  - Ctrl+j: Move down to panel (Search → History/Collections)
  - Ctrl+k: Move up to panel (History/Collections → Search)
  - Ctrl+h/l: Navigate left/right between panels in Collections view
- Fuzzy search popup to add commands to collections (press `a` in collection items view)
- Delete confirmation popup for collections and commands (Enter: Delete, Esc: Cancel)
- Page up/down navigation with Ctrl+D/U or PageUp/PageDown keys
- Go to top/bottom navigation with gg/G (vim-style, 500ms timeout)
- Ctrl+N/Ctrl+P for popup navigation (vim-style suggestions)

### Changed
- Search results now include all commands from history, not just non-contained ones

### Fixed
- Minimum height in tag and command popups ensures create option is always visible

---

## [0.1.3] - 2026-04-14

### Added
- `Action` enum for structured return values from input handlers (`None`, `Exit`, `Execute(String)`)
- `app/` module containing application state and action types
- `input/` module with separated input handlers (`normal.rs`, `tag.rs`, `collection.rs`)
- `ui/` module with dedicated UI components
- `storage::hydrate_commands()` for single-entry DB command enrichment
- `AppState::bootstrap()` for simplified app initialization

### Changed
- `ListState` management moved from `main.rs` to `AppState` for centralized UI state
- Input handling refactored into dedicated modules by `InputMode`
- Simplified event loop with `match action { ... }` pattern instead of `Option<String>`
- `highlight_text()` function extracted for reusable fuzzy match highlighting
- Bootstrap logic encapsulated in `AppState::bootstrap()` - `main.rs` no longer knows about DB details

### Refactored
- Extracted `handle_key` into `input/` submodules (`tag::handle`, `collection::handle`, `normal::handle`)
- Moved `state.rs` to `app/state.rs` with `app/` module as central location
- All 5 `ListState` instances now live in `AppState` (avoids split-brain state issues)
- Render functions extracted into `ui/` module (`layout.rs`, `components.rs`, `history.rs`, `collections.rs`, `popups.rs`)
- DB enrichment consolidated into `storage::hydrate_commands()`
- `main.rs` reduced from 921 to 90 lines

---

## [0.1.2] - 2026-04-10

### Added
- Demo GIF in README showcasing ctrlr in action

### Fixed
- macOS zsh widget compatibility with crossterm's `use-dev-tty` feature
- TTY check before terminal initialization to prevent cryptic errors

### Documentation
- Improved README structure and content
- Added curl-based installation script

---

## [0.1.1] - 2026-04-08

### Added
- Collections feature for curated command lists
- Detail panel with command info and metadata
- Global `c` shortcut to add commands to collections
- Checkmark indicator for collections containing selected command
- Fuzzy search support in collections view
- Search shortcut in collection panes

### Fixed
- Type-to-filter search in AddToCollection popup
- State updates when adding/removing commands from collections
- Selection preservation after adding to collection

### Refactored
- Extracted AppState and state logic to separate `state.rs` module
- Simplified nested ifs in popup rendering

### Style
- Bold styling for create items in popups
- Fixed clippy warnings

---

## [0.1.0] - 2026-04-02

### Added
- Initial release
- Shell history management (bash, zsh, fish)
- Fuzzy search through command history
- Favorites system for frequently used commands
- Tags for organizing commands
- Ctrl+R integration with shell keybindings
- Interactive TUI powered by ratatui
- SQLite-based local storage
- View modes: History and Favorites
- Keyboard-first workflow with vim-style navigation
- Installation via curl script
- Multi-platform releases (Linux, macOS)
- MIT License

### Keybindings
- Tab: Switch pane
- Enter: Select/execute command
- Esc: Exit/cancel
- 1/2/3: Switch views
- j/k: Vim-style navigation
- t: Tag editing mode
- f: Toggle favorite
- /: Jump to search
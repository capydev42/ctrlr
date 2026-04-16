# Changelog

All notable changes to ctrlr will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [Unreleased]

### Added
- Fuzzy search popup to add commands to collections (press `a` in collection items view)
- Delete confirmation popup for collections and commands (Enter: Delete, Esc: Cancel)
- Page up/down navigation with Ctrl+D/U or PageUp/PageDown keys
- Go to top/bottom navigation with gg/G (vim-style, 500ms timeout)

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

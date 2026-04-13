# Changelog

All notable changes to ctrlr will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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

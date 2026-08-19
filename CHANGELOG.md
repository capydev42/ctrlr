# Changelog

All notable changes to ctrlr will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [Unreleased]

### Added
- Keybindings are yours. `ctrlr config --print > ~/.config/ctrlr/config.toml` writes out every default, and editing it rebinds any key in any pane or popup. Listing an action replaces its defaults, so you can drop one you dislike; anything you leave out stays as it was
- A line ctrlr cannot read keeps its default and is listed under **Config** at the top of the help popup, with what was wrong. A broken config never stops ctrlr from starting
- Commands can be edited before they run. `e` on a row opens it in an edit line with a real cursor — arrows, Home/End, Delete, Ctrl+U — and Enter hands the edited text to your prompt. `Ctrl+x` does the same from any pane, including the search bar
- From the edit line, `Ctrl+x` opens the command in `$VISUAL` / `$EDITOR`, the way `fc` and readline's `Ctrl+X Ctrl+E` do. ctrlr steps out of the way while the editor runs and picks the text back up when it exits. Quitting the editor with a non-zero status, or emptying the file, leaves the line as it was
- Editing never touches ctrlr's database. The original command keeps its favorite, tags and run count, because the original is not what ran; the edited version appears on the next launch once your shell has actually run it
- `Ctrl+C` now cancels, alongside `Esc`. Both behave the same way and in stages: the first press closes whatever is on top — a popup, then a tag or collection prompt — then clears the search box, and only exits ctrlr once there is nothing left to close

### Changed
- The help popup and the footer now show the keys that are actually bound, instead of a hand-written list that had already drifted. Rebind a key and both follow
- Esc no longer has its own special case in the event loop, and the individual key handlers no longer carry Esc arms. Both cancel keys resolve through one place, so they cannot drift apart
- Enter on a help-popup entry now runs exactly the same code as its keybinding. A few entries had quietly grown their own slightly different behaviour

---

## [0.8.0] - 2026-08-15

### Added
- ctrlr now responds to the mouse. The wheel scrolls whichever list is under the pointer, a click selects a row and focuses its pane, a double-click runs the command, and the tabs and search bar are clickable. Popups scroll with the wheel and close when you click outside them
- Right-clicking a row opens a context menu with Run, Copy, Favorite, Tag, and Add to / Remove from collection — whichever of those apply to the current view. The menu takes the keyboard as well: `j` / `k` to move, Enter to pick, Esc to close
- Right-clicking the collections pane opens a menu for the collection itself: Open, Rename, Delete and New collection. Deleting still asks for confirmation, and right-clicking an empty pane offers to create the first collection
- Pane widths are yours to set. `<` and `>` narrow and widen the details pane, or the collections pane when that one has focus, and the width is remembered across launches. Panes shrink toward a minimum as the terminal narrows and step aside entirely when there is no room left, so a small window still shows a usable list
- The same panes resize by dragging their border with the mouse. Dragging the details border past its minimum hides the pane, and `d` brings it back at the width it had
- The help popup (`?`) documents the mouse gestures and the resize keys, and it no longer hides shortcuts that apply everywhere
- ctrlr holds the mouse for as long as it runs, so your terminal's own drag-to-select needs **Shift** held while ctrlr is open. Mouse reporting is released when ctrlr exits, including on a panic

### Changed
- Building ctrlr from source now needs Rust 1.88 (was 1.86). Several crates in the dependency tree raised their own minimum; the installer and the released binaries are unaffected
- The History list draws several times faster. It used to build a styled line for every command in your history on every frame — a thousand commands meant tens of milliseconds per redraw, which showed up as lag while dragging a pane divider. Only the visible rows are built now, so redraw cost no longer grows with the size of your history

---

## [0.7.0] - 2026-08-14

### Added
- ctrlr now records **where** each command ran. No shell writes the working directory to its history file, so the shell integration appends one line per command to a run log (`~/.local/share/ctrlr/runs.log`) using a shell builtin — no process is spawned per prompt — and ctrlr drains it into a new `command_runs` table on launch. The details panel shows how often a command was recorded, how often in the current directory, its last exit code and the directories it runs in most. Data accumulates going forward; it cannot be recovered from existing history
- zsh and fish capture the directory the command was *typed* in, so a `cd` is attributed correctly. On bash the same holds when bash-preexec is loaded (starship and atuin both bring it); without it ctrlr falls back to logging at prompt time, where a `cd` is attributed to the directory it moved to
- ctrlr now offers to install or update the shell integration in a popup on startup, and can write it for you — your existing config is copied to `<config>.ctrlr.bak` first. Launched through the `Ctrl+R` integration, it puts `exec bash` on the prompt line afterwards so the reload is one keypress; run directly, it tells you to restart your shell. The popup also points at `ctrlr init` for anyone who would rather do it by hand. Declining silences it for 40 launches and it gives up after three declines, saying so on the last one — a changed integration script asks again regardless
- Run the shell integration update with `ctrlr init` to start recording. ctrlr now also says so on stdout when the installed integration is outdated — it previously only spoke up when none was installed at all, so an upgrade looked like it had done nothing: the old block still binds `Ctrl+R`, while the features it has no hooks for stay silently empty
- Commands recorded in the current directory now rank above ones that were not, between recency and an explicit favourite, so searching in a repo surfaces what you run there without pressing anything
- `.` (and `Alt+.` from the search bar) scopes the list to commands recorded in the current directory. The scoped directory stays visible in the footer. The toggle declines while nothing is recorded for the directory rather than emptying the list

### Fixed
- `ctrlr init --print` no longer **deletes** the integration block from your shell config. When the installed block was outdated, init removed it before deciding what to do next — so asking it to only print the script, or answering `n` at the confirmation, left the config with no integration at all and nothing put back. Nothing is written now until after the prompt, and the strip and the install land in a single write
- The bash integration no longer `export`s `PROMPT_COMMAND`. Exporting it leaks the hook into every child bash and subshell, which installs it a second time
- Updating an outdated shell integration no longer leaves part of the old block behind in the shell config. Removal counted a fixed number of lines, which was already shorter than the block it was removing; blocks now carry an explicit end marker, and older ones are cut at their key binding

---

## [0.6.1] - 2026-07-21

### Added
- `Alt+1`, `Alt+2` and `Alt+3` switch between the History, Favorites and Collections views from any pane, including while the search bar is focused. `F1` opens the help popup from anywhere. The view tabs now display the `Alt+1/2/3` hints

### Fixed
- The search bar now accepts `1`, `2`, `3` and `?` as literal input. Because the search bar is focused by default, these keys previously always switched view or opened help — making them impossible to type in a query. They now type into the query when the search bar has focus, while keeping their shortcut behaviour in the list panes

---

## [0.6.0] - 2026-07-17

### Added
- `Ctrl+u` clears the search bar when it is focused, following the readline convention. It still pages up in the list panes, and `PageUp` is unchanged
- `Ctrl+u` also clears the input in the tag, collection and import/export popups

### Fixed
- Search field now accepts uppercase letters (Shift+letter combinations) instead of silently dropping them
- Clearing the search with `Esc` no longer leaves the selection on whatever row it was on, pointing at an unrelated command once the full list returns; it now goes back to the top
- Commands whose text was not already lowercase (e.g. `Git Status`) could occupy two database rows and appear twice in the list, permanently. Command ids were hashed from normalized text in one place and raw text everywhere else, so favoriting or tagging such a command wrote a second row instead of updating the first. All ids now derive from the normalized text
- Existing databases are repaired on first launch by a one-time migration that merges the duplicated rows, summing use counts, keeping the command favorited if either copy was, and preserving tags and collection membership. The database is backed up to `ctrlr.db.pre-migration.bak` beforehand
- Exporting and re-importing is now idempotent for those commands; previously they were re-imported as duplicates
- Importing an export written by an older version no longer fails with `UNIQUE constraint failed: commands.id` when the file contains commands differing only by case
- Renaming a collection no longer silently drops every command's membership on export/import. Exports now carry `collection_ids`; files without it still import
- Quitting with `Esc` no longer leaves the terminal cursor invisible until you reopen ctrlr and pick a command. ctrlr draws its own cursor glyph, so ratatui hides the real one on every frame and only restores it when the terminal is dropped — which the cancel path skipped by exiting the process outright. Only affected launches via the shell integration, which always passes `--output-file`

### Changed
- Commands differing only by case or surrounding whitespace now share one entry in storage, and therefore one set of favorites, tags and usage counts. The list already treated them as one; storage now agrees. The text you execute is unchanged

---

## [0.5.1] - 2026-05-28

### Added
- Import/export shortcuts (`Ctrl+E`, `Ctrl+O`) shown in help popup across all views

### Fixed
- Commands added to collections that never existed in shell history are now persisted across restarts
- DB operations in `add_command_to_collection` now run before in-memory state updates to prevent inconsistency on error

---

## [0.5.0] - 2026-05-04

### Added
- **Import/export TUI popups** for backup and sharing
  - `Ctrl+E` – export popup (type file path, Enter to export)
  - `Ctrl+O` – import popup (type file path, Enter for preview, Enter again to import)
  - Import mode selector: Merge (default) or Replace
  - Import preview shows new commands, collections, and duplicates
  - Esc to close popups without exiting app
- Catppuccin theme support with 4 flavors (Latte, Frappe, Macchiato, Mocha)
  - Theme selector popup with color swatches and live preview (`Ctrl+T`)
  - j/k or ↑/↓ navigation with instant theme switching
  - Esc to cancel and revert to previous theme
  - Current theme name displayed in footer (e.g. `Ctrl+T: Theme (Mocha)`)
  - `Ctrl+t` shortcut added to help popup
- Theme persistence in SQLite `settings` table
  - Selected theme is saved and restored on startup
- **Import/export** via CLI with JSON format
  - `ctrlr export` – export all data to stdout or file
  - `ctrlr import backup.json` – import with merge mode (default)
  - `ctrlr import --dry-run` – preview changes without applying
  - `ctrlr import --replace` – replace all data (with y/N confirmation)
  - Deduplication by command text hash; tags and collections are merged
  - Versioned export format for future migration support

### Changed
- Each theme now has distinct selection highlight colors (Mocha: mauve, Macchiato: sky, Frappe: teal, Latte: blue)
- Unfocus border colors use theme-specific subtle colors instead of generic `DarkGray`

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
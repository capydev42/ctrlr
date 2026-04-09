# ctrlr

> Turn your shell history into a searchable command palette  
> Stop googling commands you already used.

<!-- TODO: Add demo gif -->

---

## Features

- **Instant search** through your shell history
- **Favorites** for frequently used commands
- **Tags & collections** to organize commands
- **Fast TUI** powered by `ratatui`
- **Keyboard-first workflow**
- Works with bash, zsh, fish

---

## Usage

### Open the command picker

```bash
ctrlr
```

Search, select, execute.

### Enable Ctrl+R integration

```bash
ctrlr init
```

Replaces your default reverse search with ctrlr.

---

## Why ctrlr?

Default shell history search is:
- Linear and hard to navigate
- Not searchable in a meaningful way
- Impossible to organize

ctrlr gives you:
- Fuzzy search
- Favorites & tagging
- Structure over time

### Motivation

I kept forgetting useful commands and re-googling them.

ctrlr turns your shell history into a personal command palette.

---

## Installation

### From Release (recommended)

Download from: https://github.com/ger4ik/ctrlr/releases

```bash
tar -xzf ctrlr-x86_64-unknown-linux-gnu.tar.gz
sudo mv ctrlr /usr/local/bin/
```

### From Source

```bash
git clone https://github.com/ger4ik/ctrlr.git
cd ctrlr
cargo build --release
sudo cp target/release/ctrlr /usr/local/bin/
```

---

## Keybindings

### Global

| Key   | Action                      |
|-------|----------------------------|
| Tab   | Switch pane                 |
| Enter | Select command / Focus list |
| Esc   | Exit / cancel               |
| 1     | Show History                |
| 2     | Show Favorites              |
| 3     | Show Collections            |
| c     | Add to collection           |

### Navigation (History / Favorites)

| Key          | Action           |
|--------------|------------------|
| j / k        | Navigate (vim)   |
| Up / Down    | Navigate         |
| /            | Jump to search   |
| Enter        | Execute command  |

### Search

| Key       | Action           |
|-----------|------------------|
| Type      | Search           |
| Backspace | Delete character |
| Enter     | Focus list       |
| Esc       | Clear / exit     |

### Tag Editor

| Key       | Action              |
|-----------|---------------------|
| Type      | Add tags            |
| Up / Down | Navigate suggestions|
| Enter     | Select / Create     |
| Tab       | Autocomplete        |
| Esc       | Cancel              |

### Collections View

| Key   | Action                        |
|-------|-------------------------------|
| n     | Create new collection         |
| e     | Edit / rename collection      |
| d     | Delete collection             |
| r     | Remove command from collection |

---

## Storage

Data is stored locally using SQLite:

- Command metadata (favorites, usage)
- Tags
- Collections

**Locations:**

- Linux: `~/.local/share/ctrlr/ctrlr.db`
- macOS: `~/Library/Application Support/ctrlr/ctrlr.db`

---

## Roadmap

- [x] Fuzzy search
- [x] Favorites & tags
- [x] Collections
- [ ] Better ranking (recency + frequency)
- [ ] Improved tagging UX
- [ ] Command preview / details panel
- [ ] Plugin / extensibility ideas

---

## Contributing

Ideas, feedback, and UX suggestions are very welcome.

# ctrlr

> A fast, keyboard-driven command history picker for your shell

**ctrlr** is a small TUI tool to search, filter, and reuse your shell history — with tags, favorites, and usage tracking.

---

## Status

This project is currently a **work-in-progress MVP / prototype**.

- Built primarily for my own workflow
- APIs, UX, and structure may change frequently
- Things might be rough around the edges
- Look and feel currently are not at peak yet =)

That said — it's already usable and evolving quickly.

---

## Features

- **Interactive search** through your shell history
- **Favorites** for frequently used commands
- **Tagging system** to organize commands
- **Usage tracking** (use count, last used) - not really used atm
- **Fast TUI interface** (powered by `ratatui`)
- **Keyboard-first navigation**
- Supports: bash, zsh, fish

---

## Motivation

I am building this because:
- I did not figure out how to remember all the hundreds of different commands I use when I work with different tech stacks.
- I still have no idea how others work with that many different commands without googling them frequently.

`ctrlr` is my attempt to turn shell history into something closer to a **personal command palette**.

---

## Installation

Currently, the recommended way is to build from source:

```bash
git clone https://github.com/ger4ik/ctrlr.git
cd ctrlr
cargo build --release
```

Or checkout the [pre-built release packages](https://github.com/ger4ik/ctrlr/releases).

---

## Usage

### Start the TUI

```bash
ctrlr
```

### Add shell integration (Ctrl+R)

```bash
ctrlr init
```

---

## Keybindings

### Global

| Key | Action |
|-----|--------|
| Tab | Switch pane |
| Enter | Select command |
| Esc | Clear search / exit |

### Search

| Key | Action |
|-----|--------|
| Type | Search |
| Backspace | Delete |

### History

| Key | Action |
|-----|--------|
| j / k or ↑ / ↓ | Navigate |
| f | Toggle favorite |
| t | Edit tags |
| / | Jump to search |

### Tag Editor

| Key | Action |
|-----|--------|
| Type | Add tags |
| Tab | Autocomplete |
| Enter | Save |
| Esc | Cancel |
| ← / → | Select existing tags |
| Backspace | Delete tag |

---

## Storage

Data is stored locally using SQLite:

- Command metadata (favorites, usage)
- Tags
- Relationships between commands and tags

**Location (platform-dependent):**

- Linux: `~/.local/share/ctrlr/ctrlr.db`
- macOS: `~/Library/Application Support/ctrlr/ctrlr.db`

---

## Caveats

- No Windows support (yet)
- UI is still evolving
- No fuzzy search (yet)
- Performance not heavily optimized
- Some edge cases in history parsing likely exist

---

## Roadmap (very loose)

- [ ] Better search (fuzzy / ranking)
- [ ] Improved tag UX
- [ ] Smarter sorting (recency + frequency)
- [ ] Preview / command details panel
- [ ] Plugin / extensibility ideas
- [ ] Potential shell expansion support

---

## Contributing

Right now, this is mainly a personal project — but:

- Ideas
- Feedback
- UX suggestions

are very welcome.

---

## License

TBD

---

## Notes

This is my first iteration of this idea, and also an exploration of:

- Rust TUI development
- Terminal UX design
- Personal tooling

If it breaks, feels weird, or incomplete — that's expected :)

But if it already improves your workflow, that's even better.
# ctrlr

> Supercharged command history for your terminal.

`ctrlr` is a fast, minimal command history manager with a Terminal User Interface (TUI).
It helps you search, organize, and reuse your shell commands more efficiently than traditional `Ctrl+R`.

---

## Features (MVP)

* **Fast Search**

  * Fuzzy search through your command history (like fzf, but focused)

* **Favorites**

  * Mark frequently used commands for quick access

* **Tags**

  * Organize commands with simple tags like `#git`, `#docker`

* **Interactive TUI**

  * Navigate with keyboard
  * Select and reuse commands instantly

* **Persistent Storage**

  * Commands are stored in a local SQLite database

---

## Usage

Start the interactive UI:

```bash
ctrlr
```

Select a command and execute it:

```bash
eval "$(ctrlr)"
```

---

## Keybindings

| Key   | Action          |
| ----- | --------------- |
| ↑ / ↓ | Navigate        |
| Enter | Select command  |
| /     | Search          |
| f     | Toggle favorite |
| t     | Add tag         |
| Esc   | Exit            |

---

## Installation

*(to be added)*

---

## Philosophy

Traditional shell history is just a log.

`ctrlr` turns it into a **tool**:

* Find commands faster
* Reuse them smarter
* Organize your workflows

---

## Roadmap

* [ ] Advanced filtering
* [ ] Snippets with placeholders
* [ ] Timeline view
* [ ] Cross-device sync

---

## Platform Support

* ✅ Linux (Bash, Zsh)
* ✅ macOS (Bash, Zsh)
* ⚠️ Windows (WSL recommended)




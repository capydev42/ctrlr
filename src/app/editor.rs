//! Handing the edit line to `$VISUAL` / `$EDITOR`, the way readline's
//! `Ctrl+X Ctrl+E` does.
//!
//! This is the only place in ctrlr that gives the terminal to a child process.
//! Everything else that spawns (`clipboard`, `history`) pipes its output and
//! never touches the tty, so the suspend/resume dance lives here rather than
//! being a general facility.

use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use crossterm::event::{self, DisableMouseCapture, EnableMouseCapture};
use crossterm::execute;
use crossterm::terminal::{EnterAlternateScreen, enable_raw_mode};
use ratatui::DefaultTerminal;

/// What came back from the editor. `text` is `None` whenever the buffer should
/// be left as it was — a non-zero exit, an unreadable file, or an emptied line.
pub struct EditorOutcome {
    pub text: Option<String>,
    pub message: Option<String>,
}

impl EditorOutcome {
    fn kept(message: impl Into<String>) -> Self {
        Self {
            text: None,
            message: Some(message.into()),
        }
    }
}

/// Suspends the TUI, runs the editor on `initial`, and resumes.
///
/// The terminal is always restored, whatever the child did — every failure
/// path after the suspend goes through the resume below.
pub fn edit_in_external_editor(terminal: &mut DefaultTerminal, initial: &str) -> EditorOutcome {
    let argv = resolve_editor(std::env::var("VISUAL").ok(), std::env::var("EDITOR").ok());
    let Some((prog, args)) = argv.split_first() else {
        return EditorOutcome::kept("Set $EDITOR to edit externally");
    };

    // Written before the terminal is touched, so a failure here costs nothing.
    let path = temp_path();
    if let Err(e) = write_temp(&path, initial) {
        return EditorOutcome::kept(format!("Could not create a temp file: {}", e));
    }

    suspend();
    let status = Command::new(prog).args(args).arg(&path).status();
    resume(terminal);

    let outcome = match status {
        Err(e) => EditorOutcome::kept(format!("Could not run {}: {}", prog, e)),
        Ok(status) if !status.success() => EditorOutcome::kept(format!(
            "{} exited with {} — text unchanged",
            prog,
            status.code().unwrap_or(-1)
        )),
        Ok(_) => match std::fs::read_to_string(&path) {
            Err(e) => EditorOutcome::kept(format!("Could not read the edited file: {}", e)),
            Ok(contents) => interpret(&contents),
        },
    };

    let _ = std::fs::remove_file(&path);
    outcome
}

/// Always present, and a working default beats an error message.
const DEFAULT_EDITOR: &str = if cfg!(windows) { "notepad" } else { "vi" };

/// `$VISUAL` wins over `$EDITOR`, then [`DEFAULT_EDITOR`]. Split here rather
/// than through `sh -c`, which would drag in quoting hazards for no gain.
fn resolve_editor(visual: Option<String>, editor: Option<String>) -> Vec<String> {
    visual
        .into_iter()
        .chain(editor)
        .map(|v| split_command(&v))
        .find(|argv| !argv.is_empty())
        .unwrap_or_else(|| vec![DEFAULT_EDITOR.to_owned()])
}

/// Splits on whitespace, but honours quotes so a path containing spaces can be
/// given as `"C:\Program Files\...\Code.exe" --wait`. An unquoted value that
/// names an existing file is taken whole, which covers the same path without
/// quotes as long as no arguments follow it.
fn split_command(value: &str) -> Vec<String> {
    let value = value.trim();
    if value.is_empty() {
        return Vec::new();
    }
    if !value.contains(['"', '\'']) && Path::new(value).is_file() {
        return vec![value.to_owned()];
    }

    let mut argv = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    let mut started = false;

    for c in value.chars() {
        match quote {
            Some(q) if c == q => quote = None,
            Some(_) => current.push(c),
            None if c == '"' || c == '\'' => {
                quote = Some(c);
                started = true;
            }
            None if c.is_whitespace() => {
                if started {
                    argv.push(std::mem::take(&mut current));
                    started = false;
                }
            }
            None => {
                current.push(c);
                started = true;
            }
        }
    }
    if started {
        argv.push(current);
    }
    argv.retain(|a| !a.is_empty());
    argv
}

/// Editors append a trailing newline; anything else the user wrote is kept
/// verbatim, interior newlines included. An emptied file means "cancel" — an
/// empty string must never reach `run_tui`, which writes an empty output file
/// to tell the shell nothing was chosen.
fn interpret(contents: &str) -> EditorOutcome {
    let text = contents.trim_end_matches('\n');
    if text.trim().is_empty() {
        return EditorOutcome::kept("Nothing to run — text unchanged");
    }
    EditorOutcome {
        text: Some(text.to_owned()),
        message: None,
    }
}

/// Picks a syntax mode in the editor, and on Windows avoids handing a `.sh`
/// to something that has no idea what that is.
const EDIT_SUFFIX: &str = if cfg!(windows) { ".ps1" } else { ".sh" };

fn temp_path() -> PathBuf {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    std::env::temp_dir().join(format!(
        "ctrlr-edit-{}-{}{}",
        std::process::id(),
        unique,
        EDIT_SUFFIX
    ))
}

/// `create_new` rather than `create`: it fails instead of following a symlink
/// someone planted at the path, and 0600 keeps the command out of other users'
/// reach while the editor holds it.
fn write_temp(path: &Path, contents: &str) -> io::Result<()> {
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    writeln!(file, "{}", contents)
}

fn suspend() {
    let _ = execute!(io::stdout(), DisableMouseCapture);
    let _ = ratatui::try_restore();
}

/// Deliberately not `ratatui::init()`: that installs a panic hook on every
/// call, so editing ten commands would leave ten chained hooks. The hook
/// `run_tui` set up is still in place and still correct.
fn resume(terminal: &mut DefaultTerminal) {
    let _ = enable_raw_mode();
    let _ = execute!(io::stdout(), EnterAlternateScreen, EnableMouseCapture);
    // The child wrote all over the alternate screen, but ratatui's diff still
    // believes the pre-suspend frame is up there and would redraw almost
    // nothing.
    let _ = terminal.clear();
    // Mode switches and any terminal replies to the editor's own queries land
    // in the input queue; without this they are read as keystrokes.
    while event::poll(Duration::ZERO).unwrap_or(false) {
        let _ = event::read();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_editor_prefers_visual() {
        assert_eq!(
            resolve_editor(Some("hx".into()), Some("vim".into())),
            vec!["hx"]
        );
    }

    #[test]
    fn test_resolve_editor_falls_back_to_editor_then_default() {
        assert_eq!(resolve_editor(None, Some("vim".into())), vec!["vim"]);
        assert_eq!(resolve_editor(None, None), vec![DEFAULT_EDITOR]);
    }

    /// A blank or whitespace-only variable must not become an empty argv.
    #[test]
    fn test_resolve_editor_ignores_blank_variables() {
        assert_eq!(
            resolve_editor(Some("".into()), Some("vim".into())),
            vec!["vim"]
        );
        assert_eq!(
            resolve_editor(Some("   ".into()), None),
            vec![DEFAULT_EDITOR]
        );
    }

    #[test]
    fn test_resolve_editor_splits_arguments() {
        assert_eq!(
            resolve_editor(None, Some("code --wait".into())),
            vec!["code", "--wait"]
        );
    }

    /// `C:\Program Files\...` is the case plain whitespace splitting got
    /// wrong, and the reason quoting is honoured at all.
    #[test]
    fn test_resolve_editor_keeps_a_quoted_path_together() {
        assert_eq!(
            resolve_editor(None, Some(r#""C:\Program Files\Ed\ed.exe" --wait"#.into())),
            vec![r"C:\Program Files\Ed\ed.exe", "--wait"]
        );
        assert_eq!(
            resolve_editor(None, Some("'/opt/my ed/ed'".into())),
            vec!["/opt/my ed/ed"]
        );
    }

    /// An unquoted path with spaces is taken whole when it names a real file,
    /// which is the shape `$EDITOR` most often has on Windows.
    #[test]
    fn test_resolve_editor_takes_an_existing_path_whole() {
        let dir = tempfile::TempDir::new().unwrap();
        let editor = dir.path().join("my editor");
        std::fs::write(&editor, "").unwrap();
        let raw = editor.to_string_lossy().into_owned();

        assert_eq!(resolve_editor(None, Some(raw.clone())), vec![raw]);
    }

    /// The whole-path shortcut must not swallow arguments.
    #[test]
    fn test_resolve_editor_still_splits_a_path_with_arguments() {
        let dir = tempfile::TempDir::new().unwrap();
        let editor = dir.path().join("ed");
        std::fs::write(&editor, "").unwrap();
        let raw = format!("{} --wait", editor.to_string_lossy());

        assert_eq!(
            resolve_editor(None, Some(raw)),
            vec![editor.to_string_lossy().into_owned(), "--wait".to_owned()]
        );
    }

    #[test]
    fn test_temp_path_uses_a_platform_suffix() {
        let path = temp_path();
        assert!(path.to_string_lossy().ends_with(EDIT_SUFFIX));
    }

    #[test]
    fn test_interpret_strips_only_the_trailing_newline() {
        let out = interpret("git status\n");
        assert_eq!(out.text.as_deref(), Some("git status"));
        assert!(out.message.is_none());
    }

    #[test]
    fn test_interpret_keeps_interior_newlines() {
        let out = interpret("for f in *; do\n  echo $f\ndone\n");
        assert_eq!(out.text.as_deref(), Some("for f in *; do\n  echo $f\ndone"));
    }

    /// Emptying the file is how someone cancels from inside the editor.
    #[test]
    fn test_interpret_treats_an_emptied_file_as_a_cancel() {
        for contents in ["", "\n", "   \n\n"] {
            let out = interpret(contents);
            assert!(out.text.is_none(), "{:?} should cancel", contents);
            assert!(out.message.is_some());
        }
    }

    #[test]
    fn test_write_temp_refuses_an_existing_path() {
        let path = temp_path();
        write_temp(&path, "first").unwrap();
        assert!(write_temp(&path, "second").is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "first\n");
        std::fs::remove_file(&path).unwrap();
    }
}

//! `~/.config/ctrlr/config.toml` — user keybindings.
//!
//! Nothing here is fatal. A missing file, a syntax error, an unknown action:
//! ctrlr starts either way, with the default binding for whatever it could not
//! read, and says what went wrong. That matches how the rest of ctrlr treats
//! its own storage — `db: Option<Connection>`, swallowed migration failures —
//! and it matters more here, because a config that refused to start would lock
//! someone out of the tool with their own typo.

use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::keymap::{KeyAction, KeyContext, Keymap, defaults, format_binding, parse_binding};

/// Something in the file that could not be applied. The entry keeps its default.
#[derive(Clone, Debug, PartialEq)]
pub struct ConfigProblem {
    pub entry: String,
    pub message: String,
}

impl std::fmt::Display for ConfigProblem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.entry.is_empty() {
            f.write_str(&self.message)
        } else {
            write!(f, "{}: {}", self.entry, self.message)
        }
    }
}

pub fn config_path() -> Option<PathBuf> {
    Some(dirs::config_dir()?.join("ctrlr").join("config.toml"))
}

/// The keymap to run with, plus anything that could not be read.
pub fn load() -> (Keymap, Vec<ConfigProblem>) {
    let Some(path) = config_path() else {
        return (defaults::keymap(), Vec::new());
    };
    match std::fs::read_to_string(&path) {
        Ok(source) => from_str(&source),
        // No file is the normal case, not a problem worth reporting.
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => (defaults::keymap(), Vec::new()),
        Err(e) => (
            defaults::keymap(),
            vec![ConfigProblem {
                entry: path.display().to_string(),
                message: format!("could not be read: {}", e),
            }],
        ),
    }
}

/// One or several keys for the same action, so both spellings work:
/// `edit = "e"` and `edit = ["e", "ctrl+x"]`.
#[derive(Deserialize)]
#[serde(untagged)]
enum OneOrMany {
    One(String),
    Many(Vec<String>),
}

impl OneOrMany {
    fn into_vec(self) -> Vec<String> {
        match self {
            OneOrMany::One(s) => vec![s],
            OneOrMany::Many(v) => v,
        }
    }
}

#[derive(Deserialize, Default)]
struct Config {
    #[serde(default)]
    keys: HashMap<String, HashMap<String, OneOrMany>>,
}

/// The testable half of [`load`]: no filesystem.
pub fn from_str(source: &str) -> (Keymap, Vec<ConfigProblem>) {
    let mut keymap = defaults::keymap();
    let mut problems = Vec::new();

    let config: Config = match toml::from_str(source) {
        Ok(c) => c,
        Err(e) => {
            problems.push(ConfigProblem {
                entry: "config.toml".into(),
                // toml's message already carries the line and column.
                message: e.message().to_string(),
            });
            return (keymap, problems);
        }
    };

    for (context_name, actions) in config.keys {
        let Some(context) = KeyContext::from_str(&context_name) else {
            problems.push(ConfigProblem {
                entry: format!("[keys.{}]", context_name),
                message: "unknown context".into(),
            });
            continue;
        };
        for (action_name, keys) in actions {
            let entry = format!("[keys.{}] {}", context_name, action_name);
            let Some(action) = KeyAction::from_str(&action_name) else {
                problems.push(ConfigProblem {
                    entry,
                    message: "unknown action".into(),
                });
                continue;
            };

            let keys = keys.into_vec();
            let mut parsed = Vec::new();
            let mut failed = false;
            for key in &keys {
                match parse_binding(key) {
                    Ok(binding) => parsed.push(binding),
                    Err(e) => {
                        problems.push(ConfigProblem {
                            entry: entry.clone(),
                            message: e.to_string(),
                        });
                        failed = true;
                    }
                }
            }
            // All or nothing per entry: applying half a list would leave a
            // binding nobody wrote.
            if failed {
                continue;
            }

            // Replace rather than extend, or a default key could never be
            // removed.
            keymap.clear_action(context, action);
            for binding in parsed {
                // A rebind onto an occupied key takes it; leaving the old owner
                // in place would make the new binding dead on arrival.
                if let Some(previous) = keymap.evict(context, &binding)
                    && previous != action
                {
                    problems.push(ConfigProblem {
                        entry: entry.clone(),
                        message: format!(
                            "{} took {} from {}",
                            action.as_str(),
                            format_binding(&binding),
                            previous.as_str()
                        ),
                    });
                }
                keymap.bind(context, binding, action);
            }
        }
    }

    (keymap, problems)
}

/// The whole default keymap as valid TOML, for `ctrlr config --print`.
/// Round-tripping it must be a no-op — a test asserts that.
pub fn print_defaults() -> String {
    let mut out = String::from(
        "# ctrlr keybindings\n\
         #\n\
         # Listing an action REPLACES its default keys, so you can drop one you\n\
         # dislike. Anything you leave out keeps the default below.\n\
         #\n\
         # Modifiers: ctrl, alt, shift. Named keys: enter, esc, tab, space,\n\
         # backspace, delete, insert, home, end, pageup, pagedown, up, down,\n\
         # left, right, f1-f12. Anything else of length one is that character,\n\
         # case-sensitively. Two keys separated by a space make a sequence.\n\
         #\n\
         # A plain character always types when the search bar has focus, so a\n\
         # letter bound here only fires from the other panes.\n",
    );
    for &context in KeyContext::ALL {
        let entries = defaults::keymap();
        let entries = entries.entries(context);
        if entries.is_empty() {
            continue;
        }
        out.push_str(&format!("\n[keys.{}]\n", context.as_str()));
        let mut seen: Vec<KeyAction> = Vec::new();
        for (_, action) in entries {
            if seen.contains(action) {
                continue;
            }
            seen.push(*action);
            let keys: Vec<String> = entries
                .iter()
                .filter(|(_, a)| a == action)
                .map(|(b, _)| format!("{:?}", format_binding(b)))
                .collect();
            out.push_str(&format!("{} = [{}]\n", action.as_str(), keys.join(", ")));
        }
    }

    // Listed so the file is self-describing: an action absent from every
    // context above is still bindable there, and a typo in a name is the most
    // likely reason an entry silently falls back to its default.
    out.push_str("\n# Every action name ctrlr understands:\n");
    for chunk in KeyAction::ALL.chunks(4) {
        let names: Vec<&str> = chunk.iter().map(|a| a.as_str()).collect();
        out.push_str(&format!("#   {}\n", names.join(", ")));
    }
    out
}

/// The keymap as TOML, recording only what differs from the defaults.
///
/// A full dump would be wrong for a file ctrlr maintains: it pins today's
/// defaults forever, so a later change to them would never reach anyone who
/// used the editor once. An action whose keys were all removed is written as
/// `action = []`, which [`from_str`] already reads as "unbind".
pub fn to_toml(keymap: &Keymap) -> String {
    let defaults = defaults::keymap();
    let mut out = String::from(
        "# ctrlr keybindings — written by ctrlr, safe to edit by hand.\n\
         #\n\
         # Only differences from the defaults are recorded, so anything not\n\
         # named here follows ctrlr's own default and keeps following it.\n\
         # An empty list unbinds. `ctrlr config --print` shows every default\n\
         # and every action name.\n",
    );

    for &context in KeyContext::ALL {
        // Actions in the order the defaults list them, then anything the user
        // bound that has no default here — stable output, so re-saving an
        // unchanged keymap produces an identical file.
        let mut actions: Vec<KeyAction> = Vec::new();
        for (_, action) in defaults.entries(context) {
            if !actions.contains(action) {
                actions.push(*action);
            }
        }
        for (_, action) in keymap.entries(context) {
            if !actions.contains(action) {
                actions.push(*action);
            }
        }

        let changed: Vec<(KeyAction, Vec<String>)> = actions
            .into_iter()
            .filter_map(|action| {
                let mine = keymap.keys_for(context, action);
                if mine == defaults.keys_for(context, action) {
                    None
                } else {
                    Some((action, mine))
                }
            })
            .collect();

        if changed.is_empty() {
            continue;
        }
        out.push_str(&format!("\n[keys.{}]\n", context.as_str()));
        for (action, keys) in changed {
            let keys: Vec<String> = keys.iter().map(|k| format!("{:?}", k)).collect();
            out.push_str(&format!("{} = [{}]\n", action.as_str(), keys.join(", ")));
        }
    }
    out
}

/// Writes the keymap to [`config_path`], backing up whatever was there.
///
/// The backup is what makes overwriting acceptable: this destroys any comments
/// or ordering the user had, and `config.toml.ctrlr.bak` is where they get them
/// back. Same convention `ctrlr init` uses for shell rc files.
///
/// The write is a temp file plus a rename, so a crash cannot leave a truncated
/// config that fails to parse on the next launch.
pub fn save(keymap: &Keymap) -> io::Result<Saved> {
    let path =
        config_path().ok_or_else(|| io::Error::other("no config directory on this system"))?;
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }

    let backup = match fs::read(&path) {
        Ok(previous) => {
            let backup = backup_path(&path);
            fs::write(&backup, previous)?;
            Some(backup)
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => None,
        Err(e) => return Err(e),
    };

    // Same directory as the target, or the rename could cross a filesystem.
    let temp = path.with_extension("toml.tmp");
    fs::write(&temp, to_toml(keymap))?;
    fs::rename(&temp, &path)?;
    Ok(Saved { path, backup })
}

pub struct Saved {
    pub path: PathBuf,
    pub backup: Option<PathBuf>,
}

/// `.ctrlr.bak` appended rather than substituted: `with_extension` would turn
/// `config.toml` into `config.ctrlr.bak` and lose which file it came from.
fn backup_path(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(".ctrlr.bak");
    PathBuf::from(name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keymap::{Binding, KeyContext as C};

    fn bound(keymap: &Keymap, context: C, action: KeyAction) -> Vec<String> {
        keymap.keys_for(context, action)
    }

    #[test]
    fn test_empty_config_keeps_every_default() {
        let (keymap, problems) = from_str("");
        assert!(problems.is_empty());
        assert_eq!(
            bound(&keymap, C::History, KeyAction::ToggleFavorite),
            vec!["f"]
        );
    }

    /// Replace, not extend: otherwise a default key could never be dropped.
    #[test]
    fn test_a_rebind_replaces_the_defaults() {
        let (keymap, problems) = from_str("[keys.history]\ntoggle_favorite = \"v\"\n");
        assert!(problems.is_empty(), "{:?}", problems);
        assert_eq!(
            bound(&keymap, C::History, KeyAction::ToggleFavorite),
            vec!["v"]
        );
    }

    #[test]
    fn test_a_single_key_and_a_list_both_work() {
        let (one, _) = from_str("[keys.history]\ntoggle_favorite = \"v\"\n");
        let (many, _) = from_str("[keys.history]\ntoggle_favorite = [\"v\", \"ctrl+b\"]\n");
        assert_eq!(
            bound(&one, C::History, KeyAction::ToggleFavorite),
            vec!["v"]
        );
        assert_eq!(
            bound(&many, C::History, KeyAction::ToggleFavorite),
            vec!["v", "Ctrl+b"]
        );
    }

    #[test]
    fn test_an_unknown_action_keeps_its_neighbours() {
        let (keymap, problems) = from_str("[keys.history]\nlaunch_rocket = \"r\"\n");
        assert_eq!(problems.len(), 1);
        assert!(problems[0].message.contains("unknown action"));
        assert_eq!(
            bound(&keymap, C::History, KeyAction::ToggleFavorite),
            vec!["f"],
            "the rest of the context survives"
        );
    }

    #[test]
    fn test_an_unknown_context_is_reported() {
        let (keymap, problems) = from_str("[keys.nowhere]\ntoggle_favorite = \"v\"\n");
        assert_eq!(problems.len(), 1);
        assert!(problems[0].message.contains("unknown context"));
        assert_eq!(
            bound(&keymap, C::History, KeyAction::ToggleFavorite),
            vec!["f"]
        );
    }

    #[test]
    fn test_an_unparseable_key_leaves_the_default_in_place() {
        let (keymap, problems) = from_str("[keys.history]\ntoggle_favorite = \"hyper+f\"\n");
        assert_eq!(problems.len(), 1);
        assert!(problems[0].message.contains("unknown modifier"));
        assert_eq!(
            bound(&keymap, C::History, KeyAction::ToggleFavorite),
            vec!["f"],
            "a half-applied entry would leave a binding nobody wrote"
        );
    }

    /// One bad key in a list must not apply the good ones either.
    #[test]
    fn test_a_partly_bad_list_is_rejected_whole() {
        let (keymap, problems) = from_str("[keys.history]\ntoggle_favorite = [\"v\", \"nope\"]\n");
        assert_eq!(problems.len(), 1);
        assert_eq!(
            bound(&keymap, C::History, KeyAction::ToggleFavorite),
            vec!["f"]
        );
    }

    #[test]
    fn test_broken_toml_falls_back_to_the_defaults() {
        let (keymap, problems) = from_str("[keys.history\ntoggle_favorite = \"v\"\n");
        assert_eq!(problems.len(), 1);
        assert_eq!(problems[0].entry, "config.toml");
        assert_eq!(
            bound(&keymap, C::History, KeyAction::ToggleFavorite),
            vec!["f"]
        );
    }

    /// A rebind onto a key another action already owns has to win, or it would
    /// be shadowed by the default and appear not to work at all.
    #[test]
    fn test_a_rebind_evicts_the_previous_owner_and_says_so() {
        let (keymap, problems) = from_str("[keys.history]\ntoggle_favorite = \"d\"\n");
        assert_eq!(
            bound(&keymap, C::History, KeyAction::ToggleFavorite),
            vec!["d"]
        );
        assert!(
            bound(&keymap, C::History, KeyAction::ToggleDetails).is_empty(),
            "the evicted action lost the key"
        );
        assert_eq!(problems.len(), 1, "the eviction is reported");
        assert!(problems[0].message.contains("toggle_details"));
    }

    #[test]
    fn test_scoping_only_touches_the_named_context() {
        let (keymap, _) = from_str("[keys.history]\nnavigate_down = \"m\"\n");
        assert_eq!(
            bound(&keymap, C::History, KeyAction::NavigateDown),
            vec!["m"]
        );
        assert_eq!(
            bound(&keymap, C::CollectionItems, KeyAction::NavigateDown),
            vec!["j"],
            "the other pane is untouched"
        );
    }

    #[test]
    fn test_sequences_survive_the_round_trip() {
        let (keymap, problems) = from_str("[keys.global]\ngo_to_top = \"z z\"\n");
        assert!(problems.is_empty(), "{:?}", problems);
        assert_eq!(bound(&keymap, C::Global, KeyAction::GoToTop), vec!["z z"]);
    }

    /// An untouched keymap has nothing to say, so saving one leaves a file
    /// that pins no defaults at all.
    #[test]
    fn test_to_toml_of_the_defaults_records_nothing() {
        let out = to_toml(&defaults::keymap());
        assert!(!out.contains("[keys."), "{}", out);
    }

    #[test]
    fn test_to_toml_records_only_what_changed() {
        let mut keymap = defaults::keymap();
        keymap.clear_action(C::History, KeyAction::ToggleFavorite);
        keymap.bind(
            C::History,
            crate::keymap::parse_binding("v").unwrap(),
            KeyAction::ToggleFavorite,
        );
        let out = to_toml(&keymap);
        assert!(out.contains("[keys.history]"), "{}", out);
        assert!(out.contains(r#"toggle_favorite = ["v"]"#), "{}", out);
        assert!(
            !out.contains("navigate_down"),
            "untouched actions stay out of the file: {}",
            out
        );
        assert!(!out.contains("[keys.global]"), "{}", out);
    }

    #[test]
    fn test_to_toml_writes_an_empty_list_for_an_unbound_action() {
        let mut keymap = defaults::keymap();
        keymap.clear_action(C::History, KeyAction::ToggleFavorite);
        let out = to_toml(&keymap);
        assert!(out.contains("toggle_favorite = []"), "{}", out);
    }

    /// The whole point of the editor writing the file: what it writes has to
    /// come back as what it had.
    #[test]
    fn test_to_toml_round_trips_through_from_str() {
        let mut keymap = defaults::keymap();
        keymap.clear_action(C::History, KeyAction::ToggleFavorite);
        keymap.bind(
            C::History,
            crate::keymap::parse_binding("v").unwrap(),
            KeyAction::ToggleFavorite,
        );
        keymap.clear_action(C::Global, KeyAction::GoToTop);
        keymap.bind(
            C::Global,
            crate::keymap::parse_binding("z z").unwrap(),
            KeyAction::GoToTop,
        );
        keymap.clear_action(C::CollectionsList, KeyAction::DeleteCollection);
        keymap.clear_action(C::Global, KeyAction::ShowHelp);
        keymap.bind(
            C::Global,
            crate::keymap::parse_binding("f2").unwrap(),
            KeyAction::ShowHelp,
        );

        let (reloaded, problems) = from_str(&to_toml(&keymap));
        assert!(problems.is_empty(), "{:?}", problems);
        for &context in C::ALL {
            let mut a: Vec<(Binding, KeyAction)> = reloaded.entries(context).to_vec();
            let mut b: Vec<(Binding, KeyAction)> = keymap.entries(context).to_vec();
            a.sort_by_key(|(bd, ac)| (format_binding(bd), ac.as_str()));
            b.sort_by_key(|(bd, ac)| (format_binding(bd), ac.as_str()));
            assert_eq!(a, b, "{:?} did not round-trip", context);
        }
    }

    /// Re-saving an unchanged keymap must not churn the file.
    #[test]
    fn test_to_toml_output_is_stable() {
        let mut keymap = defaults::keymap();
        keymap.clear_action(C::History, KeyAction::ToggleFavorite);
        keymap.bind(
            C::History,
            crate::keymap::parse_binding("v").unwrap(),
            KeyAction::ToggleFavorite,
        );
        let once = to_toml(&keymap);
        let (reloaded, _) = from_str(&once);
        assert_eq!(to_toml(&reloaded), once);
    }

    #[test]
    fn test_backup_path_appends_rather_than_replacing() {
        assert_eq!(
            backup_path(Path::new("/home/u/.config/ctrlr/config.toml")),
            PathBuf::from("/home/u/.config/ctrlr/config.toml.ctrlr.bak")
        );
    }

    /// `ctrlr config --print > config.toml` must change nothing, or the file it
    /// hands people is not a starting point.
    #[test]
    fn test_printed_defaults_round_trip_to_the_default_keymap() {
        let printed = print_defaults();
        let (keymap, problems) = from_str(&printed);
        assert!(problems.is_empty(), "{:?}", problems);
        let defaults = defaults::keymap();
        for &context in C::ALL {
            let mut from_file: Vec<(Binding, KeyAction)> = keymap.entries(context).to_vec();
            let mut built_in: Vec<(Binding, KeyAction)> = defaults.entries(context).to_vec();
            from_file.sort_by_key(|(b, a)| (format_binding(b), a.as_str()));
            built_in.sort_by_key(|(b, a)| (format_binding(b), a.as_str()));
            assert_eq!(from_file, built_in, "{:?} did not round-trip", context);
        }
    }
}

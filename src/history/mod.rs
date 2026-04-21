use crate::app::Command;

mod bash;
mod fish;
mod zsh;

use std::collections::{HashMap, HashSet};

// think about windows support (powershell? cmd?)

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct HistoryEntry {
    pub command: String,
    pub timestamp: Option<i64>,
}

pub fn flush_history() {
    let shell = detect_shell();
    let result = match shell {
        "bash" => std::process::Command::new("bash")
            .arg("-c")
            .arg("history -a")
            .output(),
        "zsh" => std::process::Command::new("zsh")
            .arg("-c")
            .arg("fc -W")
            .output(),
        "fish" => std::process::Command::new("fish")
            .arg("-c")
            .arg("history save")
            .output(),
        _ => return,
    };

    if let Err(e) = result {
        eprintln!("Failed to flush {} history: {}", shell, e);
    } else {
        let output = result.unwrap();
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            eprintln!("History flush failed for {}: {}", shell, stderr);
        }
    }
}

pub fn load_history() -> Vec<Command> {
    flush_history();

    let mut commands = Vec::new();

    let shell = detect_shell();

    let home = dirs::home_dir().unwrap_or_default();

    match shell {
        "bash" => {
            let path = home.join(".bash_history");
            for entry in bash::read_history(&path) {
                let normalized = normalize(&entry.command);
                commands.push(Command {
                    id: hash_command(&normalized),
                    text: entry.command,
                    tags: vec!["bash".to_string()],
                    collection_ids: vec![],
                    favorite: false,
                    _context: vec!["shell:bash".to_string()],
                    use_count: 0,
                    last_used: None,
                });
            }
        }
        "zsh" => {
            let path = home.join(".zsh_history");
            for entry in zsh::read_history(&path) {
                let normalized = normalize(&entry.command);
                commands.push(Command {
                    id: hash_command(&normalized),
                    text: entry.command,
                    tags: vec!["zsh".to_string()],
                    collection_ids: vec![],
                    favorite: false,
                    _context: vec!["shell:zsh".to_string()],
                    use_count: 0,
                    last_used: None,
                });
            }
        }
        "fish" => {
            let path = home.join(".local/share/fish/fish_history");
            for entry in fish::read_history(&path) {
                let normalized = normalize(&entry.command);
                commands.push(Command {
                    id: hash_command(&normalized),
                    text: entry.command,
                    tags: vec!["fish".to_string()],
                    collection_ids: vec![],
                    favorite: false,
                    _context: vec!["shell:fish".to_string()],
                    use_count: 0,
                    last_used: None,
                });
            }
        }
        _ => {}
    }

    commands
}

pub fn deduplicate(commands: Vec<Command>) -> Vec<Command> {
    let mut seen: HashMap<String, Command> = HashMap::new();
    let mut result: Vec<Command> = Vec::new();

    for cmd in commands.into_iter().rev() {
        let key = normalize(&cmd.text);

        if let Some(existing) = seen.get_mut(&key) {
            existing.use_count += cmd.use_count;

            let mut tags_set: HashSet<String> = existing.tags.drain(..).collect();
            tags_set.extend(cmd.tags.clone());
            existing.tags = tags_set.into_iter().collect();
            existing.tags.sort();

            existing.favorite |= cmd.favorite;

            if cmd.last_used > existing.last_used {
                existing.last_used = cmd.last_used;
            }

            for ctx in cmd._context.clone() {
                if !existing._context.contains(&ctx) {
                    existing._context.push(ctx);
                }
            }
        } else {
            seen.insert(key.clone(), cmd.clone());
            result.push(cmd);
        }
    }

    result.reverse();
    result
}

fn normalize(s: &str) -> String {
    s.trim().to_lowercase()
}

fn hash_command(text: &str) -> String {
    use sha1::{Digest, Sha1};
    let mut hasher = Sha1::new();
    hasher.update(text.as_bytes());
    format!("{:x}", hasher.finalize())
}

#[allow(dead_code)]
pub fn detect_shell() -> &'static str {
    std::env::var("SHELL")
        .unwrap_or_default()
        .split('/')
        .next_back()
        .map(|s| match s {
            "bash" => "bash",
            "zsh" => "zsh",
            "fish" => "fish",
            _ => "bash",
        })
        .unwrap_or("bash")
}

#[cfg(test)]
mod tests {
    use super::{deduplicate, hash_command, normalize};
    use crate::app::Command as AppCommand;

    fn make_cmd(text: &str, use_count: i32, tags: Vec<String>, favorite: bool) -> AppCommand {
        AppCommand {
            id: hash_command(text),
            text: text.to_string(),
            tags,
            collection_ids: vec![],
            favorite,
            _context: vec![],
            use_count,
            last_used: None,
        }
    }

    #[test]
    fn test_normalize_trims_and_lowercases() {
        assert_eq!(normalize("  LS  "), "ls");
        assert_eq!(normalize("Git Status"), "git status");
        assert_eq!(normalize("\t\techo\n"), "echo");
    }

    #[test]
    fn test_normalize_case_insensitive() {
        assert_eq!(normalize("GIT"), normalize("git"));
        assert_eq!(normalize("Git"), normalize("GIT"));
    }

    #[test]
    fn test_hash_command_deterministic() {
        let hash1 = hash_command("ls");
        let hash2 = hash_command("ls");
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_hash_command_different_inputs() {
        let hash1 = hash_command("ls");
        let hash2 = hash_command("ls -la");
        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_hash_command_sha1_format() {
        let hash = hash_command("test");
        assert_eq!(hash.len(), 40);
        assert!(hash.chars().all(|c: char| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_deduplicate_empty() {
        let input: Vec<AppCommand> = vec![];
        let result = deduplicate(input);
        assert!(result.is_empty());
    }

    #[test]
    fn test_deduplicate_single() {
        let cmd = make_cmd("ls", 1, vec![], false);
        let input = vec![cmd];
        let result = deduplicate(input);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_deduplicate_merges_use_count() {
        let cmd1 = make_cmd("ls", 5, vec![], false);
        let cmd2 = make_cmd("LS", 3, vec![], false);
        let input = vec![cmd1, cmd2];
        let result = deduplicate(input);
        assert_eq!(result.len(), 1);
        assert!(result[0].use_count >= 3);
        assert_eq!(result[0].text, "LS");
    }

    #[test]
    fn test_deduplicate_preserves_order() {
        let cmd1 = make_cmd("ls", 1, vec![], false);
        let cmd2 = make_cmd("pwd", 2, vec![], false);
        let cmd3 = make_cmd("git", 3, vec![], false);
        let input = vec![cmd1, cmd2, cmd3];
        let result = deduplicate(input);
        assert_eq!(result.len(), 3);
        assert_eq!(result[0].text, "ls");
        assert_eq!(result[1].text, "pwd");
        assert_eq!(result[2].text, "git");
    }

    #[test]
    fn test_deduplicate_merges_tags() {
        let cmd1 = make_cmd("ls", 1, vec!["bash".to_string()], false);
        let cmd2 = make_cmd("LS", 1, vec!["linux".to_string()], false);
        let input = vec![cmd1, cmd2];
        let result = deduplicate(input);
        assert_eq!(result.len(), 1);
        assert!(result[0].tags.len() >= 1);
        assert_eq!(result[0].text, "LS");
    }

    #[test]
    fn test_deduplicate_takes_favorite() {
        let cmd1 = make_cmd("ls", 1, vec![], false);
        let cmd2 = make_cmd("LS", 1, vec![], true);
        let input = vec![cmd1, cmd2];
        let result = deduplicate(input);
        assert_eq!(result.len(), 1);
        assert!(result[0].favorite);
    }

    #[test]
    fn test_deduplicate_takes_later_last_used() {
        let mut cmd1 = make_cmd("ls", 1, vec![], false);
        cmd1.last_used = Some(100);
        let mut cmd2 = make_cmd("LS", 1, vec![], false);
        cmd2.last_used = Some(200);
        let input = vec![cmd1, cmd2];
        let result = deduplicate(input);
        assert_eq!(result[0].last_used, Some(200));
    }

    #[test]
    fn test_deduplicate_no_false_positives() {
        let cmd1 = make_cmd("ls", 1, vec![], false);
        let cmd2 = make_cmd("ls -la", 1, vec![], false);
        let input = vec![cmd1, cmd2];
        let result = deduplicate(input);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_deduplicate_complex_command() {
        let cmd1 = make_cmd("git commit -m 'fix'", 1, vec!["vcs".to_string()], true);
        let cmd2 = make_cmd("GIT COMMIT -M 'FIX'", 2, vec!["git".to_string()], false);
        let input = vec![cmd1, cmd2];
        let result = deduplicate(input);
        assert_eq!(result.len(), 1);
        assert!(result[0].use_count >= 2);
        assert_eq!(result[0].text, "GIT COMMIT -M 'FIX'");
    }
}

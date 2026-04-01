use crate::Command;

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

pub fn load_history() -> Vec<Command> {
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
    let mut map: HashMap<String, Command> = HashMap::new();

    for cmd in commands.into_iter().rev() {
        let key = normalize(&cmd.text);

        map.entry(key)
            .and_modify(|existing| {
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
            })
            .or_insert(cmd);
    }

    let mut result: Vec<_> = map.into_values().collect();
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

use crate::Command;

mod bash;
mod fish;
mod zsh;

// think about windows support (powershell? cmd?)

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct HistoryEntry {
    pub command: String,
    pub timestamp: Option<i64>,
}

pub fn load_history() -> Vec<Command> {
    let mut commands = Vec::new();
    let mut id: u32 = 0;

    let shell = detect_shell();

    let home = dirs::home_dir().unwrap_or_default();

    match shell {
        "bash" => {
            let path = home.join(".bash_history");
            for entry in bash::read_history(&path) {
                commands.push(Command {
                    id,
                    text: entry.command,
                    tags: vec!["bash".to_string()],
                    favorite: false,
                    _context: "shell:bash".to_string(),
                    use_count: 0,
                    last_used: None,
                });
                id += 1;
            }
        }
        "zsh" => {
            let path = home.join(".zsh_history");
            for entry in zsh::read_history(&path) {
                commands.push(Command {
                    id,
                    text: entry.command,
                    tags: vec!["zsh".to_string()],
                    favorite: false,
                    _context: "shell:zsh".to_string(),
                    use_count: 0,
                    last_used: None,
                });
                id += 1;
            }
        }
        "fish" => {
            let path = home.join(".local/share/fish/fish_history");
            for entry in fish::read_history(&path) {
                commands.push(Command {
                    id,
                    text: entry.command,
                    tags: vec!["fish".to_string()],
                    favorite: false,
                    _context: "shell:fish".to_string(),
                    use_count: 0,
                    last_used: None,
                });
                id += 1;
            }
        }
        _ => {}
    }

    // last command first
    commands.reverse();
    commands
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

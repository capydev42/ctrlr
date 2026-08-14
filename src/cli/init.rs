use crate::cli::shells::{self, Shell};
use color_eyre::Report;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

pub fn run(shell: Option<Shell>, print_only: bool) -> Result<(), Report> {
    let shell = match shell {
        Some(s) => s,
        None => match Shell::detect() {
            Some(s) => s,
            None => {
                let current_shell =
                    std::env::var("SHELL").unwrap_or_else(|_| "unknown".to_string());
                println!(
                    "⚠️ Could not confidently detect shell\n\nDetected: {} (unsupported)\n\nSupported:\n  - bash\n  - zsh\n  - fish\n\nTry:\n  ctrlr init --shell bash\n  ctrlr init --print",
                    current_shell
                );
                return Ok(());
            }
        },
    };

    println!("✔ Detected shell: {}", shell);

    let config_path = shell.config_path();
    let config_content = fs::read_to_string(&config_path).unwrap_or_default();

    let is_installed = shells::is_installed(shell, &config_content);
    let is_current = shells::is_up_to_date(shell, &config_content);

    if is_installed && is_current {
        println!(
            "✔ ctrlr integration is up to date in {}",
            config_path.display()
        );
        return Ok(());
    }

    if is_installed && !is_current {
        println!("ctrlr integration found but outdated. Updating...");
        remove_integration(&config_path, &config_content)?;
    }

    let script = shells::generate_script(shell);

    if print_only {
        println!(
            "# Copy this into your shell config ({}):\n",
            config_path.display()
        );
        println!("{}", script);
        return Ok(());
    }

    println!("\nWe will add the following to {}:", config_path.display());
    println!("{}", script);

    print!("\nProceed? (y/n) ");
    std::io::stdout().flush()?;

    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    let input = input.trim().to_lowercase();

    if input != "y" && input != "yes" {
        println!("Aborted.");
        return Ok(());
    }

    install(&config_path, &script)?;

    println!("✔ Installed ctrlr integration");
    println!("→ Restart shell or run: source {}", config_path.display());

    Ok(())
}

const START_MARKER: &str = "# ctrlr integration";
const END_MARKER: &str = "# ctrlr integration end";

/// Strips the integration block from a shell config.
///
/// Blocks written by current versions are delimited by [`END_MARKER`]. Older
/// ones are not, so they are cut at their last line — the key binding — which
/// every one of the three scripts ends with. Counting a fixed number of lines,
/// as this used to, left the tail of the block behind whenever a script grew.
fn strip_integration(content: &str) -> String {
    let lines: Vec<&str> = content.lines().collect();
    let mut kept: Vec<&str> = Vec::new();
    let mut i = 0;

    while i < lines.len() {
        if !lines[i].trim_start().starts_with(START_MARKER) {
            kept.push(lines[i]);
            i += 1;
            continue;
        }

        let end = lines[i + 1..]
            .iter()
            .position(|l| l.trim_start().starts_with(END_MARKER))
            .or_else(|| {
                lines[i + 1..].iter().position(|l| {
                    let l = l.trim_start();
                    l.starts_with("bind -x") || l.starts_with("bindkey") || l.starts_with("bind \\")
                })
            });

        // No terminator at all: drop only the marker line rather than guessing
        // at a length and eating unrelated config.
        i += end.map(|e| e + 2).unwrap_or(1);
    }

    kept.join("\n")
}

fn remove_integration(config_path: &PathBuf, content: &str) -> Result<(), Report> {
    let new_content = strip_integration(content);
    fs::write(config_path, new_content).map_err(|e| {
        Report::new(std::io::Error::other(format!(
            "Failed to update config: {}",
            e
        )))
    })?;

    Ok(())
}

fn install(config_path: &PathBuf, script: &str) -> Result<(), Report> {
    let config_dir = config_path
        .parent()
        .ok_or_else(|| Report::new(std::io::Error::other("Invalid config path")))?;

    if !config_dir.exists() {
        fs::create_dir_all(config_dir).map_err(|e| {
            Report::new(std::io::Error::other(format!(
                "Failed to create config directory: {}",
                e
            )))
        })?;
    }

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(config_path)
        .map_err(|e| {
            Report::new(std::io::Error::other(format!(
                "Failed to open config file: {}",
                e
            )))
        })?;

    writeln!(file)?;
    writeln!(file, "{}", script)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const LEGACY_ZSH: &str = "# ctrlr integration
autoload -Uz add-zsh-hook
add-zsh-hook precmd _flush_zsh_history
_flush_zsh_history() { fc -W }

_ctrlr_widget() {
    local tmpfile=$(mktemp)
    ctrlr --output-file \"$tmpfile\"
    if [[ -s \"$tmpfile\" ]]; then
        BUFFER=$(cat \"$tmpfile\")
        CURSOR=$#BUFFER
    fi
    rm -f \"$tmpfile\"
}
zle -N _ctrlr_widget
bindkey '^R' _ctrlr_widget";

    #[test]
    fn test_strip_removes_block_with_end_marker() {
        let content = format!(
            "export FOO=1\n\n{}\nline\nline\n{}\n\nexport BAR=2",
            START_MARKER, END_MARKER
        );
        assert_eq!(
            strip_integration(&content),
            "export FOO=1\n\n\nexport BAR=2"
        );
    }

    #[test]
    fn test_strip_removes_legacy_zsh_block_entirely() {
        // The old block had no end marker and was longer than the fixed line
        // count the previous implementation skipped.
        let content = format!("export FOO=1\n{}\nexport BAR=2", LEGACY_ZSH);
        let stripped = strip_integration(&content);
        assert_eq!(stripped, "export FOO=1\nexport BAR=2");
        assert!(!stripped.contains("_ctrlr_widget"));
        assert!(!stripped.contains("bindkey"));
    }

    #[test]
    fn test_strip_removes_legacy_bash_block() {
        let content = "export FOO=1\n# ctrlr integration\nexport PROMPT_COMMAND=\"history -a\"\n_ctrlr_widget() {\n  :\n}\nbind -x '\"\\C-r\": _ctrlr_widget'\nexport BAR=2";
        assert_eq!(strip_integration(content), "export FOO=1\nexport BAR=2");
    }

    #[test]
    fn test_strip_removes_legacy_fish_block() {
        let content = "set -g FOO 1\n# ctrlr integration\nfunction _ctrlr_widget\nend\nbind \\cr _ctrlr_widget\nset -g BAR 2";
        assert_eq!(strip_integration(content), "set -g FOO 1\nset -g BAR 2");
    }

    #[test]
    fn test_strip_leaves_unrelated_config_alone() {
        let content = "export FOO=1\n# some other integration\nexport BAR=2";
        assert_eq!(strip_integration(content), content);
    }

    #[test]
    fn test_strip_unterminated_block_drops_only_the_marker() {
        let content = "export FOO=1\n# ctrlr integration\nexport BAR=2";
        assert_eq!(strip_integration(content), "export FOO=1\nexport BAR=2");
    }

    #[test]
    fn test_strip_removes_every_block() {
        let content = format!(
            "a\n{}\nx\n{}\nb\n{}\ny\n{}\nc",
            START_MARKER, END_MARKER, START_MARKER, END_MARKER
        );
        assert_eq!(strip_integration(&content), "a\nb\nc");
    }
}

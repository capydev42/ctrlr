use crate::cli::shells::{self, Shell};
use color_eyre::Report;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

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

    // Stripping the old block used to happen before this prompt, so answering
    // "n" left the config with no integration at all.
    let outcome = install_integration(shell)?;

    println!("✔ Installed ctrlr integration");
    if let Some(backup) = &outcome.backup {
        println!("→ Previous config saved to {}", backup.display());
    }
    println!("→ Restart shell or run: source {}", config_path.display());

    Ok(())
}

/// Where the install wrote, and what it saved first.
pub struct InstallOutcome {
    pub config_path: PathBuf,
    pub backup: Option<PathBuf>,
}

/// Replaces the integration block in the user's shell config.
///
/// No prompting and no printing: shared by `ctrlr init` and the in-TUI update
/// popup. The existing config is copied aside first — this rewrites the file
/// that decides whether the user's next shell starts correctly — and the strip
/// and the append land in one write, so a failure cannot leave the config with
/// the old block removed and nothing in its place.
pub fn install_integration(shell: Shell) -> Result<InstallOutcome, Report> {
    let config_path = shell.config_path();
    let content = fs::read_to_string(&config_path).unwrap_or_default();

    let backup = if content.is_empty() {
        None
    } else {
        let backup_path = backup_path(&config_path);
        fs::write(&backup_path, &content).map_err(|e| {
            Report::new(std::io::Error::other(format!(
                "Failed to back up {}: {}",
                config_path.display(),
                e
            )))
        })?;
        Some(backup_path)
    };

    let mut new_content = strip_integration(&content);
    while new_content.ends_with('\n') {
        new_content.pop();
    }
    if !new_content.is_empty() {
        new_content.push('\n');
    }
    new_content.push('\n');
    new_content.push_str(&shells::generate_script(shell));
    if !new_content.ends_with('\n') {
        new_content.push('\n');
    }

    if let Some(dir) = config_path.parent()
        && !dir.exists()
    {
        fs::create_dir_all(dir).map_err(|e| {
            Report::new(std::io::Error::other(format!(
                "Failed to create config directory: {}",
                e
            )))
        })?;
    }

    fs::write(&config_path, new_content).map_err(|e| {
        Report::new(std::io::Error::other(format!(
            "Failed to update config: {}",
            e
        )))
    })?;

    Ok(InstallOutcome {
        config_path,
        backup,
    })
}

/// `.bashrc` has no extension to replace, so the suffix is appended: using
/// `with_extension` would turn `~/.bashrc` into `~/.bak`.
fn backup_path(config_path: &Path) -> PathBuf {
    let mut name = config_path.as_os_str().to_os_string();
    name.push(".ctrlr.bak");
    PathBuf::from(name)
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

#[cfg(test)]
mod install_tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    /// `install_integration` resolves the config path from the environment, so
    /// these drive the pieces it is built from instead.
    fn install_into(config: &Path, shell: Shell) -> String {
        let content = fs::read_to_string(config).unwrap_or_default();
        let mut new_content = strip_integration(&content);
        while new_content.ends_with('\n') {
            new_content.pop();
        }
        if !new_content.is_empty() {
            new_content.push('\n');
        }
        new_content.push('\n');
        new_content.push_str(&shells::generate_script(shell));
        new_content.push('\n');
        fs::write(config, &new_content).unwrap();
        new_content
    }

    #[test]
    fn test_backup_path_appends_rather_than_replacing() {
        // with_extension would turn ~/.bashrc into ~/.bak.
        let path = backup_path(Path::new("/home/u/.bashrc"));
        assert_eq!(path, PathBuf::from("/home/u/.bashrc.ctrlr.bak"));

        let fish = backup_path(Path::new("/home/u/.config/fish/config.fish"));
        assert_eq!(
            fish,
            PathBuf::from("/home/u/.config/fish/config.fish.ctrlr.bak")
        );
    }

    #[test]
    fn test_reinstall_does_not_stack_blocks() {
        let dir = TempDir::new().unwrap();
        let config = dir.path().join(".bashrc");
        fs::write(&config, "export FOO=1\n").unwrap();

        install_into(&config, Shell::Bash);
        let twice = install_into(&config, Shell::Bash);

        assert_eq!(
            twice.matches("# ctrlr integration\n").count(),
            1,
            "a second install replaces the block instead of appending another"
        );
        assert!(twice.contains("export FOO=1"), "user config survives");
        assert_eq!(
            shells::integration_state(Shell::Bash, &twice),
            shells::IntegrationState::Current
        );
    }

    #[test]
    fn test_install_over_a_legacy_block_leaves_nothing_behind() {
        let dir = TempDir::new().unwrap();
        let config = dir.path().join(".bashrc");
        fs::write(
            &config,
            "export FOO=1\n# ctrlr integration\nexport PROMPT_COMMAND=\"history -a\"\nbind -x '\"\\C-r\": _ctrlr_widget'\nexport BAR=2\n",
        )
        .unwrap();

        let after = install_into(&config, Shell::Bash);

        assert!(
            !after.contains("export PROMPT_COMMAND"),
            "old block is gone"
        );
        assert!(after.contains("export FOO=1") && after.contains("export BAR=2"));
        assert_eq!(
            shells::integration_state(Shell::Bash, &after),
            shells::IntegrationState::Current
        );
    }
}

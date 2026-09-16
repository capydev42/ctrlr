pub mod bash;
pub mod fish;
pub mod zsh;

use std::fmt;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shell {
    Bash,
    Zsh,
    Fish,
}

impl Shell {
    /// Every variant, so the help text, the "Supported:" list and the
    /// per-shell tests cannot fall behind a new one. Hand-maintained; the
    /// test below is what keeps it honest.
    pub const ALL: &'static [Shell] = &[Shell::Bash, Shell::Zsh, Shell::Fish];

    /// What `--shell` accepts, and what `detect` matches a `$SHELL` basename
    /// against.
    pub fn aliases(&self) -> &'static [&'static str] {
        match self {
            Shell::Bash => &["bash"],
            Shell::Zsh => &["zsh"],
            Shell::Fish => &["fish"],
        }
    }

    pub fn detect() -> Option<Self> {
        let shell = std::env::var("SHELL").ok()?;
        let basename = std::path::Path::new(&shell).file_name()?.to_str()?;
        Self::from_str(basename)
    }

    pub fn from_str(s: &str) -> Option<Self> {
        let s = s.to_lowercase();
        Self::ALL
            .iter()
            .copied()
            .find(|shell| shell.aliases().contains(&s.as_str()))
    }

    pub fn config_path(&self) -> std::path::PathBuf {
        match self {
            Shell::Bash => dirs::home_dir()
                .map(|p| p.join(".bashrc"))
                .unwrap_or_else(|| std::path::PathBuf::from(".bashrc")),
            Shell::Zsh => dirs::home_dir()
                .map(|p| p.join(".zshrc"))
                .unwrap_or_else(|| std::path::PathBuf::from(".zshrc")),
            Shell::Fish => dirs::home_dir()
                .map(|p| p.join(".config/fish/config.fish"))
                .unwrap_or_else(|| std::path::PathBuf::from(".config/fish/config.fish")),
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Shell::Bash => "bash",
            Shell::Zsh => "zsh",
            Shell::Fish => "fish",
        }
    }
}

impl fmt::Display for Shell {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

/// What the shell config currently holds, from ctrlr's side.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntegrationState {
    Missing,
    Outdated,
    Current,
}

pub fn integration_state(shell: Shell, config_content: &str) -> IntegrationState {
    if !is_installed(shell, config_content) {
        IntegrationState::Missing
    } else if !is_up_to_date(shell, config_content) {
        IntegrationState::Outdated
    } else {
        IntegrationState::Current
    }
}

/// `Ok(None)` is a missing file, a normal first install. An unreadable one is
/// an error and never an empty string: that conflation is how the install came
/// to overwrite a config it had never read.
pub fn read_config(path: &Path) -> std::io::Result<Option<String>> {
    match std::fs::read_to_string(path) {
        Ok(content) => Ok(Some(content)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}

/// `None` when the shell is unsupported or its config cannot be read. A config
/// that does not exist yet reports `Missing` instead, so the offer still
/// reaches users with no rc file.
pub fn detect_integration_state() -> Option<(Shell, IntegrationState)> {
    let shell = Shell::detect()?;
    let content = read_config(&shell.config_path()).ok()?.unwrap_or_default();
    Some((shell, integration_state(shell, &content)))
}

/// The command that replaces the running shell with a fresh one, so a
/// just-installed integration takes effect. ctrlr cannot source anything into
/// its parent itself: it is a child process, and whatever it sources dies with
/// it.
pub fn reload_command(shell: Shell) -> &'static str {
    match shell {
        Shell::Bash => "exec bash",
        Shell::Zsh => "exec zsh",
        Shell::Fish => "exec fish",
    }
}

/// Identifies the exact script a config was last offered, so a dismissal lasts
/// until the integration actually changes.
pub fn script_fingerprint(shell: Shell) -> String {
    crate::hash::sha1_hex(&generate_script(shell))
}

/// Opens and closes every block ctrlr writes into a shell config. `strip`,
/// `is_installed` and each script constant all read the same two strings.
pub const START_MARKER: &str = "# ctrlr integration";
pub const END_MARKER: &str = "# ctrlr integration end";

fn script_template(shell: Shell) -> &'static str {
    match shell {
        Shell::Bash => bash::SCRIPT,
        Shell::Zsh => zsh::SCRIPT,
        Shell::Fish => fish::SCRIPT,
    }
}

pub fn generate_script(shell: Shell) -> String {
    script_template(shell).replace("{LOG}", &crate::storage::runs_log_path().to_string_lossy())
}

/// The shell is irrelevant today - every script opens with the same marker -
/// but a shell whose comment character is not `#` would need it.
pub fn is_installed(_shell: Shell, config_content: &str) -> bool {
    config_content.contains(START_MARKER)
}

pub fn is_up_to_date(shell: Shell, config_content: &str) -> bool {
    config_content.contains(&generate_script(shell))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_read_config_returns_the_contents() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join(".bashrc");
        std::fs::write(&path, "export FOO=1\n").unwrap();

        assert_eq!(
            read_config(&path).unwrap(),
            Some("export FOO=1\n".to_string())
        );
    }

    #[test]
    fn test_read_config_reports_a_missing_file_as_none() {
        let dir = TempDir::new().unwrap();
        assert_eq!(read_config(&dir.path().join("nope")).unwrap(), None);
    }

    /// The distinction the function exists for: unreadable must not read back
    /// as empty, or the install writes over it.
    #[test]
    fn test_read_config_errors_on_an_undecodable_file() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join(".bashrc");
        std::fs::write(&path, [0xff, 0xfe, 0x00, b'b']).unwrap();

        assert!(read_config(&path).is_err());
    }

    /// A missing config still gets the offer.
    #[test]
    fn test_an_empty_config_reports_missing() {
        for &shell in &[Shell::Bash, Shell::Zsh, Shell::Fish] {
            assert_eq!(
                integration_state(shell, ""),
                IntegrationState::Missing,
                "{} with no config yet still gets the offer",
                shell
            );
        }
    }

    /// `ALL` is hand-maintained. The match makes adding a variant without
    /// listing it a compile error rather than a silently untested shell.
    #[test]
    fn test_all_lists_every_variant() {
        fn exhaustive(shell: Shell) -> usize {
            match shell {
                Shell::Bash => 0,
                Shell::Zsh => 1,
                Shell::Fish => 2,
            }
        }
        assert_eq!(Shell::ALL.len(), 3);
        for (i, &shell) in Shell::ALL.iter().enumerate() {
            assert_eq!(exhaustive(shell), i, "{} is out of order in ALL", shell);
        }
    }

    #[test]
    fn test_from_str_round_trips_every_display_name() {
        for &shell in Shell::ALL {
            assert_eq!(Shell::from_str(shell.display_name()), Some(shell));
            assert_eq!(
                Shell::from_str(&shell.display_name().to_uppercase()),
                Some(shell)
            );
        }
        assert_eq!(Shell::from_str("nonesuch"), None);
    }

    #[test]
    fn test_is_installed_and_up_to_date_per_shell() {
        for &shell in Shell::ALL {
            let script = generate_script(shell);
            assert!(is_installed(shell, &script));
            assert!(is_up_to_date(shell, &script));
            assert!(!is_installed(shell, "# other integration\nfoo"));
            assert!(!is_up_to_date(shell, "other stuff"));
        }
    }

    /// Both markers are what `strip_integration` cuts on, so every script has
    /// to carry them verbatim.
    #[test]
    fn test_every_script_carries_both_markers() {
        for &shell in Shell::ALL {
            let script = generate_script(shell);
            assert!(script.starts_with(START_MARKER), "{} start marker", shell);
            assert!(
                script.trim_end().ends_with(END_MARKER),
                "{} end marker",
                shell
            );
            assert!(
                !script.contains("{LOG}"),
                "{} still has the placeholder",
                shell
            );
        }
    }
}

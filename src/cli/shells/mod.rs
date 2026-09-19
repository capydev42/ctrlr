pub mod bash;
pub mod fish;
pub mod powershell;
pub mod zsh;

use std::fmt;
use std::path::Path;

// `PowerShell` tripping enum_variant_names is the product's name, not a
// stutter; `Pwsh` would read as excluding Windows PowerShell 5.1.
#[allow(clippy::enum_variant_names)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shell {
    Bash,
    Zsh,
    Fish,
    PowerShell,
}

impl Shell {
    /// Every variant, so the help text, the "Supported:" list and the
    /// per-shell tests cannot fall behind a new one. Hand-maintained; the
    /// test below is what keeps it honest.
    pub const ALL: &'static [Shell] = &[Shell::Bash, Shell::Zsh, Shell::Fish, Shell::PowerShell];

    /// What `--shell` accepts, and what `detect` matches a `$SHELL` basename
    /// against.
    pub fn aliases(&self) -> &'static [&'static str] {
        match self {
            Shell::Bash => &["bash"],
            Shell::Zsh => &["zsh"],
            Shell::Fish => &["fish"],
            Shell::PowerShell => &["powershell", "pwsh", "ps"],
        }
    }

    /// Where this shell keeps the history file ctrlr parses.
    pub fn history_path(&self) -> Option<std::path::PathBuf> {
        let home = dirs::home_dir()?;
        Some(match self {
            Shell::Bash => home.join(".bash_history"),
            Shell::Zsh => home.join(".zsh_history"),
            // fish uses the XDG layout on macOS too, so not `dirs::data_dir()`.
            Shell::Fish => home.join(".local/share/fish/fish_history"),
            Shell::PowerShell => {
                if let Some(path) = std::env::var_os("CTRLR_POWERSHELL_HISTORY") {
                    return Some(std::path::PathBuf::from(path));
                }
                // PSReadLine, not PowerShell itself. Windows keeps it in
                // Roaming and shares it between 5.1 and 7; unix uses the XDG
                // layout on macOS too. `(Get-PSReadLineOption).HistorySavePath`
                // is what to check when this is wrong.
                if cfg!(windows) {
                    dirs::data_dir()?
                        .join("Microsoft")
                        .join("Windows")
                        .join("PowerShell")
                        .join("PSReadLine")
                        .join("ConsoleHost_history.txt")
                } else {
                    home.join(".local/share/powershell/PSReadLine/ConsoleHost_history.txt")
                }
            }
        })
    }

    /// How to make the shell write its in-memory history to that file.
    pub fn flush_argv(&self) -> Option<(&'static str, &'static str)> {
        Some(match self {
            Shell::Bash => ("bash", "history -a"),
            Shell::Zsh => ("zsh", "fc -W"),
            Shell::Fish => ("fish", "history save"),
            // PSReadLine defaults HistorySaveStyle to SaveIncrementally, so
            // the file is already current.
            Shell::PowerShell => return None,
        })
    }

    /// Which shell ctrlr is running under.
    ///
    /// `$SHELL` names the *login* shell, which is the wrong question: pwsh
    /// leaves it pointing at bash. On unix `PSModulePath` is set only by
    /// pwsh, so it answers the right one and is checked first. On Windows it
    /// is machine-wide and says nothing, but PowerShell is the only supported
    /// shell there anyway.
    ///
    /// Known miss: `pwsh` -> `bash` -> ctrlr inherits `PSModulePath` and
    /// reports PowerShell. `CTRLR_SHELL` and `--shell` both override.
    pub fn detect() -> Option<Self> {
        Self::detect_from(
            std::env::var("CTRLR_SHELL").ok().as_deref(),
            std::env::var_os("PSModulePath").is_some(),
            std::env::var("SHELL").ok().as_deref(),
        )
    }

    /// The rules, without the environment, so they can be tested.
    fn detect_from(
        ctrlr_shell: Option<&str>,
        ps_module_path: bool,
        shell: Option<&str>,
    ) -> Option<Self> {
        if let Some(name) = ctrlr_shell {
            return Self::from_str(name);
        }
        // Only pwsh sets this on unix. On Windows it is machine-wide and says
        // nothing, but PowerShell is the only supported shell there anyway.
        if ps_module_path && !cfg!(windows) {
            return Some(Shell::PowerShell);
        }
        match shell {
            // `$SHELL` names the login shell, which is why it is checked last:
            // pwsh leaves it pointing at bash.
            Some(shell) => {
                let basename = std::path::Path::new(shell).file_name()?.to_str()?;
                Self::from_str(basename)
            }
            // Windows PowerShell leaves it unset.
            None => cfg!(windows).then_some(Shell::PowerShell),
        }
    }

    /// `detect` with a platform fallback, for loading history: a shell ctrlr
    /// cannot name would otherwise mean no commands at all. `detect` itself
    /// stays strict, because writing into the wrong config is worse than not
    /// offering to.
    pub fn detect_or_default() -> Self {
        Self::detect().unwrap_or(if cfg!(windows) {
            Shell::PowerShell
        } else {
            Shell::Bash
        })
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
            Shell::PowerShell => powershell_profile(),
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Shell::Bash => "bash",
            Shell::Zsh => "zsh",
            Shell::Fish => "fish",
            Shell::PowerShell => "powershell",
        }
    }
}

/// `$PROFILE.CurrentUserCurrentHost`, computed rather than asked for.
///
/// Asking pwsh would be exact, but `config_path` runs on every launch and a
/// pwsh startup ahead of the first frame is not affordable. `ctrlr init`
/// prints the path it picked and asks before writing, and
/// `CTRLR_POWERSHELL_PROFILE` is the way out when the guess is wrong.
fn powershell_profile() -> std::path::PathBuf {
    match std::env::var_os("CTRLR_POWERSHELL_PROFILE") {
        Some(path) => std::path::PathBuf::from(path),
        None => default_powershell_profile(),
    }
}

fn default_powershell_profile() -> std::path::PathBuf {
    const PROFILE: &str = "Microsoft.PowerShell_profile.ps1";

    if cfg!(windows) {
        // Through the known folder, not home.join("Documents"): that is what
        // follows OneDrive and Group Policy redirection.
        let documents = dirs::document_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
        let dir = if has_pwsh7() {
            "PowerShell"
        } else {
            "WindowsPowerShell"
        };
        documents.join(dir).join(PROFILE)
    } else {
        // Not `dirs::config_dir()`: on macOS that is ~/Library/Application
        // Support, while pwsh uses ~/.config there like everywhere else.
        let home = dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
        home.join(".config").join("powershell").join(PROFILE)
    }
}

/// PowerShell 7 and Windows PowerShell 5.1 keep separate profiles.
fn has_pwsh7() -> bool {
    std::env::var_os("PATH")
        .map(|path| std::env::split_paths(&path).any(|dir| dir.join("pwsh.exe").is_file()))
        .unwrap_or(false)
        || std::env::var_os("ProgramFiles")
            .map(|p| {
                std::path::Path::new(&p)
                    .join("PowerShell")
                    .join("7")
                    .join("pwsh.exe")
                    .is_file()
            })
            .unwrap_or(false)
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
        // Dot-sourcing works here: the widget puts this on the prompt line,
        // so it runs *in* the shell rather than in a child.
        Shell::PowerShell => ". $PROFILE",
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
        Shell::PowerShell => powershell::SCRIPT,
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
                Shell::PowerShell => 3,
            }
        }
        assert_eq!(Shell::ALL.len(), 4);
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

    #[test]
    fn test_every_shell_has_a_history_path() {
        for &shell in Shell::ALL {
            let path = shell.history_path().expect("home is set under test");
            assert!(
                path.is_absolute(),
                "{} history path is relative: {}",
                shell,
                path.display()
            );
        }
    }

    /// PowerShell is the exception: PSReadLine saves incrementally, so there
    /// is nothing to flush and nothing to spawn.
    #[test]
    fn test_flush_argv_names_the_shell_itself() {
        for &shell in Shell::ALL {
            match shell.flush_argv() {
                Some((program, _)) => assert_eq!(program, shell.display_name()),
                None => assert_eq!(shell, Shell::PowerShell),
            }
        }
    }

    #[test]
    fn test_ctrlr_shell_overrides_everything_else() {
        for &shell in Shell::ALL {
            let name = shell.display_name();
            assert_eq!(
                Shell::detect_from(Some(name), true, Some("/bin/zsh")),
                Some(shell)
            );
        }
        // An unusable override is not a silent fallback to $SHELL.
        assert_eq!(
            Shell::detect_from(Some("nonesuch"), false, Some("/bin/zsh")),
            None
        );
    }

    /// The case `$SHELL` gets wrong: pwsh leaves it naming the login shell.
    #[test]
    fn test_ps_module_path_wins_over_shell_on_unix() {
        let detected = Shell::detect_from(None, true, Some("/bin/bash"));
        if cfg!(windows) {
            assert_eq!(detected, Some(Shell::Bash), "machine-wide on Windows");
        } else {
            assert_eq!(detected, Some(Shell::PowerShell));
        }
    }

    #[test]
    fn test_shell_basename_is_used_when_nothing_else_applies() {
        assert_eq!(
            Shell::detect_from(None, false, Some("/bin/zsh")),
            Some(Shell::Zsh)
        );
        assert_eq!(
            Shell::detect_from(None, false, Some("/usr/bin/fish")),
            Some(Shell::Fish)
        );
        assert_eq!(Shell::detect_from(None, false, Some("/bin/nonesuch")), None);
    }

    /// Unset `$SHELL` means PowerShell on Windows and nothing elsewhere.
    #[test]
    fn test_unset_shell() {
        let expected = cfg!(windows).then_some(Shell::PowerShell);
        assert_eq!(Shell::detect_from(None, false, None), expected);
    }

    #[test]
    fn test_powershell_profile_is_named_for_the_current_user_host() {
        // Not asserting absoluteness: both branches fall back to "." when the
        // base directory cannot be resolved, which says nothing about the
        // logic here.
        let path = default_powershell_profile();
        assert!(path.ends_with("Microsoft.PowerShell_profile.ps1"));
    }
}

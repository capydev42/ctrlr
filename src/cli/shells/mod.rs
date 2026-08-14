pub mod bash;
pub mod fish;
pub mod zsh;

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shell {
    Bash,
    Zsh,
    Fish,
}

impl Shell {
    pub fn detect() -> Option<Self> {
        let shell = std::env::var("SHELL").ok()?;
        let basename = std::path::Path::new(&shell).file_name()?.to_str()?;
        match basename {
            "bash" => Some(Shell::Bash),
            "zsh" => Some(Shell::Zsh),
            "fish" => Some(Shell::Fish),
            _ => None,
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "bash" => Some(Shell::Bash),
            "zsh" => Some(Shell::Zsh),
            "fish" => Some(Shell::Fish),
            _ => None,
        }
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

/// Reads the detected shell's config and reports what is installed.
///
/// `None` when the shell is unsupported or its config cannot be read — there is
/// nothing to offer in either case.
pub fn detect_integration_state() -> Option<(Shell, IntegrationState)> {
    let shell = Shell::detect()?;
    let content = std::fs::read_to_string(shell.config_path()).ok()?;
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

pub fn generate_script(shell: Shell) -> String {
    match shell {
        Shell::Bash => bash::generate(),
        Shell::Zsh => zsh::generate(),
        Shell::Fish => fish::generate(),
    }
}

pub fn is_installed(shell: Shell, config_content: &str) -> bool {
    match shell {
        Shell::Bash => bash::is_installed(config_content),
        Shell::Zsh => zsh::is_installed(config_content),
        Shell::Fish => fish::is_installed(config_content),
    }
}

pub fn is_up_to_date(shell: Shell, config_content: &str) -> bool {
    match shell {
        Shell::Bash => bash::is_up_to_date(config_content),
        Shell::Zsh => zsh::is_up_to_date(config_content),
        Shell::Fish => fish::is_up_to_date(config_content),
    }
}

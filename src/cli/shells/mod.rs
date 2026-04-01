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

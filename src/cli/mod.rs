pub mod export;
pub mod import;
pub mod init;
pub mod shells;

use crate::cli::shells::Shell;
use std::fmt;

pub fn run() -> color_eyre::Result<()> {
    let args: Vec<String> = std::env::args().collect();

    // Resolved against the subcommand, not globally: `args.iter().any(...)`
    // used to answer `ctrlr init --help` with the general help, which left
    // every subcommand help unreachable.
    if args.iter().any(|a| a == "--help" || a == "-h") {
        print!("{}", help_text_for(subcommand(&args)));
        return Ok(());
    }

    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("{}", version_line());
        return Ok(());
    }

    match subcommand(&args) {
        Some("init") => {
            let shell = match get_shell_flag(&args) {
                Ok(shell) => shell,
                Err(e) => {
                    eprintln!("{}", e);
                    std::process::exit(1);
                }
            };
            let print_only = args.iter().any(|a| a == "--print");
            crate::cli::init::run(shell, print_only)?;
        }
        Some("config") => {
            if args.iter().any(|a| a == "--print") {
                print!("{}", crate::config::print_defaults());
            } else {
                match crate::config::config_path() {
                    Some(path) => println!("{}", path.display()),
                    None => eprintln!("Could not determine a config directory"),
                }
            }
        }
        Some("export") => {
            let output_path = get_export_output_path(&args);
            crate::cli::export::run(output_path.as_deref())?;
        }
        Some("import") => {
            let input_path = get_import_input_path(&args);
            if input_path.is_none() {
                eprintln!("Error: import requires a file path");
                print!("{}", import_help_text());
                std::process::exit(1);
            }
            let input_path = input_path.unwrap();
            let merge = args.iter().any(|a| a == "--merge");
            let replace = args.iter().any(|a| a == "--replace");
            let dry_run = args.iter().any(|a| a == "--dry-run");
            crate::cli::import::run(&input_path, merge, replace, dry_run)?;
        }
        _ => {
            let output_file = get_output_file_flag(&args);
            check_integration_warning();
            crate::run_tui(output_file)?;
        }
    }

    Ok(())
}

/// The subcommand, if there is one. A leading flag is not one, so
/// `ctrlr --help` and `ctrlr -o /tmp/cmd` still mean the TUI.
fn subcommand(args: &[String]) -> Option<&str> {
    args.get(1)
        .filter(|a| !a.starts_with('-'))
        .map(|s| s.as_str())
}

/// Answered before anything can reach the TUI: a package manager verifies an
/// install by running the binary headless (Homebrew's `test do` block), and
/// bare `ctrlr` would try to take the alternate screen.
fn version_line() -> String {
    format!("ctrlr {}", env!("CARGO_PKG_VERSION"))
}

/// `--shell` with no name, or a name ctrlr does not know. Both are kept apart
/// from "not given", which means auto-detect: silently dropping an unknown
/// name reported as a bug in the detection instead.
#[derive(Debug, PartialEq, Eq)]
enum ShellFlagError {
    Missing,
    Unknown(String),
}

impl fmt::Display for ShellFlagError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing => write!(f, "Error: --shell needs a shell name")?,
            Self::Unknown(name) => write!(f, "Error: unknown shell: {}", name)?,
        }
        write!(f, "\n\nSupported:\n{}", shells::supported_list())
    }
}

fn get_shell_flag(args: &[String]) -> Result<Option<Shell>, ShellFlagError> {
    let Some(i) = args.iter().position(|a| a == "--shell") else {
        return Ok(None);
    };
    let name = args.get(i + 1).ok_or(ShellFlagError::Missing)?;
    Shell::from_str(name)
        .map(Some)
        .ok_or_else(|| ShellFlagError::Unknown(name.clone()))
}

fn get_output_file_flag(args: &[String]) -> Option<String> {
    args.iter()
        .position(|a| a == "--output-file" || a == "-o")
        .and_then(|i| args.get(i + 1))
        .map(|s| s.to_string())
}

fn get_export_output_path(args: &[String]) -> Option<String> {
    args.get(2)
        .filter(|s| !s.starts_with('-'))
        .map(|s| s.to_string())
}

fn get_import_input_path(args: &[String]) -> Option<String> {
    args.get(2)
        .filter(|s| !s.starts_with('-'))
        .map(|s| s.to_string())
}

/// Printed before the TUI takes the alternate screen, so it is only really
/// visible on the way out. The in-TUI popup is what the user actually sees;
/// this stays for the case where ctrlr's output is being read directly.
fn check_integration_warning() {
    let state = shells::detect_integration_state().map(|(_, state)| state);

    match state {
        Some(shells::IntegrationState::Missing) => {
            println!();
            println!("⚡ ctrlr shell integration not found");
            println!();
            println!("Run:");
            println!("    ctrlr init");
            println!();
            println!("to enable keybindings (Ctrl+R)");
            println!();
        }
        // Without this an upgraded ctrlr looks broken: the old block still
        // binds Ctrl+R, so nothing seems wrong, while the features it has no
        // hooks for stay silently empty.
        Some(shells::IntegrationState::Outdated) => {
            println!();
            println!("⚡ ctrlr shell integration is out of date");
            println!();
            println!("Run:");
            println!("    ctrlr init");
            println!();
            println!("then restart your shell, or newer features may not work");
            println!();
        }
        _ => {}
    }
}

/// Every help text is a `String` rather than a `println!` block so the tests
/// can read them; an unreachable one is exactly what this change fixes.
fn help_text_for(subcommand: Option<&str>) -> String {
    match subcommand {
        Some("init") => init_help_text(),
        Some("config") => config_help_text(),
        Some("export") => export_help_text(),
        Some("import") => import_help_text(),
        _ => help_text(),
    }
}

fn help_text() -> String {
    "\
ctrlr - Command history picker

Usage: ctrlr [COMMAND]

Commands:
  init              Add shell integration
  config            Print the config file path (--print dumps the defaults)
  export [FILE]     Export data to JSON (stdout if no file)
  import FILE       Import data from JSON

Options:
  --help, -h        Show this help
  --version, -V     Show the version
  --output-file, -o Write the selected command to this file. The shell
                    integration sets it; without it nothing is printed.

Examples:
  ctrlr             Open the TUI
  ctrlr init        Add shell integration (Ctrl+R)
  ctrlr init --print   Print integration script
  ctrlr config --print > ~/.config/ctrlr/config.toml   Customise keybindings
  ctrlr --output-file /tmp/cmd  Write output to file
  ctrlr export      Export all data to stdout
  ctrlr export backup.json  Export to file
  ctrlr import backup.json  Import (merge mode)
  ctrlr import backup.json --dry-run  Preview import
  ctrlr import backup.json --replace  Replace all data
"
    .to_string()
}

fn init_help_text() -> String {
    let names: Vec<&str> = Shell::ALL.iter().map(|s| s.display_name()).collect();
    format!(
        "\
ctrlr init - Add shell integration

Usage: ctrlr init [OPTIONS]

Options:
  --shell <SHELL>   Force a specific shell ({})
  --print           Only print the integration script, don't install
  --help, -h        Show this help
",
        names.join(", ")
    )
}

fn config_help_text() -> String {
    "\
ctrlr config - Show the config file path

Usage: ctrlr config [OPTIONS]

Options:
  --print           Dump the default keymap as TOML instead of the path
  --help, -h        Show this help

Examples:
  ctrlr config      Print where the config file is read from
  ctrlr config --print > ~/.config/ctrlr/config.toml   Start from the defaults
"
    .to_string()
}

fn export_help_text() -> String {
    "\
ctrlr export - Export data to JSON

Usage: ctrlr export [FILE]

Without a file the JSON goes to stdout.

Options:
  --help, -h        Show this help

Examples:
  ctrlr export      Export all data to stdout
  ctrlr export backup.json  Export to file
"
    .to_string()
}

fn import_help_text() -> String {
    "\
ctrlr import - Import data from JSON

Usage: ctrlr import FILE [OPTIONS]

Options:
  --merge           Merge with existing data (default)
  --replace         Replace all existing data
  --dry-run         Preview changes without applying
  --help, -h        Show this help
"
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_line_is_name_and_semver() {
        let line = version_line();
        let (name, version) = line.split_once(' ').expect("name and version");
        assert_eq!(name, "ctrlr");
        assert_eq!(
            version.split('.').count(),
            3,
            "expected a semver triple, got {version}"
        );
    }

    const LEGACY_BASH: &str = "# ctrlr integration
export PROMPT_COMMAND=\"${PROMPT_COMMAND:+$PROMPT_COMMAND; } history -a\"
bind -x '\"\\C-r\": _ctrlr_widget'";

    #[test]
    fn test_integration_state_missing() {
        assert_eq!(
            shells::integration_state(Shell::Bash, "export FOO=1"),
            shells::IntegrationState::Missing
        );
    }

    #[test]
    fn test_integration_state_outdated() {
        // The pre-run-log block still binds Ctrl+R, so nothing looks broken
        // while the run log is never written.
        assert_eq!(
            shells::integration_state(Shell::Bash, LEGACY_BASH),
            shells::IntegrationState::Outdated
        );
    }

    #[test]
    fn test_integration_state_current() {
        let installed = shells::generate_script(Shell::Bash);
        assert_eq!(
            shells::integration_state(Shell::Bash, &installed),
            shells::IntegrationState::Current
        );
    }

    #[test]
    fn test_integration_state_per_shell() {
        for &shell in Shell::ALL {
            let installed = shells::generate_script(shell);
            assert_eq!(
                shells::integration_state(shell, &installed),
                shells::IntegrationState::Current,
                "{} reports its own script as current",
                shell
            );
        }
    }

    /// Builds an argv the way `std::env::args()` hands one over, program name
    /// included.
    fn argv(args: &[&str]) -> Vec<String> {
        args.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn test_subcommand_ignores_leading_flags() {
        assert_eq!(subcommand(&argv(&["ctrlr"])), None);
        assert_eq!(subcommand(&argv(&["ctrlr", "--help"])), None);
        assert_eq!(subcommand(&argv(&["ctrlr", "-o", "/tmp/cmd"])), None);
        assert_eq!(
            subcommand(&argv(&["ctrlr", "init", "--help"])),
            Some("init")
        );
    }

    #[test]
    fn test_help_follows_the_subcommand() {
        for sub in ["init", "config", "export", "import"] {
            let text = help_text_for(Some(sub));
            assert!(
                text.starts_with(&format!("ctrlr {} - ", sub)),
                "{} --help answered with:\n{}",
                sub,
                text
            );
        }
        assert_eq!(help_text_for(None), help_text());
        assert_eq!(help_text_for(Some("nonesuch")), help_text());
    }

    /// The only place `--shell` is documented, and it was unreachable once.
    #[test]
    fn test_init_help_lists_every_shell() {
        let text = init_help_text();
        for &shell in Shell::ALL {
            assert!(
                text.contains(shell.display_name()),
                "{} is missing from the init help",
                shell
            );
        }
    }

    #[test]
    fn test_shell_flag_distinguishes_absent_unknown_and_missing() {
        assert_eq!(get_shell_flag(&argv(&["ctrlr", "init"])), Ok(None));
        assert_eq!(
            get_shell_flag(&argv(&["ctrlr", "init", "--shell", "powershell"])),
            Ok(Some(Shell::PowerShell))
        );
        assert_eq!(
            get_shell_flag(&argv(&["ctrlr", "init", "--shell", "quatsch"])),
            Err(ShellFlagError::Unknown("quatsch".to_string()))
        );
        assert_eq!(
            get_shell_flag(&argv(&["ctrlr", "init", "--shell"])),
            Err(ShellFlagError::Missing)
        );
    }

    #[test]
    fn test_shell_flag_error_lists_the_supported_shells() {
        let message = ShellFlagError::Unknown("quatsch".to_string()).to_string();
        for &shell in Shell::ALL {
            assert!(
                message.contains(shell.display_name()),
                "{} is missing from the error",
                shell
            );
        }
    }
}

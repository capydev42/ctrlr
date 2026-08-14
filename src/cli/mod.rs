pub mod export;
pub mod import;
pub mod init;
pub mod shells;

use crate::cli::shells::Shell;

pub fn run() -> color_eyre::Result<()> {
    let args: Vec<String> = std::env::args().collect();

    if args.iter().any(|a| a == "--help" || a == "-h") {
        print_help();
        return Ok(());
    }

    if args.len() > 1 && args[1] == "init" {
        if args.iter().any(|a| a == "--help" || a == "-h") {
            print_init_help();
            return Ok(());
        }
        let shell = get_shell_flag(&args);
        let print_only = args.iter().any(|a| a == "--print");
        crate::cli::init::run(shell, print_only)?;
    } else if args.len() > 1 && args[1] == "export" {
        let output_path = get_export_output_path(&args);
        crate::cli::export::run(output_path.as_deref())?;
    } else if args.len() > 1 && args[1] == "import" {
        if args.iter().any(|a| a == "--help" || a == "-h") {
            print_import_help();
            return Ok(());
        }
        let input_path = get_import_input_path(&args);
        if input_path.is_none() {
            eprintln!("Error: import requires a file path");
            print_import_help();
            std::process::exit(1);
        }
        let input_path = input_path.unwrap();
        let merge = args.iter().any(|a| a == "--merge");
        let replace = args.iter().any(|a| a == "--replace");
        let dry_run = args.iter().any(|a| a == "--dry-run");
        crate::cli::import::run(&input_path, merge, replace, dry_run)?;
    } else {
        let output_file = get_output_file_flag(&args);
        check_integration_warning();
        crate::run_tui(output_file)?;
    }

    Ok(())
}

fn get_shell_flag(args: &[String]) -> Option<Shell> {
    args.iter()
        .position(|a| a == "--shell")
        .and_then(|i| args.get(i + 1))
        .and_then(|s| Shell::from_str(s))
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

fn print_help() {
    println!("ctrlr - Command history picker");
    println!();
    println!("Usage: ctrlr [COMMAND]");
    println!();
    println!("Commands:");
    println!("  init              Add shell integration");
    println!("  export [FILE]     Export data to JSON (stdout if no file)");
    println!("  import FILE       Import data from JSON");
    println!();
    println!("Options:");
    println!("  --help, -h        Show this help");
    println!("  --output-file, -o Write selected command to file instead of stdout");
    println!();
    println!("Examples:");
    println!("  ctrlr             Open the TUI");
    println!("  ctrlr init        Add shell integration (Ctrl+R)");
    println!("  ctrlr init --print   Print integration script");
    println!("  ctrlr --output-file /tmp/cmd  Write output to file");
    println!("  ctrlr export      Export all data to stdout");
    println!("  ctrlr export backup.json  Export to file");
    println!("  ctrlr import backup.json  Import (merge mode)");
    println!("  ctrlr import backup.json --dry-run  Preview import");
    println!("  ctrlr import backup.json --replace  Replace all data");
}

fn print_init_help() {
    println!("ctrlr init - Add shell integration");
    println!();
    println!("Usage: ctrlr init [OPTIONS]");
    println!();
    println!("Options:");
    println!("  --shell <SHELL>   Force a specific shell (bash, zsh, fish)");
    println!("  --print           Only print the integration script, don't install");
    println!("  --help, -h        Show this help");
}

fn print_import_help() {
    println!("ctrlr import - Import data from JSON");
    println!();
    println!("Usage: ctrlr import FILE [OPTIONS]");
    println!();
    println!("Options:");
    println!("  --merge           Merge with existing data (default)");
    println!("  --replace         Replace all existing data");
    println!("  --dry-run         Preview changes without applying");
    println!("  --help, -h        Show this help");
}

#[cfg(test)]
mod tests {
    use super::*;

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
        for shell in [Shell::Bash, Shell::Zsh, Shell::Fish] {
            let installed = shells::generate_script(shell);
            assert_eq!(
                shells::integration_state(shell, &installed),
                shells::IntegrationState::Current,
                "{} reports its own script as current",
                shell
            );
        }
    }
}

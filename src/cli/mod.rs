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
    } else {
        check_integration_warning();
        crate::run_tui()?;
    }

    Ok(())
}

fn get_shell_flag(args: &[String]) -> Option<Shell> {
    args.iter()
        .position(|a| a == "--shell")
        .and_then(|i| args.get(i + 1))
        .and_then(|s| Shell::from_str(s))
}

fn check_integration_warning() {
    if let Some(shell) = Shell::detect() {
        let config_path = shell.config_path();
        if let Ok(content) = std::fs::read_to_string(&config_path)
            && !shells::is_installed(shell, &content)
        {
            println!();
            println!("⚡ ctrlr shell integration not found");
            println!();
            println!("Run:");
            println!("    ctrlr init");
            println!();
            println!("to enable keybindings (Ctrl+R)");
            println!();
        }
    }
}

fn print_help() {
    println!("ctrlr - Command history picker");
    println!();
    println!("Usage: ctrlr [COMMAND]");
    println!();
    println!("Commands:");
    println!("  init              Add shell integration");
    println!();
    println!("Options:");
    println!("  --help, -h        Show this help");
    println!();
    println!("Examples:");
    println!("  ctrlr             Open the TUI");
    println!("  ctrlr init        Add shell integration (Ctrl+R)");
    println!("  ctrlr init --print   Print integration script");
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
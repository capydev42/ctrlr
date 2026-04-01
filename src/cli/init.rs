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

    if shells::is_installed(shell, &config_content) {
        println!("✔ ctrlr already installed in {}", config_path.display());
        return Ok(());
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

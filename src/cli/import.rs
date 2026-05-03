use std::io::{self, Write};

use crate::storage::{
    import_export::{self, ImportMode},
    init_db,
};

pub fn run(input_path: &str, _merge: bool, replace: bool, dry_run: bool) -> color_eyre::Result<()> {
    if !std::path::Path::new(input_path).exists() {
        eprintln!("Error: file not found: {}", input_path);
        std::process::exit(1);
    }

    let content = std::fs::read_to_string(input_path)?;
    let data: import_export::ExportData = serde_json::from_str(&content).map_err(|e| {
        eprintln!("Error: invalid file format: {}", e);
        std::process::exit(1);
    })?;

    if data.version != 1 {
        eprintln!(
            "Error: unsupported export version {} (expected 1)",
            data.version
        );
        std::process::exit(1);
    }

    let mode = if dry_run {
        ImportMode::DryRun
    } else if replace {
        ImportMode::Replace
    } else {
        ImportMode::Merge
    };

    let mut conn = init_db()?;

    let preview = import_export::preview_import(&conn, &data)?;

    if dry_run {
        println!("Dry run preview:");
        println!("  + {} commands", preview.new_commands);
        println!("  + {} collections", preview.new_collections);
        if preview.duplicates > 0 {
            println!("  ~ {} duplicates (skipped)", preview.duplicates);
        }
        println!();
        println!("No changes will be made.");
        return Ok(());
    }

    if preview.new_commands == 0 && preview.new_collections == 0 {
        println!("Nothing to import: all data already exists.");
        return Ok(());
    }

    println!("This will:");
    if preview.new_commands > 0 {
        println!("  + add {} commands", preview.new_commands);
    }
    if preview.new_collections > 0 {
        println!("  + add {} collections", preview.new_collections);
    }
    if preview.duplicates > 0 {
        println!("  ~ skip {} duplicates", preview.duplicates);
    }

    if replace {
        println!();
        println!("WARNING: --replace will delete all existing data.");
        print!("Continue? [y/N]: ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim().to_lowercase();

        if input != "y" && input != "yes" {
            println!("Import cancelled.");
            return Ok(());
        }
    }

    let result = import_export::import_data(&mut conn, &data, &mode)?;

    println!();
    if result.imported_commands > 0 {
        println!("Imported {} commands", result.imported_commands);
    }
    if result.imported_collections > 0 {
        println!("Imported {} collections", result.imported_collections);
    }
    if result.skipped_commands > 0 {
        println!("Skipped {} duplicates", result.skipped_commands);
    }

    Ok(())
}

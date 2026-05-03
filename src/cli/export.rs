use std::io::Write;

use crate::storage::{import_export, init_db};

pub fn run(output_path: Option<&str>) -> color_eyre::Result<()> {
    let conn = init_db()?;
    let data = import_export::export_data(&conn)?;
    let json = serde_json::to_string_pretty(&data)?;

    match output_path {
        Some(path) => {
            let mut file = std::fs::File::create(path)?;
            file.write_all(json.as_bytes())?;
            println!(
                "Exported {} commands, {} collections, {} tags to {}",
                data.commands.len(),
                data.collections.len(),
                data.tags.len(),
                path,
            );
        }
        None => {
            println!("{}", json);
        }
    }

    Ok(())
}

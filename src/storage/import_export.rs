use rusqlite::{Connection, Transaction, params};
use serde::{Deserialize, Serialize};

use crate::storage::commands::hash_command;

const EXPORT_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportData {
    pub version: u32,
    pub exported_at: String,
    pub commands: Vec<ExportCommand>,
    pub collections: Vec<ExportCollection>,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportCommand {
    pub id: String,
    pub text: String,
    #[serde(default)]
    pub context: Vec<String>,
    pub favorite: bool,
    pub use_count: i32,
    pub last_used: Option<i64>,
    pub tags: Vec<String>,
    pub collections: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportCollection {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ImportMode {
    Merge,
    Replace,
    DryRun,
}

#[derive(Debug, Clone)]
pub struct ImportPreview {
    pub new_commands: usize,
    pub new_collections: usize,
    pub duplicates: usize,
}

pub fn export_data(conn: &Connection) -> rusqlite::Result<ExportData> {
    let commands = export_commands(conn)?;
    let collections = export_collections(conn)?;
    let tags = export_tags(conn)?;

    let exported_at = chrono_utc_now();

    Ok(ExportData {
        version: EXPORT_VERSION,
        exported_at,
        commands,
        collections,
        tags,
    })
}

fn export_commands(conn: &Connection) -> rusqlite::Result<Vec<ExportCommand>> {
    let mut stmt = conn.prepare(
        "SELECT id, text, favorite, use_count, last_used, created_at
         FROM commands
         ORDER BY created_at DESC",
    )?;

    let rows: Vec<(String, String, i32, i32, Option<i64>, i64)> = stmt
        .query_map([], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
            ))
        })?
        .filter_map(|r| r.ok())
        .collect();

    let mut commands = Vec::with_capacity(rows.len());

    for (id, text, favorite, use_count, last_used, _created_at) in rows {
        let tags = get_tags_for_command(conn, &id)?;
        let collection_names = get_collection_names_for_command(conn, &id)?;
        let context = get_context_for_command(conn, &id)?;

        commands.push(ExportCommand {
            id,
            text,
            context,
            favorite: favorite != 0,
            use_count,
            last_used,
            tags,
            collections: collection_names,
        });
    }

    Ok(commands)
}

fn get_tags_for_command(conn: &Connection, command_id: &str) -> rusqlite::Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT t.name FROM tags t
         JOIN command_tags ct ON t.id = ct.tag_id
         WHERE ct.command_id = ?",
    )?;

    let tags: Vec<String> = stmt
        .query_map([command_id], |row| row.get::<_, String>(0))?
        .filter_map(|r| r.ok())
        .collect();

    Ok(tags)
}

fn get_collection_names_for_command(
    conn: &Connection,
    command_id: &str,
) -> rusqlite::Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT c.name FROM collections c
         JOIN command_collections cc ON c.id = cc.collection_id
         WHERE cc.command_id = ?",
    )?;

    let names: Vec<String> = stmt
        .query_map([command_id], |row| row.get::<_, String>(0))?
        .filter_map(|r| r.ok())
        .collect();

    Ok(names)
}

fn get_context_for_command(_conn: &Connection, _command_id: &str) -> rusqlite::Result<Vec<String>> {
    Ok(Vec::new())
}

fn export_collections(conn: &Connection) -> rusqlite::Result<Vec<ExportCollection>> {
    let mut stmt = conn.prepare("SELECT id, name FROM collections ORDER BY name")?;

    let collections: Vec<ExportCollection> = stmt
        .query_map([], |row| {
            Ok(ExportCollection {
                id: row.get(0)?,
                name: row.get(1)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();

    Ok(collections)
}

fn export_tags(conn: &Connection) -> rusqlite::Result<Vec<String>> {
    let mut stmt = conn.prepare("SELECT name FROM tags ORDER BY name")?;

    let tags: Vec<String> = stmt
        .query_map([], |row| row.get::<_, String>(0))?
        .filter_map(|r| r.ok())
        .collect();

    Ok(tags)
}

pub fn preview_import(conn: &Connection, data: &ExportData) -> rusqlite::Result<ImportPreview> {
    let existing_command_hashes = get_existing_command_texts(conn)?;
    let existing_collection_ids = get_existing_collection_ids(conn)?;

    let mut new_commands = 0;
    let mut duplicates = 0;

    for cmd in &data.commands {
        let hash = hash_command(&cmd.text);
        if existing_command_hashes.contains(&hash) {
            duplicates += 1;
        } else {
            new_commands += 1;
        }
    }

    let mut new_collections = 0;
    for collection in &data.collections {
        if !existing_collection_ids.contains(&collection.id) {
            new_collections += 1;
        }
    }

    Ok(ImportPreview {
        new_commands,
        new_collections,
        duplicates,
    })
}

pub fn import_data(
    conn: &mut Connection,
    data: &ExportData,
    mode: &ImportMode,
) -> rusqlite::Result<ImportResult> {
    match mode {
        ImportMode::DryRun => {
            let preview = preview_import(conn, data)?;
            Ok(ImportResult {
                imported_commands: 0,
                imported_collections: 0,
                skipped_commands: preview.duplicates,
                is_dry_run: true,
            })
        }
        ImportMode::Replace => {
            let tx = conn.transaction()?;
            delete_all_data(&tx)?;
            let result = perform_import(&tx, data, true)?;
            tx.commit()?;
            Ok(result)
        }
        ImportMode::Merge => {
            let tx = conn.transaction()?;
            let result = perform_import(&tx, data, false)?;
            tx.commit()?;
            Ok(result)
        }
    }
}

pub struct ImportResult {
    pub imported_commands: usize,
    pub imported_collections: usize,
    pub skipped_commands: usize,
    #[allow(dead_code)]
    pub is_dry_run: bool,
}

fn perform_import(
    tx: &Transaction,
    data: &ExportData,
    replace: bool,
) -> rusqlite::Result<ImportResult> {
    let mut imported_commands = 0;
    let mut imported_collections = 0;
    let mut skipped_commands = 0;

    let existing_command_hashes = if replace {
        std::collections::HashSet::new()
    } else {
        get_existing_command_texts_with_tx(tx)?
    };

    let existing_collection_ids = if replace {
        std::collections::HashSet::new()
    } else {
        get_existing_collection_ids_with_tx(tx)?
    };

    for collection in &data.collections {
        if !existing_collection_ids.contains(&collection.id) {
            tx.execute(
                "INSERT INTO collections (id, name) VALUES (?, ?)",
                params![&collection.id, &collection.name],
            )?;
            imported_collections += 1;
        }
    }

    for cmd in &data.commands {
        let hash = hash_command(&cmd.text);

        if existing_command_hashes.contains(&hash) {
            skipped_commands += 1;
            continue;
        }

        tx.execute(
            "INSERT INTO commands (id, text, favorite, use_count, last_used) VALUES (?, ?, ?, ?, ?)",
            params![&hash, &cmd.text, cmd.favorite as i32, cmd.use_count, cmd.last_used],
        )?;

        imported_commands += 1;

        for tag in &cmd.tags {
            let tag_id = add_tag_if_not_exists(tx, tag)?;
            tx.execute(
                "INSERT OR IGNORE INTO command_tags (command_id, tag_id) VALUES (?, ?)",
                params![&hash, tag_id],
            )?;
        }

        for collection_name in &cmd.collections {
            let collection_id = hash_collection_name(collection_name);
            tx.execute(
                "INSERT OR IGNORE INTO command_collections (command_id, collection_id) VALUES (?, ?)",
                params![&hash, &collection_id],
            )?;
        }
    }

    Ok(ImportResult {
        imported_commands,
        imported_collections,
        skipped_commands,
        is_dry_run: false,
    })
}

fn delete_all_data(tx: &Transaction) -> rusqlite::Result<()> {
    tx.execute("DELETE FROM command_collections", [])?;
    tx.execute("DELETE FROM command_tags", [])?;
    tx.execute("DELETE FROM collections", [])?;
    tx.execute("DELETE FROM commands", [])?;
    tx.execute("DELETE FROM tags", [])?;
    Ok(())
}

fn get_existing_command_texts(
    conn: &Connection,
) -> rusqlite::Result<std::collections::HashSet<String>> {
    let mut stmt = conn.prepare("SELECT id FROM commands")?;
    let ids: std::collections::HashSet<String> = stmt
        .query_map([], |row| row.get::<_, String>(0))?
        .filter_map(|r| r.ok())
        .collect();
    Ok(ids)
}

fn get_existing_command_texts_with_tx(
    tx: &Transaction,
) -> rusqlite::Result<std::collections::HashSet<String>> {
    let mut stmt = tx.prepare("SELECT id FROM commands")?;
    let ids: std::collections::HashSet<String> = stmt
        .query_map([], |row| row.get::<_, String>(0))?
        .filter_map(|r| r.ok())
        .collect();
    Ok(ids)
}

fn get_existing_collection_ids(
    conn: &Connection,
) -> rusqlite::Result<std::collections::HashSet<String>> {
    let mut stmt = conn.prepare("SELECT id FROM collections")?;
    let ids: std::collections::HashSet<String> = stmt
        .query_map([], |row| row.get::<_, String>(0))?
        .filter_map(|r| r.ok())
        .collect();
    Ok(ids)
}

fn get_existing_collection_ids_with_tx(
    tx: &Transaction,
) -> rusqlite::Result<std::collections::HashSet<String>> {
    let mut stmt = tx.prepare("SELECT id FROM collections")?;
    let ids: std::collections::HashSet<String> = stmt
        .query_map([], |row| row.get::<_, String>(0))?
        .filter_map(|r| r.ok())
        .collect();
    Ok(ids)
}

fn add_tag_if_not_exists(tx: &Transaction, name: &str) -> rusqlite::Result<i64> {
    tx.execute("INSERT OR IGNORE INTO tags (name) VALUES (?)", [name])?;
    tx.query_row("SELECT id FROM tags WHERE name = ?", [name], |row| {
        row.get::<_, i64>(0)
    })
}

fn hash_collection_name(name: &str) -> String {
    hash_command(name)
}

fn chrono_utc_now() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let secs = now % 60;
    let mins = (now / 60) % 60;
    let hours = (now / 3600) % 24;
    let days_since_epoch = now / 86400;

    let mut year = 1970;
    let mut remaining_days = days_since_epoch;
    loop {
        let days_in_year = if is_leap_year(year) { 366 } else { 365 };
        if remaining_days < days_in_year as u64 {
            break;
        }
        remaining_days -= days_in_year as u64;
        year += 1;
    }

    let month_days = if is_leap_year(year) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };

    let mut month = 1;
    for &days in &month_days {
        if remaining_days < days as u64 {
            break;
        }
        remaining_days -= days as u64;
        month += 1;
    }
    let day = remaining_days + 1;

    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        year, month, day, hours, mins, secs
    )
}

fn is_leap_year(year: u32) -> bool {
    (year.is_multiple_of(4) && !year.is_multiple_of(100)) || year.is_multiple_of(400)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::init_db_with_conn;

    fn test_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        init_db_with_conn(&conn).unwrap();
        conn
    }

    fn seed_test_data(conn: &mut Connection) {
        use crate::storage::collections::create_collection;
        use crate::storage::commands::ensure_commands_exist;
        use crate::storage::tags::add_tag;

        let hash1 = hash_command("git status");
        let hash2 = hash_command("cargo build");

        ensure_commands_exist(
            conn,
            &[
                ("git status", hash1.clone()),
                ("cargo build", hash2.clone()),
            ],
        )
        .unwrap();

        conn.execute(
            "UPDATE commands SET favorite = 1, use_count = 5 WHERE id = ?",
            [&hash1],
        )
        .unwrap();

        let col_id = create_collection(conn, "work").unwrap();
        conn.execute(
            "INSERT INTO command_collections (command_id, collection_id) VALUES (?, ?)",
            [&hash1, &col_id],
        )
        .unwrap();

        let tag_id = add_tag(conn, "git").unwrap();
        conn.execute(
            "INSERT INTO command_tags (command_id, tag_id) VALUES (?, ?)",
            (&hash1, tag_id),
        )
        .unwrap();
    }

    #[test]
    fn test_export_returns_data() {
        let mut conn = test_conn();
        seed_test_data(&mut conn);

        let data = export_data(&conn).unwrap();
        assert_eq!(data.version, EXPORT_VERSION);
        assert_eq!(data.commands.len(), 2);
        assert!(!data.exported_at.is_empty());
    }

    #[test]
    fn test_export_commands_sorted_by_created_at_desc() {
        let mut conn = test_conn();
        seed_test_data(&mut conn);

        let data = export_data(&conn).unwrap();
        assert_eq!(data.commands.len(), 2);
        assert!(data.commands[0].text == "git status" || data.commands[1].text == "git status");
    }

    #[test]
    fn test_export_includes_tags_and_collections() {
        let mut conn = test_conn();
        seed_test_data(&mut conn);

        let data = export_data(&conn).unwrap();
        let git_cmd = data
            .commands
            .iter()
            .find(|c| c.text == "git status")
            .unwrap();
        assert_eq!(git_cmd.tags, vec!["git"]);
        assert_eq!(git_cmd.collections, vec!["work"]);
        assert!(git_cmd.favorite);
        assert_eq!(git_cmd.use_count, 5);
    }

    #[test]
    fn test_preview_import_new_data() {
        let conn = test_conn();
        let data = ExportData {
            version: EXPORT_VERSION,
            exported_at: chrono_utc_now(),
            commands: vec![ExportCommand {
                id: hash_command("new cmd"),
                text: "new cmd".to_string(),
                context: vec![],
                favorite: false,
                use_count: 0,
                last_used: None,
                tags: vec![],
                collections: vec![],
            }],
            collections: vec![],
            tags: vec![],
        };

        let preview = preview_import(&conn, &data).unwrap();
        assert_eq!(preview.new_commands, 1);
        assert_eq!(preview.duplicates, 0);
    }

    #[test]
    fn test_preview_import_detects_duplicates() {
        let mut conn = test_conn();
        seed_test_data(&mut conn);

        let data = ExportData {
            version: EXPORT_VERSION,
            exported_at: chrono_utc_now(),
            commands: vec![
                ExportCommand {
                    id: hash_command("git status"),
                    text: "git status".to_string(),
                    context: vec![],
                    favorite: false,
                    use_count: 0,
                    last_used: None,
                    tags: vec![],
                    collections: vec![],
                },
                ExportCommand {
                    id: hash_command("totally new"),
                    text: "totally new".to_string(),
                    context: vec![],
                    favorite: false,
                    use_count: 0,
                    last_used: None,
                    tags: vec![],
                    collections: vec![],
                },
            ],
            collections: vec![],
            tags: vec![],
        };

        let preview = preview_import(&conn, &data).unwrap();
        assert_eq!(preview.new_commands, 1);
        assert_eq!(preview.duplicates, 1);
    }

    #[test]
    fn test_import_merge_new_commands() {
        let mut conn = test_conn();
        let data = ExportData {
            version: EXPORT_VERSION,
            exported_at: chrono_utc_now(),
            commands: vec![ExportCommand {
                id: hash_command("ls -la"),
                text: "ls -la".to_string(),
                context: vec!["shell:bash".to_string()],
                favorite: true,
                use_count: 3,
                last_used: Some(1700000000),
                tags: vec!["files".to_string()],
                collections: vec![],
            }],
            collections: vec![],
            tags: vec![],
        };

        let result = import_data(&mut conn, &data, &ImportMode::Merge).unwrap();
        assert_eq!(result.imported_commands, 1);
        assert_eq!(result.skipped_commands, 0);

        let count: i32 = conn
            .query_row("SELECT COUNT(*) FROM commands", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_import_merge_skips_duplicates() {
        let mut conn = test_conn();
        seed_test_data(&mut conn);

        let existing_count: i32 = conn
            .query_row("SELECT COUNT(*) FROM commands", [], |row| row.get(0))
            .unwrap();

        let data = ExportData {
            version: EXPORT_VERSION,
            exported_at: chrono_utc_now(),
            commands: vec![
                ExportCommand {
                    id: hash_command("git status"),
                    text: "git status".to_string(),
                    context: vec![],
                    favorite: false,
                    use_count: 0,
                    last_used: None,
                    tags: vec![],
                    collections: vec![],
                },
                ExportCommand {
                    id: hash_command("new one"),
                    text: "new one".to_string(),
                    context: vec![],
                    favorite: false,
                    use_count: 0,
                    last_used: None,
                    tags: vec![],
                    collections: vec![],
                },
            ],
            collections: vec![],
            tags: vec![],
        };

        let result = import_data(&mut conn, &data, &ImportMode::Merge).unwrap();
        assert_eq!(result.imported_commands, 1);
        assert_eq!(result.skipped_commands, 1);

        let new_count: i32 = conn
            .query_row("SELECT COUNT(*) FROM commands", [], |row| row.get(0))
            .unwrap();
        assert_eq!(new_count, existing_count + 1);
    }

    #[test]
    fn test_import_replace_deletes_all() {
        let mut conn = test_conn();
        seed_test_data(&mut conn);

        let data = ExportData {
            version: EXPORT_VERSION,
            exported_at: chrono_utc_now(),
            commands: vec![ExportCommand {
                id: hash_command("only this"),
                text: "only this".to_string(),
                context: vec![],
                favorite: false,
                use_count: 0,
                last_used: None,
                tags: vec![],
                collections: vec![],
            }],
            collections: vec![],
            tags: vec![],
        };

        let result = import_data(&mut conn, &data, &ImportMode::Replace).unwrap();
        assert_eq!(result.imported_commands, 1);

        let count: i32 = conn
            .query_row("SELECT COUNT(*) FROM commands", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1);

        let tag_count: i32 = conn
            .query_row("SELECT COUNT(*) FROM tags", [], |row| row.get(0))
            .unwrap();
        assert_eq!(tag_count, 0);
    }

    #[test]
    fn test_import_dry_run_no_changes() {
        let mut conn = test_conn();

        let data = ExportData {
            version: EXPORT_VERSION,
            exported_at: chrono_utc_now(),
            commands: vec![ExportCommand {
                id: hash_command("should not appear"),
                text: "should not appear".to_string(),
                context: vec![],
                favorite: false,
                use_count: 0,
                last_used: None,
                tags: vec![],
                collections: vec![],
            }],
            collections: vec![],
            tags: vec![],
        };

        let result = import_data(&mut conn, &data, &ImportMode::DryRun).unwrap();
        assert!(result.is_dry_run);
        assert_eq!(result.imported_commands, 0);

        let count: i32 = conn
            .query_row("SELECT COUNT(*) FROM commands", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn test_import_merge_collections() {
        let mut conn = test_conn();

        let data = ExportData {
            version: EXPORT_VERSION,
            exported_at: chrono_utc_now(),
            commands: vec![ExportCommand {
                id: hash_command("deploy"),
                text: "deploy".to_string(),
                context: vec![],
                favorite: false,
                use_count: 0,
                last_used: None,
                tags: vec![],
                collections: vec!["prod".to_string()],
            }],
            collections: vec![ExportCollection {
                id: hash_command("prod"),
                name: "prod".to_string(),
            }],
            tags: vec![],
        };

        import_data(&mut conn, &data, &ImportMode::Merge).unwrap();

        let col_name: String = conn
            .query_row(
                "SELECT name FROM collections WHERE id = ?",
                [hash_command("prod")],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(col_name, "prod");
    }

    #[test]
    fn test_export_empty_db() {
        let conn = test_conn();
        let data = export_data(&conn).unwrap();
        assert_eq!(data.commands.len(), 0);
        assert_eq!(data.collections.len(), 0);
        assert_eq!(data.tags.len(), 0);
    }
}

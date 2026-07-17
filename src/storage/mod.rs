use rusqlite::{Connection, params};
use std::path::PathBuf;

pub mod collections;
pub mod commands;
pub mod import_export;
pub mod migrations;
pub mod tags;

pub fn get_db_path() -> PathBuf {
    // linux: ~/.local/share/ctrlr/ctrlr.db
    // mac: ~/Library/Application Support/ctrlr/ctrlt.db
    // windows: %APPDATA%\ctrlr\ctrlr.db
    let base = dirs::data_dir().unwrap_or_else(|| PathBuf::from("."));
    let dir = base.join("ctrlr");
    std::fs::create_dir_all(&dir).ok();
    dir.join("ctrlr.db")
}

pub fn init_db() -> rusqlite::Result<Connection> {
    let db_path = get_db_path();
    let mut conn = Connection::open(&db_path)?;
    init_db_with_conn(&conn)?;

    if migrations::needs_migration(&conn) {
        // The migration merges and deletes rows; keep a copy of the last known
        // good state. Best-effort — a failed backup must not block startup.
        let backup = db_path.with_extension("db.pre-migration.bak");
        if let Err(e) = std::fs::copy(&db_path, &backup) {
            eprintln!("ctrlr: could not back up database before migrating: {}", e);
        }
    }

    // Non-fatal by design, matching the Option<Connection> the app holds: on
    // failure the transaction rolled back and the next launch retries.
    if let Err(e) = migrations::run_migrations(&mut conn) {
        eprintln!(
            "ctrlr: database migration failed, continuing without it: {}",
            e
        );
    }

    Ok(conn)
}

/// Creates the schema only. Production callers want [`init_db`], which also
/// runs migrations; this is split out so tests can build a bare in-memory DB.
pub fn init_db_with_conn(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS commands (
            id TEXT PRIMARY KEY,
            text TEXT NOT NULL,
            favorite INTEGER DEFAULT 0,
            last_used INTEGER,
            use_count INTEGER DEFAULT 0,
            created_at INTEGER DEFAULT (strftime('%s', 'now'))
        );

        CREATE TABLE IF NOT EXISTS tags (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL UNIQUE
        );

        CREATE TABLE IF NOT EXISTS command_tags (
            command_id TEXT REFERENCES commands(id) ON DELETE CASCADE,
            tag_id INTEGER REFERENCES tags(id) ON DELETE CASCADE,
            PRIMARY KEY (command_id, tag_id)
        );

        CREATE TABLE IF NOT EXISTS collections (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS command_collections (
            command_id TEXT REFERENCES commands(id) ON DELETE CASCADE,
            collection_id TEXT REFERENCES collections(id) ON DELETE CASCADE,
            PRIMARY KEY (command_id, collection_id)
        );

        CREATE TABLE IF NOT EXISTS settings (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_commands_text ON commands(text);
        CREATE INDEX IF NOT EXISTS idx_commands_favorite ON commands(favorite);
        CREATE INDEX IF NOT EXISTS idx_commands_use_count ON commands(use_count DESC);
        CREATE INDEX IF NOT EXISTS idx_command_collections_collection ON command_collections(collection_id);
        ",
    )?;
    Ok(())
}

pub fn save_theme(conn: &Connection, theme_name: &str) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT OR REPLACE INTO settings (key, value) VALUES ('theme', ?)",
        params![theme_name],
    )?;
    Ok(())
}

pub fn load_theme(conn: &Connection) -> Option<String> {
    conn.query_row(
        "SELECT value FROM settings WHERE key = 'theme'",
        [],
        |row| row.get(0),
    )
    .ok()
}

#[derive(Debug, Clone)]
pub struct CommandMeta {
    pub favorite: bool,
    pub last_used: Option<i64>,
    pub use_count: i32,
}

pub fn load_metadata(conn: &Connection, text: &str) -> Option<CommandMeta> {
    let id = crate::hash::hash_command(text);

    let mut stmt = conn
        .prepare("SELECT favorite, last_used, use_count FROM commands WHERE id = ?")
        .ok()?;

    let meta = stmt
        .query_row([&id], |row| {
            Ok(CommandMeta {
                favorite: row.get::<_, i32>(0)? != 0,
                last_used: row.get::<_, Option<i64>>(1)?,
                use_count: row.get::<_, i32>(2)?,
            })
        })
        .ok()?;

    Some(meta)
}

pub fn load_tags(conn: &Connection, text: &str) -> Vec<String> {
    let id = crate::hash::hash_command(text);

    let mut stmt = match conn.prepare(
        "SELECT t.name FROM tags t 
         JOIN command_tags ct ON t.id = ct.tag_id 
         WHERE ct.command_id = ?",
    ) {
        Ok(s) => s,
        Err(_) => return vec![],
    };

    let tags: Vec<String> = stmt
        .query_map([&id], |row| row.get(0))
        .ok()
        .map(|iter| iter.filter_map(|t| t.ok()).collect())
        .unwrap_or_default();

    tags
}

pub fn hydrate_commands(conn: &mut rusqlite::Connection, commands: &mut [crate::app::Command]) {
    for cmd in commands {
        if let Some(meta) = load_metadata(conn, &cmd.text) {
            cmd.favorite = meta.favorite;
            if meta.use_count > cmd.use_count {
                cmd.use_count = meta.use_count;
            }
            if meta.last_used > cmd.last_used {
                cmd.last_used = meta.last_used;
            }
        }

        let tags = load_tags(conn, &cmd.text);
        if !tags.is_empty() {
            cmd.tags = tags;
        }

        let collections =
            collections::get_collections_for_command(conn, &cmd.text).unwrap_or_default();
        if !collections.is_empty() {
            cmd.collection_ids = collections;
        }
    }
}

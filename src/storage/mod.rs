use rusqlite::Connection;
use std::path::PathBuf;

pub mod commands;
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
    let conn = Connection::open(&db_path)?;
    // maybe ORM in future?
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

        CREATE INDEX IF NOT EXISTS idx_commands_text ON commands(text);
        CREATE INDEX IF NOT EXISTS idx_commands_favorite ON commands(favorite);
        CREATE INDEX IF NOT EXISTS idx_commands_use_count ON commands(use_count DESC);
        ",
    )?;

    Ok(conn)
}

#[derive(Debug, Clone)]
pub struct CommandMeta {
    pub favorite: bool,
    pub last_used: Option<i64>,
    pub use_count: i32,
}

pub fn load_metadata(conn: &Connection, text: &str) -> Option<CommandMeta> {
    let id = commands::hash_command(text);
    
    let mut stmt = conn.prepare(
        "SELECT favorite, last_used, use_count FROM commands WHERE id = ?"
    ).ok()?;
    
    let meta = stmt.query_row([&id], |row| {
        Ok(CommandMeta {
            favorite: row.get::<_, i32>(0)? != 0,
            last_used: row.get::<_, Option<i64>>(1)?,
            use_count: row.get::<_, i32>(2)?,
        })
    }).ok()?;
    
    Some(meta)
}

pub fn load_tags(conn: &Connection, text: &str) -> Vec<String> {
    let id = commands::hash_command(text);
    
    let mut stmt = match conn.prepare(
        "SELECT t.name FROM tags t 
         JOIN command_tags ct ON t.id = ct.tag_id 
         WHERE ct.command_id = ?"
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

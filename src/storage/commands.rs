use rusqlite::Connection;
use sha1::{Digest, Sha1};

pub fn hash_command(text: &str) -> String {
    let mut hasher = Sha1::new();
    hasher.update(text.as_bytes());
    format!("{:x}", hasher.finalize())
}

pub fn ensure_commands_exist(
    conn: &mut Connection,
    commands: &[(&str, String)],
) -> rusqlite::Result<()> {
    let tx = conn.transaction()?;
    {
        let mut stmt = tx.prepare("INSERT OR IGNORE INTO commands (id, text) VALUES (?, ?)")?;
        for (text, id) in commands {
            stmt.execute([id.as_str(), text])?;
        }
    }
    tx.commit()?;
    Ok(())
}

pub fn update_favorite(conn: &Connection, text: &str, favorite: bool) -> rusqlite::Result<()> {
    let id = hash_command(text);

    conn.execute(
        "INSERT INTO commands (id, text, favorite) VALUES (?1, ?2, ?3)
         ON CONFLICT(id) DO UPDATE SET favorite = excluded.favorite",
        (&id, text, favorite as i32),
    )?;

    Ok(())
}

pub fn increment_use_count(conn: &Connection, text: &str) -> rusqlite::Result<()> {
    let id = hash_command(text);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .ok();

    conn.execute(
        "INSERT INTO commands (id, text, use_count, last_used) VALUES (?1, ?2, 1, ?3)
         ON CONFLICT(id) DO UPDATE SET 
         use_count = use_count + 1,
         last_used = excluded.last_used",
        (&id, text, now),
    )?;

    Ok(())
}

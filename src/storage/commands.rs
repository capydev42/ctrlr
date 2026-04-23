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

#[cfg(test)]
mod tests {
    use super::*;

    fn test_conn() -> Connection {
        use crate::storage::init_db_with_conn;
        let conn = Connection::open_in_memory().unwrap();
        init_db_with_conn(&conn).unwrap();
        conn
    }

    #[test]
    fn test_hash_command_deterministic() {
        let h1 = hash_command("ls -la");
        let h2 = hash_command("ls -la");
        assert_eq!(h1, h2);
        assert_eq!(h1.len(), 40);
    }

    #[test]
    fn test_hash_command_different_inputs() {
        let h1 = hash_command("ls -la");
        let h2 = hash_command("ls -la ");
        assert_ne!(h1, h2);
    }

    #[test]
    fn test_ensure_commands_exist() {
        let mut conn = test_conn();
        let cmds: Vec<(&str, String)> = vec![
            ("echo hello", "abc123".to_string()),
            ("ls", "def456".to_string()),
        ];
        ensure_commands_exist(&mut conn, &cmds).unwrap();

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM commands", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 2);
    }

    #[test]
    fn test_update_favorite_toggle_on() {
        let conn = test_conn();
        update_favorite(&conn, "test cmd", true).unwrap();

        let favorite: i32 = conn
            .query_row(
                "SELECT favorite FROM commands WHERE id = ?",
                [hash_command("test cmd")],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(favorite, 1);
    }

    #[test]
    fn test_update_favorite_toggle_off() {
        let conn = test_conn();
        update_favorite(&conn, "test cmd", true).unwrap();
        update_favorite(&conn, "test cmd", false).unwrap();

        let favorite: i32 = conn
            .query_row(
                "SELECT favorite FROM commands WHERE id = ?",
                [hash_command("test cmd")],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(favorite, 0);
    }

    #[test]
    fn test_increment_use_count() {
        let conn = test_conn();
        increment_use_count(&conn, "test cmd").unwrap();
        increment_use_count(&conn, "test cmd").unwrap();

        let count: i32 = conn
            .query_row(
                "SELECT use_count FROM commands WHERE id = ?",
                [hash_command("test cmd")],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 2);
    }

    #[test]
    fn test_increment_creates_command() {
        let conn = test_conn();
        increment_use_count(&conn, "new cmd").unwrap();

        let text: String = conn
            .query_row(
                "SELECT text FROM commands WHERE id = ?",
                [hash_command("new cmd")],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(text, "new cmd");
    }
}

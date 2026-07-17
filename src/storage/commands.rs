use crate::hash::hash_command;
use rusqlite::Connection;

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
    fn test_update_favorite_uses_normalized_id() {
        let conn = test_conn();
        update_favorite(&conn, "Git Status", true).unwrap();

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM commands", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1, "mixed-case text must not create a shadow row");

        let id: String = conn
            .query_row("SELECT id FROM commands", [], |r| r.get(0))
            .unwrap();
        assert_eq!(id, hash_command("git status"));
    }

    #[test]
    fn test_increment_use_count_uses_normalized_id() {
        let conn = test_conn();
        increment_use_count(&conn, "Git Status").unwrap();
        increment_use_count(&conn, "git status").unwrap();

        let (count, uses): (i64, i64) = conn
            .query_row("SELECT COUNT(*), SUM(use_count) FROM commands", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .unwrap();
        assert_eq!(count, 1, "casing variants must share one row");
        assert_eq!(uses, 2);
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

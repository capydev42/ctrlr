use rusqlite::Connection;

pub fn add_tag(conn: &Connection, name: &str) -> rusqlite::Result<i64> {
    conn.execute("INSERT OR IGNORE INTO tags (name) VALUES (?)", [name])?;

    conn.query_row("SELECT id FROM tags WHERE name = ?", [name], |row| {
        row.get(0)
    })
}

pub fn set_tags_for_command(
    conn: &mut Connection,
    text: &str,
    new_tags: &[String],
) -> rusqlite::Result<()> {
    use crate::hash::hash_command;

    let cmd_id = hash_command(text);

    let tx = conn.transaction()?;
    {
        tx.execute(
            "INSERT OR IGNORE INTO commands (id, text) VALUES (?, ?)",
            [&cmd_id, text],
        )?;

        let current_tags: Vec<String> = {
            let mut stmt = tx.prepare(
                "SELECT t.name FROM tags t 
                 JOIN command_tags ct ON t.id = ct.tag_id 
                 WHERE ct.command_id = ?",
            )?;
            stmt.query_map([&cmd_id], |row| row.get(0))?
                .filter_map(|r| r.ok())
                .collect()
        };

        let new_set: std::collections::HashSet<_> = new_tags.iter().collect();
        let current_set: std::collections::HashSet<_> = current_tags.iter().collect();

        let to_add: Vec<_> = new_set.difference(&current_set).collect();
        let to_remove: Vec<_> = current_set.difference(&new_set).collect();

        for tag in to_remove {
            let tag_id: Option<i64> = tx
                .query_row(
                    "SELECT id FROM tags WHERE name = ?",
                    [tag.as_str()],
                    |row| row.get(0),
                )
                .ok();
            if let Some(id) = tag_id {
                tx.execute(
                    "DELETE FROM command_tags WHERE command_id = ? AND tag_id = ?",
                    (&cmd_id, id),
                )?;
            }
        }

        for tag in to_add {
            let tag_id = add_tag(&tx, tag)?;
            tx.execute(
                "INSERT OR IGNORE INTO command_tags (command_id, tag_id) VALUES (?, ?)",
                (&cmd_id, tag_id),
            )?;
        }
    }
    tx.commit()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(test)]
    fn test_conn() -> Connection {
        use crate::storage::init_db_with_conn;
        let conn = Connection::open_in_memory().unwrap();
        init_db_with_conn(&conn).unwrap();
        conn
    }

    #[test]
    fn test_add_tag() {
        let conn = test_conn();
        let id = add_tag(&conn, "rust").unwrap();
        assert!(id > 0);
    }

    #[test]
    fn test_add_tag_duplicate_returns_same_id() {
        let conn = test_conn();
        let id1 = add_tag(&conn, "rust").unwrap();
        let id2 = add_tag(&conn, "rust").unwrap();
        assert_eq!(id1, id2);
    }

    #[test]
    fn test_set_tags_for_command_adds_tags() {
        let mut conn = test_conn();
        set_tags_for_command(
            &mut conn,
            "cargo run",
            &["rust".to_string(), "dev".to_string()],
        )
        .unwrap();

        let tags: Vec<String> = {
            let mut stmt = conn
                .prepare(
                    "SELECT t.name FROM tags t 
                     JOIN command_tags ct ON t.id = ct.tag_id 
                     JOIN commands c ON ct.command_id = c.id 
                     WHERE c.text = ?",
                )
                .unwrap();
            stmt.query_map(["cargo run"], |row| row.get(0))
                .unwrap()
                .filter_map(|r| r.ok())
                .collect()
        };

        assert_eq!(tags.len(), 2);
        assert!(tags.contains(&"rust".to_string()));
    }

    #[test]
    fn test_set_tags_for_command_replaces_tags() {
        let mut conn = test_conn();
        set_tags_for_command(&mut conn, "cargo run", &["rust".to_string()]).unwrap();
        set_tags_for_command(
            &mut conn,
            "cargo run",
            &["rust".to_string(), "db".to_string()],
        )
        .unwrap();

        let tags: Vec<String> = {
            let mut stmt = conn
                .prepare(
                    "SELECT t.name FROM tags t 
                     JOIN command_tags ct ON t.id = ct.tag_id 
                     JOIN commands c ON ct.command_id = c.id 
                     WHERE c.text = ?",
                )
                .unwrap();
            stmt.query_map(["cargo run"], |row| row.get(0))
                .unwrap()
                .filter_map(|r| r.ok())
                .collect()
        };

        assert_eq!(tags.len(), 2);
    }

    #[test]
    fn test_set_tags_for_command_removes_tags() {
        let mut conn = test_conn();
        set_tags_for_command(
            &mut conn,
            "cargo run",
            &["rust".to_string(), "db".to_string()],
        )
        .unwrap();
        set_tags_for_command(&mut conn, "cargo run", &["rust".to_string()]).unwrap();

        let tags: Vec<String> = {
            let mut stmt = conn
                .prepare(
                    "SELECT t.name FROM tags t 
                     JOIN command_tags ct ON t.id = ct.tag_id 
                     JOIN commands c ON ct.command_id = c.id 
                     WHERE c.text = ?",
                )
                .unwrap();
            stmt.query_map(["cargo run"], |row| row.get(0))
                .unwrap()
                .filter_map(|r| r.ok())
                .collect()
        };

        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0], "rust");
    }
}

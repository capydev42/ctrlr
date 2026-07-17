use crate::hash::hash_command;
use rusqlite::Connection;

#[derive(Debug, Clone)]
pub struct Collection {
    pub id: String,
    pub name: String,
}

/// Collection ids hash the raw name, unlike command ids.
///
/// Renaming a collection deliberately keeps the old id, so this must never
/// normalize — two collections may legitimately differ only by case.
pub fn hash_collection_name(name: &str) -> String {
    crate::hash::sha1_hex(name)
}

pub fn create_collection(conn: &Connection, name: &str) -> rusqlite::Result<String> {
    let id = hash_collection_name(name);
    conn.execute(
        "INSERT INTO collections (id, name) VALUES (?, ?)",
        (&id, name),
    )?;
    Ok(id)
}

pub fn get_all_collections(conn: &Connection) -> rusqlite::Result<Vec<Collection>> {
    let mut stmt = conn.prepare("SELECT id, name FROM collections ORDER BY name")?;
    let collections = stmt
        .query_map([], |row| {
            Ok(Collection {
                id: row.get(0)?,
                name: row.get(1)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(collections)
}

pub fn rename_collection(conn: &Connection, id: &str, new_name: &str) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE collections SET name = ? WHERE id = ?",
        (new_name, id),
    )?;
    Ok(())
}

pub fn delete_collection(conn: &mut Connection, id: &str) -> rusqlite::Result<()> {
    let tx = conn.transaction()?;
    {
        tx.execute(
            "DELETE FROM command_collections WHERE collection_id = ?",
            [id],
        )?;
        tx.execute("DELETE FROM collections WHERE id = ?", [id])?;
    }
    tx.commit()?;
    Ok(())
}

pub fn add_command_to_collection(
    conn: &Connection,
    cmd_text: &str,
    collection_id: &str,
) -> rusqlite::Result<()> {
    let cmd_id = hash_command(cmd_text);

    let exists: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM commands WHERE id = ?)",
        [&cmd_id],
        |row| row.get(0),
    )?;

    if !exists {
        conn.execute(
            "INSERT INTO commands (id, text, favorite, use_count) VALUES (?, ?, 0, 0)",
            (&cmd_id, cmd_text),
        )?;
    }

    conn.execute(
        "INSERT OR IGNORE INTO command_collections (command_id, collection_id) VALUES (?, ?)",
        (&cmd_id, collection_id),
    )?;
    Ok(())
}

pub fn remove_command_from_collection(
    conn: &Connection,
    cmd_text: &str,
    collection_id: &str,
) -> rusqlite::Result<()> {
    let cmd_id = hash_command(cmd_text);
    conn.execute(
        "DELETE FROM command_collections WHERE command_id = ? AND collection_id = ?",
        (&cmd_id, collection_id),
    )?;
    Ok(())
}

pub fn get_command_ids_in_collection(
    conn: &Connection,
    collection_id: &str,
) -> rusqlite::Result<Vec<String>> {
    let mut stmt =
        conn.prepare("SELECT command_id FROM command_collections WHERE collection_id = ?")?;
    let ids = stmt
        .query_map([collection_id], |row| row.get(0))?
        .filter_map(|r| r.ok())
        .collect();
    Ok(ids)
}

pub fn get_collections_for_command(
    conn: &Connection,
    cmd_text: &str,
) -> rusqlite::Result<Vec<String>> {
    let cmd_id = hash_command(cmd_text);
    let mut stmt =
        conn.prepare("SELECT collection_id FROM command_collections WHERE command_id = ?")?;
    let ids = stmt
        .query_map([&cmd_id], |row| row.get(0))?
        .filter_map(|r| r.ok())
        .collect();
    Ok(ids)
}

#[allow(dead_code)]
pub fn command_in_collection(conn: &Connection, cmd_text: &str, collection_id: &str) -> bool {
    let cmd_id = hash_command(cmd_text);
    conn.query_row(
        "SELECT 1 FROM command_collections WHERE command_id = ? AND collection_id = ?",
        [&cmd_id, collection_id],
        |_| Ok(()),
    )
    .is_ok()
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
    fn test_create_collection() {
        let conn = test_conn();
        let id = create_collection(&conn, "work").unwrap();
        assert!(!id.is_empty());
    }

    #[test]
    fn test_hash_collection_name_is_case_sensitive() {
        // Unlike command ids: renames keep the old id, so normalizing here
        // would silently merge distinct collections.
        assert_ne!(hash_collection_name("Work"), hash_collection_name("work"));
        assert_eq!(hash_collection_name("work"), hash_collection_name("work"));
    }

    #[test]
    fn test_add_command_to_collection_normalizes() {
        let conn = test_conn();
        let col_id = create_collection(&conn, "build").unwrap();
        add_command_to_collection(&conn, "Cargo Build", &col_id).unwrap();

        assert!(command_in_collection(&conn, "cargo build", &col_id));
        assert!(command_in_collection(&conn, "  CARGO BUILD  ", &col_id));

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM commands", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1, "must not create a raw-hashed shadow row");
    }

    #[test]
    fn test_get_all_collections() {
        let conn = test_conn();
        create_collection(&conn, "work").unwrap();
        create_collection(&conn, "personal").unwrap();

        let all = get_all_collections(&conn).unwrap();
        assert_eq!(all.len(), 2);
        assert!(all.iter().any(|c| c.name == "work"));
        assert!(all.iter().any(|c| c.name == "personal"));
    }

    #[test]
    fn test_rename_collection() {
        let conn = test_conn();
        let id = create_collection(&conn, "old").unwrap();
        rename_collection(&conn, &id, "new").unwrap();

        let all = get_all_collections(&conn).unwrap();
        assert!(all.iter().any(|c| c.name == "new"));
        assert!(!all.iter().any(|c| c.name == "old"));
    }

    #[test]
    fn test_delete_collection() {
        let mut conn = test_conn();
        let id = create_collection(&conn, "temp").unwrap();
        delete_collection(&mut conn, &id).unwrap();

        let all = get_all_collections(&conn).unwrap();
        assert!(all.is_empty());
    }

    #[test]
    fn test_add_command_to_collection() {
        let conn = test_conn();
        let col_id = create_collection(&conn, "work").unwrap();
        add_command_to_collection(&conn, "cargo build", &col_id).unwrap();

        assert!(command_in_collection(&conn, "cargo build", &col_id));
    }

    #[test]
    fn test_remove_command_from_collection() {
        let conn = test_conn();
        let col_id = create_collection(&conn, "work").unwrap();
        add_command_to_collection(&conn, "cargo build", &col_id).unwrap();
        remove_command_from_collection(&conn, "cargo build", &col_id).unwrap();

        assert!(!command_in_collection(&conn, "cargo build", &col_id));
    }

    #[test]
    fn test_get_command_ids_in_collection() {
        let conn = test_conn();
        let col_id = create_collection(&conn, "work").unwrap();
        add_command_to_collection(&conn, "cmd1", &col_id).unwrap();
        add_command_to_collection(&conn, "cmd2", &col_id).unwrap();

        let ids = get_command_ids_in_collection(&conn, &col_id).unwrap();
        assert_eq!(ids.len(), 2);
    }

    #[test]
    fn test_get_collections_for_command() {
        let conn = test_conn();
        let col1 = create_collection(&conn, "work").unwrap();
        let col2 = create_collection(&conn, "personal").unwrap();
        add_command_to_collection(&conn, "ls", &col1).unwrap();
        add_command_to_collection(&conn, "ls", &col2).unwrap();

        let cols = get_collections_for_command(&conn, "ls").unwrap();
        assert_eq!(cols.len(), 2);
    }
}

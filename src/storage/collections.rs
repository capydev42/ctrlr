use rusqlite::Connection;
use sha1::{Digest, Sha1};

#[derive(Debug, Clone)]
pub struct Collection {
    pub id: String,
    pub name: String,
}

fn hash_name(name: &str) -> String {
    let mut hasher = Sha1::new();
    hasher.update(name.as_bytes());
    format!("{:x}", hasher.finalize())
}

pub fn create_collection(conn: &Connection, name: &str) -> rusqlite::Result<String> {
    let id = hash_name(name);
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

fn hash_command(text: &str) -> String {
    let mut hasher = Sha1::new();
    hasher.update(text.as_bytes());
    format!("{:x}", hasher.finalize())
}

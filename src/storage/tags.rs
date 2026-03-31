use rusqlite::Connection;
// one tag -> many commands
// one command -> many tags
// n to n
pub fn add_tag(conn: &Connection, name: &str) -> rusqlite::Result<i64> {
    conn.execute("INSERT OR IGNORE INTO tags (name) VALUES (?)", [name])?;

    conn.query_row("SELECT id FROM tags WHERE name = ?", [name], |row| {
        row.get(0)
    })
}

pub fn set_tags_for_command(
    conn: &Connection,
    text: &str,
    tags: &[String],
) -> rusqlite::Result<()> {
    use crate::storage::commands::hash_command;

    let cmd_id = hash_command(text);

    conn.execute("DELETE FROM command_tags WHERE command_id = ?", [&cmd_id])?;

    for tag in tags {
        let tag_id = add_tag(conn, tag)?;
        conn.execute(
            "INSERT OR IGNORE INTO command_tags (command_id, tag_id) VALUES (?, ?)",
            (&cmd_id, tag_id),
        )?;
    }

    Ok(())
}

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
    use crate::storage::commands::hash_command;

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

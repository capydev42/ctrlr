use rusqlite::Connection;

pub fn hash_command(text: &str) -> String {

    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    std::hash::Hash::hash(&text, &mut hasher);
    format!("{:x}", std::hash::Hasher::finish(&hasher))
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

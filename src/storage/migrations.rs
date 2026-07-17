use rusqlite::{Connection, Transaction, TransactionBehavior, params};
use std::collections::BTreeMap;

use crate::hash::{hash_command, normalize};

pub const SCHEMA_VERSION: i64 = 1;

struct Row {
    id: String,
    text: String,
    favorite: i64,
    last_used: Option<i64>,
    use_count: i64,
    created_at: Option<i64>,
}

/// Returns 0 for a database that predates versioning, and for anything
/// unreadable — a migration that needlessly re-runs is harmless (every step is
/// idempotent), whereas panicking here would take the whole app down.
fn read_schema_version(conn: &Connection) -> i64 {
    conn.query_row(
        "SELECT value FROM settings WHERE key = 'schema_version'",
        [],
        |row| row.get::<_, String>(0),
    )
    .ok()
    .and_then(|v| v.parse::<i64>().ok())
    .unwrap_or(0)
}

pub fn needs_migration(conn: &Connection) -> bool {
    read_schema_version(conn) < SCHEMA_VERSION
}

/// Brings the database up to [`SCHEMA_VERSION`] in a single transaction.
///
/// Any error rolls everything back and leaves the version unstamped, so the
/// next launch retries against untouched data.
pub fn run_migrations(conn: &mut Connection) -> rusqlite::Result<()> {
    if !needs_migration(conn) {
        return Ok(());
    }

    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    migrate_v1_normalize_command_ids(&tx)?;
    tx.execute(
        "INSERT OR REPLACE INTO settings (key, value) VALUES ('schema_version', ?)",
        params![SCHEMA_VERSION.to_string()],
    )?;
    tx.commit()
}

/// Re-keys every command to `hash_command(text)` and merges the rows that
/// earlier versions split across the raw and normalized hashes.
fn migrate_v1_normalize_command_ids(tx: &Transaction) -> rusqlite::Result<()> {
    let rows: Vec<Row> = {
        let mut stmt = tx
            .prepare("SELECT id, text, favorite, last_used, use_count, created_at FROM commands")?;
        let mapped = stmt.query_map([], |row| {
            Ok(Row {
                id: row.get(0)?,
                text: row.get(1)?,
                favorite: row.get::<_, Option<i64>>(2)?.unwrap_or(0),
                last_used: row.get(3)?,
                use_count: row.get::<_, Option<i64>>(4)?.unwrap_or(0),
                created_at: row.get(5)?,
            })
        })?;
        mapped.collect::<rusqlite::Result<Vec<Row>>>()?
    };

    // Grouped in Rust, never SQL: SQLite's lower()/trim() are ASCII- and
    // space-only, so they would disagree with the Rust-side canonical hash and
    // leave rows that violate the invariant this migration establishes.
    let mut groups: BTreeMap<String, Vec<Row>> = BTreeMap::new();
    for row in rows {
        groups.entry(normalize(&row.text)).or_default().push(row);
    }

    for (norm, rows) in groups {
        let canonical_id = hash_command(&norm);

        if rows.len() == 1 && rows[0].id == canonical_id {
            continue;
        }

        let favorite = rows.iter().any(|r| r.favorite != 0);
        let use_count: i64 = rows.iter().map(|r| r.use_count).sum();
        let last_used = rows.iter().filter_map(|r| r.last_used).max();
        let created_at = rows.iter().filter_map(|r| r.created_at).min();

        // Keep the user's original casing: the canonical row's text if one is
        // already correctly keyed, else the oldest, matching how
        // history::deduplicate keeps the first occurrence.
        let text = rows
            .iter()
            .find(|r| r.id == canonical_id)
            .or_else(|| {
                rows.iter()
                    .min_by(|a, b| a.created_at.cmp(&b.created_at).then(a.id.cmp(&b.id)))
            })
            .map(|r| r.text.clone())
            .unwrap_or_else(|| norm.clone());

        // Written before any delete, and an upsert rather than INSERT OR
        // REPLACE: foreign keys are live, so REPLACE would delete the
        // conflicting row and cascade the links we are about to move.
        tx.execute(
            "INSERT INTO commands (id, text, favorite, last_used, use_count, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(id) DO UPDATE SET
                 text = excluded.text,
                 favorite = excluded.favorite,
                 last_used = excluded.last_used,
                 use_count = excluded.use_count,
                 created_at = excluded.created_at",
            params![
                &canonical_id,
                &text,
                favorite as i64,
                last_used,
                use_count,
                created_at
            ],
        )?;

        for old in rows.iter().filter(|r| r.id != canonical_id) {
            // Links move before the row is deleted: the cascade would otherwise
            // take them with it. OR IGNORE absorbs the composite-PK collisions,
            // since both rows may carry the same tag or sit in the same
            // collection.
            tx.execute(
                "INSERT OR IGNORE INTO command_tags (command_id, tag_id)
                 SELECT ?1, tag_id FROM command_tags WHERE command_id = ?2",
                params![&canonical_id, &old.id],
            )?;
            tx.execute("DELETE FROM command_tags WHERE command_id = ?", [&old.id])?;

            tx.execute(
                "INSERT OR IGNORE INTO command_collections (command_id, collection_id)
                 SELECT ?1, collection_id FROM command_collections WHERE command_id = ?2",
                params![&canonical_id, &old.id],
            )?;
            tx.execute(
                "DELETE FROM command_collections WHERE command_id = ?",
                [&old.id],
            )?;

            tx.execute("DELETE FROM commands WHERE id = ?", [&old.id])?;
        }
    }

    // rusqlite enables foreign_keys, so ctrlr itself cannot leave an orphan.
    // The sqlite3 CLI defaults it OFF, so a hand-edited database still can.
    tx.execute(
        "DELETE FROM command_tags WHERE command_id NOT IN (SELECT id FROM commands)",
        [],
    )?;
    tx.execute(
        "DELETE FROM command_collections WHERE command_id NOT IN (SELECT id FROM commands)",
        [],
    )?;
    tx.execute(
        "DELETE FROM command_collections WHERE collection_id NOT IN (SELECT id FROM collections)",
        [],
    )?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hash::sha1_hex;
    use crate::storage::init_db_with_conn;

    fn test_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        init_db_with_conn(&conn).unwrap();
        conn
    }

    fn insert_command(conn: &Connection, id: &str, text: &str, favorite: i64, use_count: i64) {
        conn.execute(
            "INSERT INTO commands (id, text, favorite, use_count) VALUES (?, ?, ?, ?)",
            params![id, text, favorite, use_count],
        )
        .unwrap();
    }

    fn command_count(conn: &Connection) -> i64 {
        conn.query_row("SELECT COUNT(*) FROM commands", [], |r| r.get(0))
            .unwrap()
    }

    #[test]
    fn test_fresh_db_is_stamped() {
        let mut conn = test_conn();
        run_migrations(&mut conn).unwrap();
        assert_eq!(read_schema_version(&conn), SCHEMA_VERSION);
    }

    #[test]
    fn test_migration_is_idempotent() {
        let mut conn = test_conn();
        insert_command(&conn, &sha1_hex("Git Status"), "Git Status", 1, 3);

        run_migrations(&mut conn).unwrap();
        let after_first: i64 = conn
            .query_row("SELECT use_count FROM commands", [], |r| r.get(0))
            .unwrap();

        run_migrations(&mut conn).unwrap();
        let after_second: i64 = conn
            .query_row("SELECT use_count FROM commands", [], |r| r.get(0))
            .unwrap();

        assert_eq!(command_count(&conn), 1);
        assert_eq!(after_first, after_second, "must not re-sum on re-run");
    }

    #[test]
    fn test_merges_raw_and_normalized_rows() {
        let mut conn = test_conn();

        // The exact state ctrlr produced: bootstrap wrote the normalized row,
        // then favoriting wrote a raw-hashed shadow of the same command.
        let normalized_id = hash_command("Git Status");
        let raw_id = sha1_hex("Git Status");
        assert_ne!(normalized_id, raw_id);

        conn.execute(
            "INSERT INTO commands (id, text, favorite, last_used, use_count, created_at)
             VALUES (?, ?, 0, NULL, 0, 100)",
            params![&normalized_id, "Git Status"],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO commands (id, text, favorite, last_used, use_count, created_at)
             VALUES (?, ?, 1, 500, 3, 200)",
            params![&raw_id, "Git Status"],
        )
        .unwrap();

        let git_tag = crate::storage::tags::add_tag(&conn, "git").unwrap();
        let vcs_tag = crate::storage::tags::add_tag(&conn, "vcs").unwrap();
        // Both rows carry "git" -> forces the (command_id, tag_id) collision.
        for id in [&normalized_id, &raw_id] {
            conn.execute(
                "INSERT INTO command_tags (command_id, tag_id) VALUES (?, ?)",
                params![id, git_tag],
            )
            .unwrap();
        }
        conn.execute(
            "INSERT INTO command_tags (command_id, tag_id) VALUES (?, ?)",
            params![&raw_id, vcs_tag],
        )
        .unwrap();

        let col_id = crate::storage::collections::create_collection(&conn, "work").unwrap();
        // Both rows in the same collection -> forces the link collision.
        for id in [&normalized_id, &raw_id] {
            conn.execute(
                "INSERT INTO command_collections (command_id, collection_id) VALUES (?, ?)",
                params![id, &col_id],
            )
            .unwrap();
        }

        run_migrations(&mut conn).unwrap();

        assert_eq!(command_count(&conn), 1);

        let (id, text, favorite, last_used, use_count, created_at): (
            String,
            String,
            i64,
            Option<i64>,
            i64,
            i64,
        ) = conn
            .query_row(
                "SELECT id, text, favorite, last_used, use_count, created_at FROM commands",
                [],
                |r| {
                    Ok((
                        r.get(0)?,
                        r.get(1)?,
                        r.get(2)?,
                        r.get(3)?,
                        r.get(4)?,
                        r.get(5)?,
                    ))
                },
            )
            .unwrap();

        assert_eq!(id, hash_command("git status"));
        assert_eq!(text, "Git Status", "original casing is preserved");
        assert_eq!(favorite, 1, "favorite is OR'd");
        assert_eq!(last_used, Some(500), "last_used is maxed");
        assert_eq!(use_count, 3, "use_count is summed");
        assert_eq!(created_at, 100, "created_at is minned");

        let tag_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM command_tags", [], |r| r.get(0))
            .unwrap();
        assert_eq!(tag_count, 2, "git collision absorbed, vcs carried over");

        let link_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM command_collections", [], |r| r.get(0))
            .unwrap();
        assert_eq!(link_count, 1, "duplicate collection link absorbed");

        let orphans: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM command_tags WHERE command_id != ?",
                [&id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(orphans, 0);
    }

    #[test]
    fn test_merges_rows_with_differing_whitespace() {
        let mut conn = test_conn();
        // Different `text`, same canonical id: grouping by text would produce
        // two groups targeting one id and blow up on the primary key.
        insert_command(&conn, &sha1_hex("ls -la "), "ls -la ", 0, 1);
        insert_command(&conn, &sha1_hex("ls -la"), "ls -la", 0, 2);

        run_migrations(&mut conn).unwrap();

        assert_eq!(command_count(&conn), 1);
        let (id, use_count): (String, i64) = conn
            .query_row("SELECT id, use_count FROM commands", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .unwrap();
        assert_eq!(id, hash_command("ls -la"));
        assert_eq!(use_count, 3);
    }

    #[test]
    fn test_leaves_clean_db_untouched() {
        let mut conn = test_conn();
        insert_command(&conn, &hash_command("cargo build"), "cargo build", 1, 7);

        run_migrations(&mut conn).unwrap();

        let (id, use_count, favorite): (String, i64, i64) = conn
            .query_row("SELECT id, use_count, favorite FROM commands", [], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?))
            })
            .unwrap();
        assert_eq!(id, hash_command("cargo build"));
        assert_eq!(use_count, 7, "already-canonical rows must not be re-summed");
        assert_eq!(favorite, 1);
    }

    #[test]
    fn test_deletes_orphan_links() {
        let mut conn = test_conn();
        insert_command(&conn, &hash_command("ls"), "ls", 0, 1);
        let tag = crate::storage::tags::add_tag(&conn, "x").unwrap();

        // Only reachable via a tool that leaves foreign_keys OFF (the sqlite3
        // CLI does); rusqlite would reject this insert outright.
        conn.pragma_update(None, "foreign_keys", false).unwrap();
        conn.execute(
            "INSERT INTO command_tags (command_id, tag_id) VALUES ('bogus-id', ?)",
            [tag],
        )
        .unwrap();
        conn.pragma_update(None, "foreign_keys", true).unwrap();

        run_migrations(&mut conn).unwrap();

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM command_tags", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn test_migration_does_not_touch_collection_ids() {
        let mut conn = test_conn();
        let id = crate::storage::collections::create_collection(&conn, "Work").unwrap();
        assert_eq!(id, sha1_hex("Work"));

        run_migrations(&mut conn).unwrap();

        let stored: String = conn
            .query_row("SELECT id FROM collections", [], |r| r.get(0))
            .unwrap();
        assert_eq!(stored, sha1_hex("Work"), "collection ids must stay raw");
    }

    #[test]
    fn test_failure_rolls_back() {
        let mut conn = test_conn();
        insert_command(&conn, &sha1_hex("Git Status"), "Git Status", 1, 3);
        conn.execute("DROP TABLE settings", []).unwrap();

        let result = run_migrations(&mut conn);
        assert!(result.is_err(), "stamping the version must fail");

        // The merge is rolled back with it, so the next launch retries clean.
        let id: String = conn
            .query_row("SELECT id FROM commands", [], |r| r.get(0))
            .unwrap();
        assert_eq!(id, sha1_hex("Git Status"), "data must be untouched");
    }
}

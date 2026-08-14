//! Per-execution records: where a command ran, when, and how it exited.
//!
//! Rows here are pure metadata layered on top of `commands`, exactly like tags
//! and collections. The history file remains the source of truth for command
//! text.

use rusqlite::{Connection, params};
use std::collections::HashMap;

use crate::hash::hash_command;
use crate::history::runs::RunEntry;

/// What the details panel shows for a single command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunSummary {
    pub total: i32,
    pub last_exit: Option<i32>,
    pub last_run: Option<i64>,
    /// Directories the command ran in, most frequent first.
    pub top_dirs: Vec<(String, i32)>,
}

/// Writes a batch of runs in one transaction.
///
/// The parent `commands` row is inserted first: foreign keys are live, and
/// `OR IGNORE` absorbs a primary-key collision but not a foreign-key violation,
/// so a run whose command is missing would fail rather than be skipped. The
/// insert deliberately does not touch `use_count` or `last_used` — those are
/// owned by [`crate::storage::commands`] and the history import.
pub fn record_runs(conn: &mut Connection, entries: &[RunEntry]) -> rusqlite::Result<()> {
    if entries.is_empty() {
        return Ok(());
    }

    let tx = conn.transaction()?;
    {
        let mut parent = tx.prepare("INSERT OR IGNORE INTO commands (id, text) VALUES (?1, ?2)")?;
        let mut run = tx.prepare(
            "INSERT INTO command_runs (command_id, cwd, exit_code, ran_at, host)
             VALUES (?1, ?2, ?3, ?4, ?5)",
        )?;

        for entry in entries {
            let id = hash_command(&entry.command);
            parent.execute(params![&id, &entry.command])?;
            run.execute(params![
                &id,
                &entry.cwd,
                entry.exit_code,
                entry.ran_at,
                &entry.host
            ])?;
        }
    }
    tx.commit()
}

/// How often each command ran in `cwd`, keyed by command id.
///
/// One grouped query rather than a lookup per command: the list can hold every
/// command in the user's history, and this runs on every launch.
pub fn runs_in_dir(conn: &Connection, cwd: &str) -> HashMap<String, i32> {
    let mut stmt = match conn
        .prepare("SELECT command_id, COUNT(*) FROM command_runs WHERE cwd = ?1 GROUP BY command_id")
    {
        Ok(stmt) => stmt,
        Err(_) => return HashMap::new(),
    };

    stmt.query_map([cwd], |row| Ok((row.get(0)?, row.get(1)?)))
        .map(|rows| rows.flatten().collect())
        .unwrap_or_default()
}

/// Run statistics for one command, or `None` when it has never been recorded.
pub fn run_summary(conn: &Connection, command_id: &str) -> Option<RunSummary> {
    let (total, last_run): (i32, Option<i64>) = conn
        .query_row(
            "SELECT COUNT(*), MAX(ran_at) FROM command_runs WHERE command_id = ?1",
            [command_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .ok()?;

    if total == 0 {
        return None;
    }

    // Ordered by id as well as time so a second run recorded within the same
    // second still resolves to the one written last.
    let last_exit: Option<i32> = conn
        .query_row(
            "SELECT exit_code FROM command_runs WHERE command_id = ?1
             ORDER BY ran_at DESC, id DESC LIMIT 1",
            [command_id],
            |row| row.get(0),
        )
        .ok()
        .flatten();

    let top_dirs = conn
        .prepare(
            "SELECT cwd, COUNT(*) AS n FROM command_runs WHERE command_id = ?1
             GROUP BY cwd ORDER BY n DESC, cwd ASC LIMIT 3",
        )
        .and_then(|mut stmt| {
            stmt.query_map([command_id], |row| Ok((row.get(0)?, row.get(1)?)))
                .map(|rows| rows.flatten().collect::<Vec<(String, i32)>>())
        })
        .unwrap_or_default();

    Some(RunSummary {
        total,
        last_exit,
        last_run,
        top_dirs,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        crate::storage::init_db_with_conn(&conn).unwrap();
        conn
    }

    fn entry(command: &str, cwd: &str, ran_at: i64, exit: Option<i32>) -> RunEntry {
        RunEntry {
            command: command.to_string(),
            cwd: cwd.to_string(),
            exit_code: exit,
            ran_at,
            host: Some("box".to_string()),
        }
    }

    #[test]
    fn test_record_runs_creates_parent_command() {
        let mut conn = test_conn();
        record_runs(&mut conn, &[entry("cargo test", "/tmp", 1, Some(0))]).unwrap();

        let text: String = conn
            .query_row(
                "SELECT text FROM commands WHERE id = ?",
                [hash_command("cargo test")],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(text, "cargo test");
    }

    #[test]
    fn test_record_runs_uses_normalized_id() {
        let mut conn = test_conn();
        record_runs(
            &mut conn,
            &[
                entry("Git Status", "/tmp", 1, Some(0)),
                entry("git status", "/tmp", 2, Some(0)),
            ],
        )
        .unwrap();

        let commands: i64 = conn
            .query_row("SELECT COUNT(*) FROM commands", [], |r| r.get(0))
            .unwrap();
        let runs: i64 = conn
            .query_row("SELECT COUNT(*) FROM command_runs", [], |r| r.get(0))
            .unwrap();
        assert_eq!(commands, 1, "casing variants must share one row");
        assert_eq!(runs, 2, "both executions are still recorded");
    }

    #[test]
    fn test_record_runs_keeps_existing_command_metadata() {
        let mut conn = test_conn();
        crate::storage::commands::update_favorite(&conn, "cargo test", true).unwrap();
        record_runs(&mut conn, &[entry("cargo test", "/tmp", 1, Some(0))]).unwrap();

        let favorite: i32 = conn
            .query_row(
                "SELECT favorite FROM commands WHERE id = ?",
                [hash_command("cargo test")],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(favorite, 1, "recording a run must not reset metadata");
    }

    #[test]
    fn test_record_runs_empty_batch() {
        let mut conn = test_conn();
        record_runs(&mut conn, &[]).unwrap();
        let runs: i64 = conn
            .query_row("SELECT COUNT(*) FROM command_runs", [], |r| r.get(0))
            .unwrap();
        assert_eq!(runs, 0);
    }

    #[test]
    fn test_deleting_a_command_cascades_its_runs() {
        let mut conn = test_conn();
        record_runs(&mut conn, &[entry("cargo test", "/tmp", 1, Some(0))]).unwrap();
        conn.execute(
            "DELETE FROM commands WHERE id = ?",
            [hash_command("cargo test")],
        )
        .unwrap();

        let runs: i64 = conn
            .query_row("SELECT COUNT(*) FROM command_runs", [], |r| r.get(0))
            .unwrap();
        assert_eq!(runs, 0);
    }

    #[test]
    fn test_runs_in_dir_counts_per_directory() {
        let mut conn = test_conn();
        record_runs(
            &mut conn,
            &[
                entry("cargo test", "/a", 1, Some(0)),
                entry("cargo test", "/a", 2, Some(0)),
                entry("cargo test", "/b", 3, Some(0)),
                entry("ls", "/a", 4, Some(0)),
            ],
        )
        .unwrap();

        let here = runs_in_dir(&conn, "/a");
        assert_eq!(here.get(&hash_command("cargo test")), Some(&2));
        assert_eq!(here.get(&hash_command("ls")), Some(&1));

        let there = runs_in_dir(&conn, "/b");
        assert_eq!(there.get(&hash_command("cargo test")), Some(&1));
        assert!(!there.contains_key(&hash_command("ls")));

        assert!(runs_in_dir(&conn, "/nowhere").is_empty());
    }

    #[test]
    fn test_run_summary() {
        let mut conn = test_conn();
        record_runs(
            &mut conn,
            &[
                entry("cargo test", "/a", 10, Some(0)),
                entry("cargo test", "/a", 20, Some(0)),
                entry("cargo test", "/b", 30, Some(101)),
            ],
        )
        .unwrap();

        let summary = run_summary(&conn, &hash_command("cargo test")).unwrap();
        assert_eq!(summary.total, 3);
        assert_eq!(summary.last_run, Some(30));
        assert_eq!(summary.last_exit, Some(101));
        assert_eq!(
            summary.top_dirs,
            vec![("/a".to_string(), 2), ("/b".to_string(), 1)]
        );
    }

    #[test]
    fn test_ingest_round_trip_from_a_log_file() {
        // The whole path a launch takes: claim the log the hooks wrote, resolve
        // the directories, store, then count them for the current one.
        let dir = tempfile::TempDir::new().unwrap();
        let log = dir.path().join("runs.log");
        let here = crate::history::runs::canonical_dir(&dir.path().to_string_lossy());
        std::fs::write(
            &log,
            format!(
                "v1\t100\t0\tbox\t{}\tcargo test\nv1\t101\t1\tbox\t{}\tcargo test\nv1\t102\t0\tbox\t/elsewhere\tls\n",
                dir.path().display(),
                dir.path().display()
            ),
        )
        .unwrap();

        let entries: Vec<RunEntry> = crate::history::runs::take_run_log(&log)
            .into_iter()
            .map(|mut e| {
                e.cwd = crate::history::runs::canonical_dir(&e.cwd);
                e
            })
            .collect();
        assert_eq!(entries.len(), 3);

        let mut conn = test_conn();
        record_runs(&mut conn, &entries).unwrap();

        let counts = runs_in_dir(&conn, &here);
        assert_eq!(counts.get(&hash_command("cargo test")), Some(&2));
        assert!(!counts.contains_key(&hash_command("ls")));
        assert!(
            std::fs::read_to_string(&log).unwrap().is_empty(),
            "the log is consumed"
        );
    }

    #[test]
    fn test_run_summary_unknown_command() {
        let conn = test_conn();
        assert!(run_summary(&conn, &hash_command("never run")).is_none());
    }
}

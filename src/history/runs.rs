//! Parser for the run log written by the shell integration.
//!
//! No shell records the working directory in its history file, so ctrlr cannot
//! recover it after the fact. The integration appends one line per command with
//! a shell builtin redirect — no process spawn — and this module drains it.
//!
//! Line format, tab separated:
//!
//! ```text
//! v1<TAB>epoch<TAB>exit<TAB>host<TAB>cwd<TAB>command
//! ```
//!
//! `command` is escaped by the hook (`\\`, `\n`, `\t`, `\r`) so a single line
//! always holds a single run, and is unescaped here.

use std::path::{Path, PathBuf};

/// One recorded execution. `command` is the raw text; the caller derives the id
/// through [`crate::hash::hash_command`] like every other write path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunEntry {
    pub command: String,
    pub cwd: String,
    pub exit_code: Option<i32>,
    pub ran_at: i64,
    pub host: Option<String>,
}

/// Discarded above this, unparsed: a user who never launches ctrlr would
/// otherwise accumulate an unbounded log.
const MAX_LOG_BYTES: u64 = 10 * 1024 * 1024;
const MAX_LINES: usize = 50_000;

/// Suffix of the staging file the log is renamed to before it is read.
const INGEST_SUFFIX: &str = ".ingest";

fn ingest_path(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(INGEST_SUFFIX);
    PathBuf::from(name)
}

/// Claims the run log and returns everything it held.
///
/// Rename first, then read: the hooks open the log per redirect and close it
/// again, so nothing holds a stale descriptor and the next append recreates the
/// original path. That closes the read/truncate race against other terminals
/// without needing a lock. A staging file left behind by a crashed run is
/// picked up on the next call.
pub fn take_run_log(path: &Path) -> Vec<RunEntry> {
    let staging = ingest_path(path);

    // Anything already staged predates this call and is drained first: renaming
    // over it would discard a batch a crashed ingest never got to write. The
    // live log then waits for the next launch.
    if !staging.exists() {
        if !path.exists() {
            return Vec::new();
        }
        if std::fs::rename(path, &staging).is_err() {
            return Vec::new();
        }
    }

    let entries = read_runs(&staging);
    let _ = std::fs::remove_file(&staging);
    recreate_secure(path);
    entries
}

/// Leaves an empty log behind with owner-only permissions.
///
/// The hooks set the mode once, when they create the file at shell startup.
/// After a drain their `>>` recreates it under the user's umask instead —
/// usually 0644 — so the file holding your command text and paths has to be
/// re-established here rather than left to them.
fn recreate_secure(path: &Path) {
    if path.exists() {
        return;
    }

    let mut options = std::fs::OpenOptions::new();
    options.create(true).append(true);

    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }

    let _ = options.open(path);
}

/// Resolves a directory the way both sides of a comparison must see it.
///
/// The shells log `$PWD`, which is the *logical* path and keeps symlinks
/// intact, while `std::env::current_dir` returns the physical one. Under a
/// symlinked directory an uncanonicalized compare matches nothing at all. A
/// directory that no longer exists keeps its recorded path rather than being
/// dropped.
pub fn canonical_dir(path: &str) -> String {
    std::fs::canonicalize(path)
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| path.to_string())
}

/// The directory ctrlr itself was launched from — ctrlr is a child of the
/// shell, so this is the shell's directory.
pub fn current_dir() -> Option<String> {
    std::env::current_dir()
        .ok()
        .map(|p| canonical_dir(&p.to_string_lossy()))
}

/// Parses a run log, skipping anything malformed.
///
/// Torn and partial lines are expected rather than exceptional: appends are
/// only atomic below `PIPE_BUF` on a local filesystem, and a shared home over
/// NFS gives no such guarantee.
pub fn read_runs(path: &Path) -> Vec<RunEntry> {
    let too_big = std::fs::metadata(path)
        .map(|m| m.len() > MAX_LOG_BYTES)
        .unwrap_or(false);
    if too_big {
        return Vec::new();
    }

    let Ok(content) = std::fs::read_to_string(path) else {
        return Vec::new();
    };

    content
        .lines()
        .take(MAX_LINES)
        .filter_map(parse_run_line)
        .collect()
}

fn parse_run_line(line: &str) -> Option<RunEntry> {
    let fields: Vec<&str> = line.splitn(6, '\t').collect();
    if fields.len() != 6 || fields[0] != "v1" {
        return None;
    }

    let ran_at = fields[1].parse::<i64>().ok()?;
    // An absent exit code is normal: the fallback bash path has no way to
    // report one. An unparseable one is treated the same way.
    let exit_code = fields[2].parse::<i32>().ok();
    let host = Some(fields[3])
        .filter(|h| !h.is_empty())
        .map(str::to_string);
    let cwd = fields[4];
    let command = unescape(fields[5]);

    if cwd.is_empty() || command.trim().is_empty() {
        return None;
    }

    Some(RunEntry {
        command,
        cwd: cwd.to_string(),
        exit_code,
        ran_at,
        host,
    })
}

/// Inverse of the hook's escaping. A trailing lone backslash is kept verbatim
/// rather than dropped, so a torn line never silently changes the command text.
fn unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();

    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('r') => out.push('\r'),
            Some('\\') => out.push('\\'),
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
            None => out.push('\\'),
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_parse_run_line_full() {
        let entry = parse_run_line("v1\t1700000000\t0\tbox\t/home/u/dev\tcargo test").unwrap();
        assert_eq!(entry.command, "cargo test");
        assert_eq!(entry.cwd, "/home/u/dev");
        assert_eq!(entry.exit_code, Some(0));
        assert_eq!(entry.ran_at, 1700000000);
        assert_eq!(entry.host.as_deref(), Some("box"));
    }

    #[test]
    fn test_parse_run_line_nonzero_exit() {
        let entry = parse_run_line("v1\t1700000000\t130\tbox\t/tmp\tsleep 9").unwrap();
        assert_eq!(entry.exit_code, Some(130));
    }

    #[test]
    fn test_parse_run_line_empty_host_is_none() {
        let entry = parse_run_line("v1\t1700000000\t0\t\t/tmp\tls").unwrap();
        assert!(entry.host.is_none());
    }

    #[test]
    fn test_parse_run_line_unparseable_exit_is_none() {
        let entry = parse_run_line("v1\t1700000000\t\tbox\t/tmp\tls").unwrap();
        assert!(entry.exit_code.is_none());
    }

    #[test]
    fn test_parse_run_line_keeps_tabs_in_command() {
        // Only the first five fields are split off; a tab the hook failed to
        // escape still leaves the command intact rather than truncating it.
        let entry = parse_run_line("v1\t1700000000\t0\tbox\t/tmp\techo a\tb").unwrap();
        assert_eq!(entry.command, "echo a\tb");
    }

    #[test]
    fn test_parse_run_line_unescapes() {
        let entry = parse_run_line("v1\t1700000000\t0\tbox\t/tmp\tprintf 'a\\nb'").unwrap();
        assert_eq!(entry.command, "printf 'a\nb'");
    }

    #[test]
    fn test_parse_run_line_escaped_backslash_survives() {
        // printf 'a\nb' typed with a literal backslash-n must not become a
        // newline: the hook doubles the backslash, and we halve it back.
        let entry = parse_run_line("v1\t1700000000\t0\tbox\t/tmp\tprintf 'a\\\\nb'").unwrap();
        assert_eq!(entry.command, "printf 'a\\nb'");
    }

    #[test]
    fn test_parse_run_line_wrong_version() {
        assert!(parse_run_line("v2\t1700000000\t0\tbox\t/tmp\tls").is_none());
    }

    #[test]
    fn test_parse_run_line_too_few_fields() {
        assert!(parse_run_line("v1\t1700000000\t0\t/tmp").is_none());
        assert!(parse_run_line("").is_none());
        assert!(parse_run_line("garbage").is_none());
    }

    #[test]
    fn test_parse_run_line_bad_timestamp() {
        assert!(parse_run_line("v1\tnope\t0\tbox\t/tmp\tls").is_none());
    }

    #[test]
    fn test_parse_run_line_empty_command_or_cwd() {
        assert!(parse_run_line("v1\t1700000000\t0\tbox\t/tmp\t   ").is_none());
        assert!(parse_run_line("v1\t1700000000\t0\tbox\t\tls").is_none());
    }

    #[test]
    fn test_unescape_trailing_backslash() {
        assert_eq!(unescape("echo \\"), "echo \\");
        assert_eq!(unescape("echo \\q"), "echo \\q");
    }

    #[test]
    fn test_read_runs_skips_bad_lines() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("runs.log");
        fs::write(
            &path,
            "v1\t1\t0\tbox\t/tmp\tls\ntorn line\nv1\t2\t1\tbox\t/tmp\tfalse\n",
        )
        .unwrap();

        let entries = read_runs(&path);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].command, "ls");
        assert_eq!(entries[1].exit_code, Some(1));
    }

    #[test]
    fn test_read_runs_empty_and_missing() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("runs.log");
        fs::write(&path, "").unwrap();
        assert!(read_runs(&path).is_empty());
        assert!(read_runs(&dir.path().join("nope.log")).is_empty());
    }

    #[test]
    fn test_read_runs_discards_oversized_log() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("runs.log");
        let line = "v1\t1\t0\tbox\t/tmp\tls\n";
        let repeats = (MAX_LOG_BYTES as usize / line.len()) + 2;
        fs::write(&path, line.repeat(repeats)).unwrap();

        assert!(read_runs(&path).is_empty());
    }

    #[test]
    fn test_take_run_log_consumes_the_file() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("runs.log");
        fs::write(&path, "v1\t1\t0\tbox\t/tmp\tls\n").unwrap();

        let entries = take_run_log(&path);
        assert_eq!(entries.len(), 1);
        assert!(!ingest_path(&path).exists(), "staging must be cleaned up");

        let left = std::fs::read_to_string(&path).unwrap();
        assert!(left.is_empty(), "log must be drained");
    }

    #[test]
    fn test_take_run_log_restores_owner_only_permissions() {
        // The hooks only set the mode when they create the file at shell
        // startup; after a drain their >> would recreate it under the umask.
        use std::os::unix::fs::PermissionsExt;

        let dir = TempDir::new().unwrap();
        let path = dir.path().join("runs.log");
        fs::write(&path, "v1\t1\t0\tbox\t/tmp\tls\n").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();

        take_run_log(&path);

        let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "run log holds command text and paths");
    }

    #[test]
    fn test_canonical_dir_resolves_symlinks() {
        let dir = TempDir::new().unwrap();
        let real = dir.path().join("real");
        let link = dir.path().join("link");
        fs::create_dir(&real).unwrap();
        std::os::unix::fs::symlink(&real, &link).unwrap();

        assert_eq!(
            canonical_dir(&link.to_string_lossy()),
            canonical_dir(&real.to_string_lossy()),
            "a symlinked cwd must resolve to the same key"
        );
    }

    #[test]
    fn test_canonical_dir_keeps_missing_paths() {
        assert_eq!(canonical_dir("/nope/gone"), "/nope/gone");
    }

    #[test]
    fn test_take_run_log_missing_file() {
        let dir = TempDir::new().unwrap();
        assert!(take_run_log(&dir.path().join("runs.log")).is_empty());
    }

    #[test]
    fn test_take_run_log_recovers_leftover_staging_file() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("runs.log");
        fs::write(ingest_path(&path), "v1\t1\t0\tbox\t/tmp\tls\n").unwrap();

        let entries = take_run_log(&path);
        assert_eq!(entries.len(), 1, "a crashed ingest must not lose data");
        assert!(!ingest_path(&path).exists());
    }

    #[test]
    fn test_take_run_log_prefers_staged_data() {
        // Staging already holds an earlier batch. Renaming over it would drop
        // that batch, so it is drained first and the live log stays put.
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("runs.log");
        fs::write(ingest_path(&path), "v1\t1\t0\tbox\t/tmp\told\n").unwrap();
        fs::write(&path, "v1\t2\t0\tbox\t/tmp\tnew\n").unwrap();

        let entries = take_run_log(&path);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].command, "old");
        assert!(path.exists(), "live log is claimed on the next call");
    }
}

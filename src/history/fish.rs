use super::HistoryEntry;
use std::collections::HashMap;
use std::path::Path;

/// Reads fish's `fish_history`: `- cmd:` per entry, an optional `  when:`,
/// an optional `  paths:` block. Both keys match at their exact indent, so a
/// path that reads like a key is not one.
///
/// Oldest first and more than one line per entry, so this reads forward and
/// reverses afterwards, like the PowerShell parser.
pub fn read_history(path: &Path) -> Vec<HistoryEntry> {
    let Ok(content) = std::fs::read_to_string(path) else {
        return Vec::new();
    };

    let mut parsed: Vec<(String, Option<i64>)> = Vec::new();

    for line in content.lines() {
        if let Some(value) = line.strip_prefix("- cmd:") {
            parsed.push((unescape(value.trim_start()), None));
        } else if let Some(value) = line.strip_prefix("  when:")
            && let Some(last) = parsed.last_mut()
        {
            last.1 = value.trim().parse::<i64>().ok();
        }
        // Anything else belongs to a `paths:` block.
    }

    let mut entries: Vec<HistoryEntry> = Vec::new();
    let mut seen: HashMap<String, usize> = HashMap::new();

    for (command, timestamp) in parsed.iter().rev() {
        let trimmed = command.trim();
        if trimmed.is_empty() {
            continue;
        }

        let key = trimmed.to_lowercase();
        if let Some(idx) = seen.get(&key) {
            entries[*idx].use_count += 1;
        } else {
            seen.insert(key, entries.len());
            entries.push(HistoryEntry {
                command: trimmed.to_string(),
                timestamp: *timestamp,
                use_count: 1,
            });
        }
    }

    entries
}

/// fish escapes two characters and no others: `\\` is a backslash, `\n` a
/// newline. A backslash before anything else is not an escape.
fn unescape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut chars = value.chars();

    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('\\') => out.push('\\'),
            Some('n') => out.push('\n'),
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
    use std::io::Write;
    use tempfile::NamedTempFile;

    /// Byte-for-byte what fish 3.7.0 wrote in a pty. The parser this replaced
    /// expected `cmd "foo"`, a format fish never writes.
    const REAL_SAMPLE: &str = r#"- cmd: echo hello
  when: 1790193955
- cmd: git commit -m "fix: thing"
  when: 1790193956
- cmd: echo a:b
  when: 1790193957
- cmd: cat /etc/hostname
  when: 1790193958
  paths:
    - /etc/hostname
- cmd: for i in 1 2 3\necho $i\nend
  when: 1790193958
- cmd: echo back\\\\slash
  when: 1790193959
- cmd: echo "Gruesse aus Muenchen -- ae oe ue"
  when: 1790193960
- cmd: echo hello
  when: 1790193961
- cmd: echo 'single #quoted'
  when: 1790193962
- cmd: exit
  when: 1790193962
"#;

    fn create_temp_file(content: &str) -> NamedTempFile {
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(content.as_bytes()).unwrap();
        file.flush().unwrap();
        file
    }

    fn read(content: &str) -> Vec<HistoryEntry> {
        let file = create_temp_file(content);
        read_history(file.path())
    }

    #[test]
    fn test_read_history_empty_file() {
        assert!(read("").is_empty());
    }

    #[test]
    fn test_read_history_missing_file() {
        assert!(read_history(Path::new("/nonexistent/fish_history")).is_empty());
    }

    /// The point of the change: this file used to yield nothing at all.
    #[test]
    fn test_read_history_parses_the_real_format() {
        let entries = read(REAL_SAMPLE);
        assert_eq!(entries.len(), 9, "ten entries, `echo hello` twice");
        assert_eq!(entries[0].command, "exit");
        assert_eq!(entries[0].timestamp, Some(1790193962));
    }

    #[test]
    fn test_read_history_is_newest_first() {
        let entries = read(REAL_SAMPLE);
        let order: Vec<&str> = entries.iter().map(|e| e.command.as_str()).collect();
        assert_eq!(order[1], "echo 'single #quoted'");
        assert_eq!(order[8], "git commit -m \"fix: thing\"");
    }

    /// The other shells fold repeats into a count; this one did not.
    #[test]
    fn test_read_history_counts_duplicates() {
        let entries = read(REAL_SAMPLE);
        let hello = entries.iter().find(|e| e.command == "echo hello").unwrap();
        assert_eq!(hello.use_count, 2);
        assert_eq!(hello.timestamp, Some(1790193961), "newest occurrence wins");
    }

    #[test]
    fn test_read_history_unescapes_a_multiline_command() {
        let entries = read(REAL_SAMPLE);
        let multi = entries
            .iter()
            .find(|e| e.command.contains("for i"))
            .unwrap();
        assert_eq!(multi.command, "for i in 1 2 3\necho $i\nend");
    }

    #[test]
    fn test_read_history_unescapes_backslashes() {
        let entries = read(REAL_SAMPLE);
        assert!(entries.iter().any(|e| e.command == r"echo back\\slash"));
    }

    /// `paths:` items are indented list entries, not commands.
    #[test]
    fn test_read_history_skips_the_paths_block() {
        let entries = read(REAL_SAMPLE);
        assert!(entries.iter().all(|e| e.command != "/etc/hostname"));
        let cat = entries
            .iter()
            .find(|e| e.command == "cat /etc/hostname")
            .unwrap();
        assert_eq!(cat.timestamp, Some(1790193958));
    }

    /// A path with a space is quoted, and is still not a command.
    #[test]
    fn test_read_history_skips_a_quoted_path() {
        let entries = read(
            "- cmd: ls /etc/hosts '/tmp/a b'\n  when: 1700000000\n  paths:\n    - /etc/hosts\n    - '/tmp/a b'\n",
        );
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].command, "ls /etc/hosts '/tmp/a b'");
    }

    /// The value is raw, not quoted, so a colon is just a colon.
    #[test]
    fn test_read_history_keeps_colons_in_the_command() {
        let entries = read("- cmd: echo -- --cmd: not-a-key\n  when: 1700000000\n");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].command, "echo -- --cmd: not-a-key");
    }

    #[test]
    fn test_read_history_without_a_when() {
        let entries = read("- cmd: echo bare\n");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].timestamp, None);
    }

    #[test]
    fn test_read_history_with_an_unparseable_when() {
        let entries = read("- cmd: echo bad\n  when: not-a-number\n");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].timestamp, None);
    }

    #[test]
    fn test_unescape_leaves_a_non_escape_alone() {
        assert_eq!(unescape(r"echo a\tb"), r"echo a\tb");
        assert_eq!(unescape(r"echo trailing\"), r"echo trailing\");
    }

    /// A `\n` the user typed is written `\\n`, so order matters.
    #[test]
    fn test_unescape_round_trips_a_literal_backslash_n() {
        assert_eq!(unescape(r"echo 'a\\nb'"), r"echo 'a\nb'");
    }
}

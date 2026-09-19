use super::HistoryEntry;
use std::collections::HashMap;
use std::path::Path;

/// Reads PSReadLine's `ConsoleHost_history.txt`.
///
/// Unlike the other three parsers this folds before it reverses: PSReadLine
/// writes a multi-line command as one physical line per input line, with a
/// trailing backtick on all but the last. Reversing first would mean
/// unfolding.
pub fn read_history(path: &Path) -> Vec<HistoryEntry> {
    let Ok(content) = std::fs::read_to_string(path) else {
        return Vec::new();
    };

    let mut commands: Vec<String> = Vec::new();
    let mut pending: Option<String> = None;

    for line in content.trim_start_matches('\u{feff}').lines() {
        let mut joined = match pending.take() {
            Some(mut started) => {
                started.push('\n');
                started
            }
            None => String::new(),
        };
        joined.push_str(line);

        if continues(&joined) {
            // Drop the marker itself: it is how PSReadLine spells the line
            // break, not something the user typed.
            joined.pop();
            pending = Some(joined);
        } else {
            commands.push(joined);
        }
    }
    // A file cut off mid-command still has a usable last entry.
    if let Some(unterminated) = pending {
        commands.push(unterminated);
    }

    let mut entries: Vec<HistoryEntry> = Vec::new();
    let mut seen: HashMap<String, usize> = HashMap::new();

    for command in commands.iter().rev() {
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
                timestamp: None,
                use_count: 1,
            });
        }
    }

    entries
}

/// Counting parity, not just "ends with a backtick": a command ending in an
/// escaped backtick is written with two, and is complete.
fn continues(line: &str) -> bool {
    line.chars().rev().take_while(|c| *c == '`').count() % 2 == 1
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn create_temp_file(content: &str) -> NamedTempFile {
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(content.as_bytes()).unwrap();
        file.flush().unwrap();
        file
    }

    #[test]
    fn test_read_history_empty_file() {
        let file = create_temp_file("");
        assert!(read_history(file.path()).is_empty());
    }

    #[test]
    fn test_read_history_nonexistent_file() {
        assert!(read_history(Path::new("/nonexistent/path")).is_empty());
    }

    #[test]
    fn test_read_history_newest_first() {
        let file = create_temp_file("Get-Date\nGet-Location\nGet-Process\n");
        let entries = read_history(file.path());
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].command, "Get-Process");
        assert_eq!(entries[2].command, "Get-Date");
    }

    /// PSReadLine writes duplicates out; ctrlr counts them like the other
    /// shells.
    #[test]
    fn test_read_history_counts_duplicates() {
        let file = create_temp_file("Get-Date\nls\nget-date\n");
        let entries = read_history(file.path());
        assert_eq!(entries.len(), 2);
        let dates = entries.iter().find(|e| e.command == "get-date").unwrap();
        assert_eq!(dates.use_count, 2, "matching is case-insensitive");
    }

    /// The exact shape PSReadLine wrote for `if ($true) { ... }`.
    #[test]
    fn test_multi_line_commands_are_folded() {
        let file = create_temp_file("if ($true) {`\n  echo eins`\n  echo zwei`\n}\nexit\n");
        let entries = read_history(file.path());
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].command, "exit");
        assert_eq!(
            entries[1].command,
            "if ($true) {\n  echo eins\n  echo zwei\n}"
        );
    }

    /// Two trailing backticks are an escaped one, not a continuation.
    #[test]
    fn test_escaped_backtick_does_not_continue() {
        let file = create_temp_file("echo backtick``\nexit\n");
        let entries = read_history(file.path());
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].command, "exit");
        assert_eq!(entries[1].command, "echo backtick``");
    }

    #[test]
    fn test_unterminated_continuation_is_kept() {
        let file = create_temp_file("echo eins`\n");
        let entries = read_history(file.path());
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].command, "echo eins");
    }

    #[test]
    fn test_leading_bom_is_stripped() {
        let file = create_temp_file("\u{feff}Get-Date\n");
        let entries = read_history(file.path());
        assert_eq!(entries[0].command, "Get-Date");
    }

    #[test]
    fn test_crlf_endings() {
        let file = create_temp_file("Get-Date\r\nGet-Location\r\n");
        let entries = read_history(file.path());
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].command, "Get-Location");
    }

    #[test]
    fn test_non_ascii_survives() {
        let file = create_temp_file("echo 'umlaut: äöü'\n");
        let entries = read_history(file.path());
        assert_eq!(entries[0].command, "echo 'umlaut: äöü'");
    }
}

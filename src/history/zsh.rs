use super::HistoryEntry;
use std::path::Path;

pub fn read_history(path: &Path) -> Vec<HistoryEntry> {
    let mut entries = Vec::new();

    if !path.exists() {
        return entries;
    }

    if let Ok(content) = std::fs::read_to_string(path) {
        for line in content.lines().rev() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            if let Some(cmd) = parse_zsh_line(line) {
                entries.push(cmd);
            }
        }
    }

    entries
}

fn parse_zsh_line(line: &str) -> Option<HistoryEntry> {
    if !line.starts_with(':') {
        return Some(HistoryEntry {
            command: line.to_string(),
            timestamp: None,
            use_count: 1,
        });
    }

    let parts: Vec<&str> = line.splitn(2, ';').collect();
    if parts.len() != 2 {
        return Some(HistoryEntry {
            command: line.to_string(),
            timestamp: None,
            use_count: 1,
        });
    }

    let header = parts[0];
    let command = parts[1].to_string();

    let timestamp = header.split(':').nth(1).and_then(|t| t.parse::<i64>().ok());

    Some(HistoryEntry {
        command,
        timestamp,
        use_count: 1,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_parse_zsh_line_simple() {
        let entry = parse_zsh_line("ls -la").unwrap();
        assert_eq!(entry.command, "ls -la");
        assert!(entry.timestamp.is_none());
        assert_eq!(entry.use_count, 1);
    }

    #[test]
    fn test_parse_zsh_line_with_timestamp() {
        let entry = parse_zsh_line(":1700000000;git commit -m 'fix'").unwrap();
        assert_eq!(entry.command, "git commit -m 'fix'");
        assert_eq!(entry.timestamp, Some(1700000000));
        assert_eq!(entry.use_count, 1);
    }

    #[test]
    fn test_parse_zsh_line_invalid_timestamp() {
        let entry = parse_zsh_line(":abc;ls").unwrap();
        assert_eq!(entry.command, "ls");
        assert!(entry.timestamp.is_none());
        assert_eq!(entry.use_count, 1);
    }

    #[test]
    fn test_parse_zsh_line_no_semicolon() {
        let entry = parse_zsh_line(":1700000000").unwrap();
        assert_eq!(entry.command, ":1700000000");
        assert!(entry.timestamp.is_none());
        assert_eq!(entry.use_count, 1);
    }

    #[test]
    fn test_parse_zsh_line_empty_command() {
        let entry = parse_zsh_line(":1700000000;").unwrap();
        assert_eq!(entry.command, "");
        assert_eq!(entry.timestamp, Some(1700000000));
        assert_eq!(entry.use_count, 1);
    }

    #[test]
    fn test_parse_zsh_line_double_semicolon() {
        let entry = parse_zsh_line(":1700000000;;ls").unwrap();
        assert_eq!(entry.command, ";ls");
        assert_eq!(entry.timestamp, Some(1700000000));
        assert_eq!(entry.use_count, 1);
    }

    #[test]
    fn test_parse_zsh_line_special_chars() {
        let entry = parse_zsh_line(":1700000000;echo $HOME/.config").unwrap();
        assert_eq!(entry.command, "echo $HOME/.config");
        assert_eq!(entry.timestamp, Some(1700000000));
        assert_eq!(entry.use_count, 1);
    }

    #[test]
    fn test_read_history_empty_file() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join(".zsh_history");
        fs::write(&path, "").unwrap();
        let entries = read_history(&path);
        assert!(entries.is_empty());
    }

    #[test]
    fn test_read_history_with_entries() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join(".zsh_history");
        let content = ":1700000000;ls\n:1700000001;pwd\n";
        fs::write(&path, content).unwrap();
        let entries = read_history(&path);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].command, "pwd"); // newest first (reversed)
        assert_eq!(entries[0].timestamp, Some(1700000001));
        assert_eq!(entries[1].command, "ls");
        assert_eq!(entries[1].timestamp, Some(1700000000));
    }

    #[test]
    fn test_read_history_nonexistent_file() {
        let entries = read_history(std::path::Path::new("/nonexistent/path"));
        assert!(entries.is_empty());
    }
}

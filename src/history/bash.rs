use super::HistoryEntry;
use std::path::Path;

pub fn read_history(path: &Path) -> Vec<HistoryEntry> {
    let mut entries = Vec::new();

    if !path.exists() {
        return entries;
    }

    if let Ok(content) = std::fs::read_to_string(path) {
        for line in content.lines() {
            let trimmed = line.trim();
            if !trimmed.is_empty() {
                entries.push(HistoryEntry {
                    command: trimmed.to_string(),
                    timestamp: None,
                });
            }
        }
    }

    entries
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
        let entries = read_history(file.path());
        assert!(entries.is_empty());
    }

    #[test]
    fn test_read_history_single_command() {
        let file = create_temp_file("git status\n");
        let entries = read_history(file.path());
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].command, "git status");
    }

    #[test]
    fn test_read_history_multiple_commands() {
        let content = "ls -la\npwd\ngit log\n";
        let file = create_temp_file(content);
        let entries = read_history(file.path());
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].command, "ls -la");
        assert_eq!(entries[1].command, "pwd");
        assert_eq!(entries[2].command, "git log");
    }

    #[test]
    fn test_read_history_ignores_empty_lines() {
        let content = "\nls\n\npwd\n\n";
        let file = create_temp_file(content);
        let entries = read_history(file.path());
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].command, "ls");
        assert_eq!(entries[1].command, "pwd");
    }

    #[test]
    fn test_read_history_trims_whitespace() {
        let content = "  ls -la  \n";
        let file = create_temp_file(content);
        let entries = read_history(file.path());
        assert_eq!(entries[0].command, "ls -la");
    }

    #[test]
    fn test_read_history_preserves_spaces_in_commands() {
        let content = "echo 'hello world'\n";
        let file = create_temp_file(content);
        let entries = read_history(file.path());
        assert_eq!(entries[0].command, "echo 'hello world'");
    }

    #[test]
    fn test_read_history_nonexistent_file() {
        let entries = read_history(std::path::Path::new("/nonexistent/path"));
        assert!(entries.is_empty());
    }
}

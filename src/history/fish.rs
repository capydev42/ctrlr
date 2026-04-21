use super::HistoryEntry;
use std::path::Path;

pub fn read_history(path: &Path) -> Vec<HistoryEntry> {
    let mut entries = Vec::new();

    if !path.exists() {
        return entries;
    }

    if let Ok(content) = std::fs::read_to_string(path) {
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            if let Some(cmd) = parse_fish_line(line) {
                entries.push(cmd);
            }
        }
    }

    entries
}

fn parse_fish_line(line: &str) -> Option<HistoryEntry> {
    if line.starts_with('-') {
        return parse_fish_yaml_line(line);
    }

    if let Some(stripped) = line.strip_prefix("cmd ") {
        let cmd = stripped
            .strip_prefix('"')
            .and_then(|s| s.strip_suffix('"'))
            .unwrap_or(stripped)
            .to_string();

        if !cmd.is_empty() {
            return Some(HistoryEntry {
                command: cmd,
                timestamp: None,
            });
        }
    }

    None
}

fn parse_fish_yaml_line(line: &str) -> Option<HistoryEntry> {
    let mut command = None;
    let mut timestamp = None;

    for part in line.split('\n') {
        let part = part.trim();
        if part.starts_with("cmd ") {
            let cmd = part
                .strip_prefix("cmd ")
                .and_then(|s| s.strip_prefix('"'))
                .and_then(|s| s.strip_suffix('"'))
                .unwrap_or("")
                .to_string();
            if !cmd.is_empty() {
                command = Some(cmd);
            }
        } else if part.starts_with("when ") {
            let ts = part
                .strip_prefix("when ")
                .and_then(|s| s.parse::<i64>().ok());
            timestamp = ts;
        }
    }

    command.map(|cmd| HistoryEntry {
        command: cmd,
        timestamp,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_fish_line_quoted() {
        let entry = parse_fish_line("cmd \"git status\"").unwrap();
        assert_eq!(entry.command, "git status");
        assert!(entry.timestamp.is_none());
    }

    #[test]
    fn test_parse_fish_line_unquoted() {
        let entry = parse_fish_line("cmd ls -la");
        assert!(entry.is_some());
        assert_eq!(entry.unwrap().command, "ls -la");
    }

    #[test]
    fn test_parse_fish_line_empty_after_strip() {
        let entry = parse_fish_line("cmd \"\"");
        assert!(entry.is_none());
    }

    #[test]
    fn test_parse_fish_line_no_prefix() {
        let entry = parse_fish_line("ls -la");
        assert!(entry.is_none());
    }

    #[test]
    fn test_parse_fish_yaml_line() {
        let entry = parse_fish_yaml_line("cmd \"docker ps\"\nwhen 1700000000").unwrap();
        assert_eq!(entry.command, "docker ps");
        assert_eq!(entry.timestamp, Some(1700000000));
    }

    #[test]
    fn test_parse_fish_yaml_multiline() {
        let yaml = "cmd \"echo hello\"\nwhen 1700000001\ncmd \"echo world\"\nwhen 1700000002";
        let entry = parse_fish_yaml_line(yaml).unwrap();
        assert_eq!(entry.command, "echo world");
        assert_eq!(entry.timestamp, Some(1700000002));
    }

    #[test]
    fn test_parse_fish_yaml_missing_cmd() {
        let entry = parse_fish_yaml_line("when 1700000000");
        assert!(entry.is_none());
    }

    #[test]
    fn test_parse_fish_yaml_empty() {
        let entry = parse_fish_yaml_line("");
        assert!(entry.is_none());
    }

    #[test]
    fn test_parse_fish_yaml_without_quotes() {
        let entry = parse_fish_yaml_line("cmd docker ps\nwhen 1700000000");
        assert!(entry.is_none());
    }

    #[test]
    fn test_parse_fish_yaml_whitespace_handling() {
        let yaml = "  cmd \"echo test\"  \n    when 1700000000  ";
        let entry = parse_fish_yaml_line(yaml).unwrap();
        assert_eq!(entry.command, "echo test");
        assert_eq!(entry.timestamp, Some(1700000000));
    }
}

use super::HistoryEntry;
use std::path::Path;

// need to test this some day
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
        });
    }

    let parts: Vec<&str> = line.splitn(2, ';').collect();
    if parts.len() != 2 {
        return Some(HistoryEntry {
            command: line.to_string(),
            timestamp: None,
        });
    }

    let header = parts[0];
    let command = parts[1].to_string();

    let timestamp = header.split(':').nth(1).and_then(|t| t.parse::<i64>().ok());

    Some(HistoryEntry { command, timestamp })
}

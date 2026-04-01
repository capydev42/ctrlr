use super::HistoryEntry;
use std::path::Path;

// need to test this some day ..
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

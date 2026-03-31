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

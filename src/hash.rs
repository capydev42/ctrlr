use sha1::{Digest, Sha1};

/// SHA1 hex of the raw bytes.
///
/// Not an identity function for commands — use [`hash_command`] for those, or
/// case-sensitive ids (collection names) end up sharing a row with their
/// differently-cased twins.
pub fn sha1_hex(s: &str) -> String {
    let mut hasher = Sha1::new();
    hasher.update(s.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Canonical form of a command, for both identity and history dedup.
pub fn normalize(s: &str) -> String {
    s.trim().to_lowercase()
}

/// The only way to derive a `commands.id`.
///
/// Always hashes the normalized text, so `Git Status` and `git status` resolve
/// to one row. Every writer must go through here: hashing raw text produces a
/// second row with the same `text`, which the schema has no constraint against.
pub fn hash_command(text: &str) -> String {
    sha1_hex(&normalize(text))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_trims_and_lowercases() {
        assert_eq!(normalize("  LS  "), "ls");
        assert_eq!(normalize("Git Status"), "git status");
        assert_eq!(normalize("\t\techo\n"), "echo");
    }

    #[test]
    fn test_normalize_case_insensitive() {
        assert_eq!(normalize("GIT"), normalize("git"));
        assert_eq!(normalize("Git"), normalize("GIT"));
    }

    #[test]
    fn test_normalize_is_idempotent() {
        assert_eq!(
            normalize(&normalize("  Git Status  ")),
            normalize("  Git Status  ")
        );
    }

    #[test]
    fn test_hash_command_deterministic() {
        assert_eq!(hash_command("ls -la"), hash_command("ls -la"));
    }

    #[test]
    fn test_hash_command_normalizes() {
        assert_eq!(hash_command("Git Status"), hash_command("  git status  "));
        assert_eq!(hash_command("ls -la"), hash_command("ls -la "));
        assert_eq!(hash_command("GIT"), hash_command("git"));
    }

    #[test]
    fn test_hash_command_different_inputs() {
        assert_ne!(hash_command("ls -la"), hash_command("ls -al"));
        assert_ne!(hash_command("ls"), hash_command("ls -la"));
    }

    #[test]
    fn test_hash_command_sha1_format() {
        let hash = hash_command("test");
        assert_eq!(hash.len(), 40);
        assert!(hash.chars().all(|c: char| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_sha1_hex_is_raw() {
        // Collection ids depend on this staying case- and whitespace-sensitive.
        assert_ne!(sha1_hex("Work"), sha1_hex("work"));
        assert_ne!(sha1_hex("work "), sha1_hex("work"));
    }

    #[test]
    fn test_hash_command_matches_sha1_of_normalized() {
        assert_eq!(hash_command("Git Status"), sha1_hex("git status"));
    }
}

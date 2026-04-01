const BASH_SCRIPT: &str = r#"# ctrlr integration
_ctrlr_widget() {
    local tmpfile=$(mktemp)
    ctrlr --output-file "$tmpfile"
    if [[ -s "$tmpfile" ]]; then
        READLINE_LINE=$(cat "$tmpfile")
        READLINE_POINT=${#READLINE_LINE}
    fi
    rm -f "$tmpfile"
}
bind -x '"\C-r": _ctrlr_widget'
"#;

pub fn generate() -> String {
    BASH_SCRIPT.to_string()
}

pub fn is_installed(config_content: &str) -> bool {
    config_content.contains("# ctrlr integration")
}

pub fn is_up_to_date(config_content: &str) -> bool {
    let generated = generate();
    config_content.contains(&generated)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate() {
        let script = generate();
        assert!(script.contains("# ctrlr integration"));
        assert!(script.contains("_ctrlr_widget"));
    }

    #[test]
    fn test_is_installed() {
        assert!(is_installed("# ctrlr integration\nfoo"));
        assert!(!is_installed("# other integration\nfoo"));
    }

    #[test]
    fn test_is_up_to_date() {
        let script = generate();
        assert!(is_up_to_date(&script));
        assert!(!is_up_to_date("other stuff"));
    }
}

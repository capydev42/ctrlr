const ZSH_SCRIPT: &str = r#"# ctrlr integration
_ctrlr_widget() {
    local result
    result=$(script -q /dev/null -c "ctrlr" 2>/dev/null)
    if [[ -n "$result" ]]; then
        BUFFER="$result"
        CURSOR=${#BUFFER}
    fi
}
zle -N _ctrlr_widget
bindkey '^R' _ctrlr_widget
"#;

const MARKER: &str = "# ctrlr integration";

pub fn generate() -> String {
    ZSH_SCRIPT.to_string()
}

pub fn is_installed(config_content: &str) -> bool {
    config_content.contains(MARKER)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate() {
        let script = generate();
        assert!(script.contains(MARKER));
        assert!(script.contains("_ctrlr_widget"));
    }

    #[test]
    fn test_is_installed() {
        assert!(is_installed("# ctrlr integration\nfoo"));
        assert!(!is_installed("# other integration\nfoo"));
    }
}

const FISH_SCRIPT: &str = r#"# ctrlr integration
function _ctrlr_widget
    set result (script -q /dev/null -c "ctrlr" 2>/dev/null)
    if test -n "$result"
        commandline --replace $result
    end
end
bind \cr _ctrlr_widget
"#;

const MARKER: &str = "# ctrlr integration";

pub fn generate() -> String {
    FISH_SCRIPT.to_string()
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

/// `{LOG}` is substituted with the run log path at generation time.
///
/// `--on-event` handlers are subscriptions, so ctrlr's coexist with starship's
/// and anyone else's. fish has no epoch variable, so the timestamp costs one
/// `date` call per command.
const FISH_SCRIPT: &str = r#"# ctrlr integration
set -g _ctrlr_log '{LOG}'
set -g _ctrlr_cwd ''
set -g _ctrlr_cmd ''

if not test -d (dirname $_ctrlr_log)
    mkdir -p (dirname $_ctrlr_log) 2>/dev/null
end
if not test -e $_ctrlr_log
    begin
        umask 077
        touch $_ctrlr_log 2>/dev/null
    end
end

# $PWD here is where the command was typed; fish_postexec runs after a `cd`.
function _ctrlr_preexec --on-event fish_preexec
    set -g _ctrlr_cwd $PWD
    set -g _ctrlr_cmd $argv[1]
end

function _ctrlr_postexec --on-event fish_postexec
    set -l ret $status
    test -n "$_ctrlr_cmd"; or return
    set -l cmd (string replace -a -- \\ \\\\ $_ctrlr_cmd \
        | string replace -a -- \t '\t' \
        | string replace -a -- \r '\r' \
        | string split \n | string join '\n' \
        | string sub -l 4000)
    printf 'v1\t%s\t%s\t%s\t%s\t%s\n' (date +%s) $ret $hostname $_ctrlr_cwd $cmd \
        >> $_ctrlr_log 2>/dev/null
    set -g _ctrlr_cmd ''
end

function _ctrlr_widget
    set -l tmpfile (mktemp)
    ctrlr --output-file $tmpfile
    if test -s $tmpfile
        commandline --replace (cat $tmpfile)
    end
    rm -f $tmpfile
end
bind \cr _ctrlr_widget
# ctrlr integration end
"#;

pub fn generate() -> String {
    FISH_SCRIPT.replace("{LOG}", &crate::storage::runs_log_path().to_string_lossy())
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
    fn test_generate_records_runs() {
        let script = generate();
        assert!(script.contains("--on-event fish_preexec"));
        assert!(script.contains("--on-event fish_postexec"));
        assert!(script.contains("printf 'v1\\t%s"));
    }

    #[test]
    fn test_generate_substitutes_log_path() {
        let script = generate();
        assert!(!script.contains("{LOG}"));
        assert!(script.contains("runs.log"));
    }

    #[test]
    fn test_generate_has_end_marker() {
        assert!(generate().contains("# ctrlr integration end"));
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

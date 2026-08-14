/// `{LOG}` is substituted with the run log path at generation time.
///
/// bash has no native preexec hook. If bash-preexec is loaded — starship and
/// atuin both bring it — appending to `preexec_functions` coexists with them
/// and records the directory the command was actually typed in. Without it we
/// fall back to logging from PROMPT_COMMAND, which cannot see a `cd` before it
/// happened and needs one subshell to read the history line back.
const BASH_SCRIPT: &str = r#"# ctrlr integration
_ctrlr_log='{LOG}'
[ -d "${_ctrlr_log%/*}" ] || mkdir -p "${_ctrlr_log%/*}" 2>/dev/null
[ -e "$_ctrlr_log" ] || ( umask 077; : >> "$_ctrlr_log" ) 2>/dev/null
_ctrlr_cwd=
_ctrlr_cmd=
_ctrlr_mode=
_ctrlr_last_hist=

# Assigns rather than prints: capturing output would fork. EPOCHSECONDS needs
# bash 5, printf %()T needs 4.2, and macOS still ships 3.2.
if [ -n "${EPOCHSECONDS+x}" ]; then
    _ctrlr_now() { _ctrlr_ts=$EPOCHSECONDS; }
elif printf -v _ctrlr_ts '%(%s)T' -1 2>/dev/null; then
    _ctrlr_now() { printf -v _ctrlr_ts '%(%s)T' -1; }
else
    _ctrlr_now() { _ctrlr_ts=$(date +%s); }
fi

# Escapes in place rather than through a helper: a command substitution here
# would fork on every prompt.
_ctrlr_write() {
    local ret=$1 cwd=$2 cmd=$3
    cmd=${cmd//\\/\\\\}
    cmd=${cmd//$'\n'/\\n}
    cmd=${cmd//$'\t'/\\t}
    cmd=${cmd//$'\r'/\\r}
    _ctrlr_now
    printf 'v1\t%s\t%s\t%s\t%s\t%s\n' \
        "$_ctrlr_ts" "$ret" "$HOSTNAME" "$cwd" "${cmd:0:4000}" \
        >> "$_ctrlr_log" 2>/dev/null
}

_ctrlr_preexec() {
    _ctrlr_cwd=$PWD
    _ctrlr_cmd=$1
}

_ctrlr_precmd() {
    local ret=$?
    [ -n "$_ctrlr_cmd" ] || return
    _ctrlr_write "$ret" "$_ctrlr_cwd" "$_ctrlr_cmd"
    _ctrlr_cmd=
}

_ctrlr_prompt() {
    local ret=$?

    # Deferred to the first prompt, not source time: bash-preexec may be loaded
    # by a line further down .bashrc than ctrlr's.
    if [ -z "$_ctrlr_mode" ]; then
        # declare -p rather than a set-test on the name: that expands element 0
        # and so reports an existing but empty array as unset.
        if declare -p preexec_functions >/dev/null 2>&1; then
            preexec_functions+=(_ctrlr_preexec)
            precmd_functions+=(_ctrlr_precmd)
            _ctrlr_mode=preexec
        else
            _ctrlr_mode=prompt
        fi
    fi

    if [ "$_ctrlr_mode" = prompt ]; then
        local line
        line=$(HISTTIMEFORMAT= history 1)
        if [[ $line =~ ^[[:space:]]*([0-9]+)[[:space:]]+(.*)$ ]]; then
            local n=${BASH_REMATCH[1]} cmd=${BASH_REMATCH[2]}
            # A bare Enter re-reports the previous entry; the index tells them
            # apart.
            if [ "$n" != "$_ctrlr_last_hist" ]; then
                _ctrlr_last_hist=$n
                _ctrlr_write "$ret" "$PWD" "$cmd"
            fi
        fi
    fi

    history -a
}

# Not exported: an exported PROMPT_COMMAND is inherited by child shells, which
# installs the hook a second time in every subshell.
PROMPT_COMMAND="_ctrlr_prompt${PROMPT_COMMAND:+; $PROMPT_COMMAND}"

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
# ctrlr integration end
"#;

pub fn generate() -> String {
    BASH_SCRIPT.replace("{LOG}", &crate::storage::runs_log_path().to_string_lossy())
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
        assert!(script.contains("_ctrlr_preexec"));
        assert!(script.contains("preexec_functions+=(_ctrlr_preexec)"));
        assert!(script.contains("v1\\t%s"));
    }

    #[test]
    fn test_generate_does_not_export_prompt_command() {
        // Exporting leaks the hook into every child bash and subshell.
        assert!(!generate().contains("export PROMPT_COMMAND"));
    }

    #[test]
    fn test_generate_prepends_to_prompt_command() {
        // Ours has to run first or $? belongs to whatever ran before it.
        let script = generate();
        assert!(
            script
                .contains(r#"PROMPT_COMMAND="_ctrlr_prompt${PROMPT_COMMAND:+; $PROMPT_COMMAND}""#)
        );
    }

    #[test]
    fn test_generate_substitutes_log_path() {
        let script = generate();
        assert!(!script.contains("{LOG}"));
        assert!(script.contains("runs.log"));
    }

    #[test]
    fn test_generate_detects_empty_preexec_array() {
        // bash-preexec may be loaded with nothing registered yet; the +x test
        // reports that as absent.
        let script = generate();
        assert!(script.contains("declare -p preexec_functions"));
        assert!(!script.contains("${preexec_functions+x}"));
    }

    #[test]
    fn test_generate_has_no_debug_trap() {
        // A raw DEBUG trap fights starship's; detection defers to bash-preexec.
        assert!(!generate().contains("trap "));
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

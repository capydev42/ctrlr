/// `{LOG}` is substituted with the run log path at generation time: the shell
/// cannot work out `dirs::data_dir()` for itself without forking, and it
/// differs between Linux and macOS.
const ZSH_SCRIPT: &str = r#"# ctrlr integration
autoload -Uz add-zsh-hook
zmodload -i zsh/datetime

typeset -g _CTRLR_LOG='{LOG}'
typeset -g _CTRLR_CWD=
typeset -g _CTRLR_CMD=

[[ -d ${_CTRLR_LOG:h} ]] || mkdir -p ${_CTRLR_LOG:h} 2>/dev/null
[[ -e $_CTRLR_LOG ]] || ( umask 077; : >> $_CTRLR_LOG ) 2>/dev/null

# $PWD here is where the command was typed; by precmd a `cd` has already moved.
_ctrlr_preexec() {
    _CTRLR_CWD=$PWD
    _CTRLR_CMD=$1
}

_ctrlr_precmd() {
    local ret=$?
    [[ -n $_CTRLR_CMD ]] || return
    local t=$'\t'
    local cmd=${_CTRLR_CMD//\\/\\\\}
    cmd=${cmd//$'\n'/\\n}
    cmd=${cmd//$'\t'/\\t}
    cmd=${cmd//$'\r'/\\r}
    print -r -- "v1$t$EPOCHSECONDS$t$ret$t$HOST$t$_CTRLR_CWD$t${cmd[1,4000]}" >> $_CTRLR_LOG 2>/dev/null
    _CTRLR_CMD=
}

_flush_zsh_history() { fc -W }

add-zsh-hook preexec _ctrlr_preexec
add-zsh-hook precmd _flush_zsh_history
# Prepended rather than appended: $? in a precmd hook is whatever the previous
# hook returned, so this has to run before any other tool's.
precmd_functions=(_ctrlr_precmd ${precmd_functions:#_ctrlr_precmd})

_ctrlr_widget() {
    local tmpfile=$(mktemp)
    ctrlr --output-file "$tmpfile"
    if [[ -s "$tmpfile" ]]; then
        BUFFER=$(cat "$tmpfile")
        CURSOR=$#BUFFER
    fi
    rm -f "$tmpfile"
}
zle -N _ctrlr_widget
bindkey '^R' _ctrlr_widget
# ctrlr integration end
"#;

pub fn generate() -> String {
    ZSH_SCRIPT.replace("{LOG}", &crate::storage::runs_log_path().to_string_lossy())
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
        assert!(script.contains("add-zsh-hook preexec _ctrlr_preexec"));
        assert!(script.contains("precmd_functions=(_ctrlr_precmd"));
        assert!(script.contains("v1$t$EPOCHSECONDS"));
    }

    #[test]
    fn test_generate_substitutes_log_path() {
        let script = generate();
        assert!(!script.contains("{LOG}"));
        assert!(script.contains("runs.log"));
    }

    #[test]
    fn test_generate_uses_add_zsh_hook_not_bare_functions() {
        // A bare `preexec() { }` would clobber starship's; the hook arrays are
        // additive.
        let script = generate();
        assert!(!script.contains("\npreexec()"));
        assert!(!script.contains("\nprecmd()"));
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

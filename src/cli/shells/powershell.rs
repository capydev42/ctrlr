//! PowerShell profile integration.
//!
//! The Ctrl+R widget lands in a later change; this is the run log only.

pub const SCRIPT: &str = r#"# ctrlr integration
# Re-entry guard. `ctrlr` tells PowerShell to reload with `. $PROFILE`, so this
# file *will* be sourced again in a live session. Without the guard each reload
# captures ctrlr's own prompt and history handler as the "previous" one and
# stacks another layer, writing one extra log line per reload.
if (-not $global:_ctrlrInstalled) {
    $global:_ctrlrInstalled = $true
    $global:_ctrlrLog = '{LOG}'
    $global:_ctrlrCmd = $null
    $global:_ctrlrCwd = $null

    $dir = Split-Path -Parent $global:_ctrlrLog
    if ($dir -and -not (Test-Path -LiteralPath $dir)) {
        New-Item -ItemType Directory -Path $dir -Force | Out-Null
    }

    # Everything here is in-process: no spawn per prompt, unlike fish needing
    # `date`.
    #
    # The redirection cmdlets are avoided on purpose: Windows PowerShell 5.1
    # defaults them to UTF-16LE and to the ANSI codepage, and ctrlr reads the
    # log as UTF-8 - one such line and the whole file is discarded.
    #
    # FileShare.ReadWrite lets a second terminal append at the same moment;
    # FileShare.Delete lets ctrlr rename the log out from under an open handle
    # while draining it, which on Windows is otherwise a sharing violation.
    function global:_ctrlrWrite([int]$ret, [string]$cwd, [string]$cmd) {
        try {
            $cmd = $cmd.Replace('\', '\\').Replace("`n", '\n').Replace("`t", '\t').Replace("`r", '\r')
            if ($cmd.Length -gt 4000) { $cmd = $cmd.Substring(0, 4000) }
            $ts = [DateTimeOffset]::UtcNow.ToUnixTimeSeconds()
            # MachineName rather than the Windows-only environment variable,
            # which is empty on every other platform.
            $host_ = [Environment]::MachineName
            $line = "v1`t$ts`t$ret`t$host_`t$cwd`t$cmd`n"

            $fs = [System.IO.File]::Open($global:_ctrlrLog, [System.IO.FileMode]::Append,
                [System.IO.FileAccess]::Write,
                [System.IO.FileShare]::ReadWrite -bor [System.IO.FileShare]::Delete)
            try {
                $sw = [System.IO.StreamWriter]::new($fs, [System.Text.UTF8Encoding]::new($false))
                $sw.Write($line)
                $sw.Flush()
                $sw.Dispose()
            } finally { $fs.Dispose() }
        } catch {
            # A broken log must never spray errors over the user's shell.
        }
    }

    # preexec equivalent. Fires when the line is accepted but before it runs,
    # so $PWD is still where the command was typed - which is the whole point
    # of the log. PSReadLine 1.x has no such handler; there the prompt hook
    # below records the *post*-command directory instead, like bash's
    # PROMPT_COMMAND fallback.
    $global:_ctrlrHasHandler =
        (Get-PSReadLineOption).PSObject.Properties.Name -contains 'AddToHistoryHandler'
    $global:_ctrlrLastId = 0

    if ($global:_ctrlrHasHandler) {
        $global:_ctrlrPrevHistory = (Get-PSReadLineOption).AddToHistoryHandler
        Set-PSReadLineOption -AddToHistoryHandler {
            param($line)

            # PSReadLine ships a handler that answers MemoryOnly for anything
            # that looks like a secret, keeping it out of the history file.
            # Chaining preserves that; recording only on MemoryAndFile keeps
            # ctrlr's own log from becoming the leak it just prevented.
            $verdict = if ($global:_ctrlrPrevHistory) {
                $global:_ctrlrPrevHistory.Invoke($line)
            } else {
                [Microsoft.PowerShell.AddToHistoryOption]::MemoryAndFile
            }

            if ("$verdict" -eq 'MemoryAndFile') {
                $global:_ctrlrCmd = $line
                # .ProviderPath, not .Path: after navigating a provider the
                # latter reads Microsoft.PowerShell.Core\FileSystem::C:\x.
                $global:_ctrlrCwd = $PWD.ProviderPath
            }

            $verdict
        }
    }

    # precmd equivalent, and the inverse of bash: there ctrlr has to run first
    # to see $?, here it has to be the outermost prompt, so it must be defined
    # *after* starship or oh-my-posh. Appending to the end of the profile does
    # that by construction.
    $global:_ctrlrPrevPrompt = $function:prompt
    function global:prompt {
        # First two statements, before anything can clobber them.
        $ok = $?
        $code = $LASTEXITCODE

        # PSReadLine 1.x has no handler, so nothing recorded the line. Read it
        # back instead; the directory is then the one *after* the command, the
        # same loss bash takes without bash-preexec. Ids tell a fresh entry
        # from a bare Enter re-reporting the previous one.
        if (-not $global:_ctrlrHasHandler) {
            $last = Get-History -Count 1 -ErrorAction SilentlyContinue
            if ($last -and $last.Id -ne $global:_ctrlrLastId) {
                $global:_ctrlrLastId = $last.Id
                $global:_ctrlrCmd = $last.CommandLine
                $global:_ctrlrCwd = $PWD.ProviderPath
            }
        }

        if ($global:_ctrlrCmd) {
            # $? wins: $LASTEXITCODE is stale after a cmdlet-only command, so
            # `cd` following a failure would otherwise inherit its code.
            $ret = if ($ok) { 0 } elseif ($code) { $code } else { 1 }
            $cwd = if ($global:_ctrlrCwd) { $global:_ctrlrCwd } else { $PWD.ProviderPath }
            _ctrlrWrite $ret $cwd $global:_ctrlrCmd
            $global:_ctrlrCmd = $null
            $global:_ctrlrCwd = $null
        }

        if ($global:_ctrlrPrevPrompt) { & $global:_ctrlrPrevPrompt } else { "PS $($PWD.Path)> " }
    }
}
# ctrlr integration end
"#;

#[cfg(test)]
mod tests {
    use crate::cli::shells::{Shell, generate_script};

    fn generate() -> String {
        generate_script(Shell::PowerShell)
    }

    #[test]
    fn test_generate_substitutes_log_path() {
        let script = generate();
        assert!(!script.contains("{LOG}"));
        assert!(script.contains("runs.log"));
    }

    #[test]
    fn test_generate_records_runs() {
        let script = generate();
        assert!(script.contains("AddToHistoryHandler"));
        assert!(script.contains("function global:prompt"));
        assert!(script.contains("v1`t$ts`t$ret`t$host_`t$cwd`t$cmd"));
    }

    /// Without it a `. $PROFILE` reload stacks another prompt and handler, and
    /// every command is logged once more per reload.
    #[test]
    fn test_generate_guards_against_re_sourcing() {
        assert!(generate().contains("if (-not $global:_ctrlrInstalled)"));
    }

    /// 5.1 defaults these to UTF-16LE and the ANSI codepage; one such line and
    /// `read_runs` discards the whole file.
    #[test]
    fn test_generate_writes_utf8_without_the_cmdlets() {
        let script = generate();
        assert!(script.contains("UTF8Encoding"));
        assert!(!script.contains("Out-File"));
        assert!(!script.contains("Add-Content"));
    }

    /// Replacing PSReadLine's own handler without chaining would start writing
    /// password-shaped commands to the history file.
    #[test]
    fn test_generate_chains_the_history_handler() {
        let script = generate();
        assert!(script.contains("$global:_ctrlrPrevHistory.Invoke($line)"));
        assert!(script.contains("-eq 'MemoryAndFile'"));
    }

    /// $env:COMPUTERNAME is empty anywhere but Windows.
    #[test]
    fn test_generate_uses_a_portable_host_name() {
        let script = generate();
        assert!(script.contains("[Environment]::MachineName"));
        assert!(!script.contains("COMPUTERNAME"));
    }

    /// Both are needed for ctrlr's rename-then-parse drain and for a second
    /// terminal appending at the same moment.
    #[test]
    fn test_generate_opens_the_log_shared() {
        let script = generate();
        assert!(script.contains("[System.IO.FileShare]::ReadWrite"));
        assert!(script.contains("[System.IO.FileShare]::Delete"));
    }

    #[test]
    fn test_generate_ends_with_the_marker() {
        assert!(generate().trim_end().ends_with("# ctrlr integration end"));
    }
}

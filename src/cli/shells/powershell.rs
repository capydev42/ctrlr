//! PowerShell profile integration: the run log and the Ctrl+R widget.

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

# Outside the guard on purpose: reloading the profile is how a user gets the
# binding back after something else claimed Ctrl+R. Re-registering is a no-op.
# -Key, not -Chord: PSReadLine 1.x knows only the former, and on 2.x it is an
# alias for the latter. Windows 10 still ships 1.x with Windows PowerShell 5.1,
# where -Chord fails to bind and Ctrl+R silently stays PSReadLine's own search.
Set-PSReadLineKeyHandler -Key 'Ctrl+r' -BriefDescription 'ctrlr' -ScriptBlock {
    $tmp = [System.IO.Path]::GetTempFileName()
    try {
        ctrlr --output-file $tmp
        # An empty file means the picker was cancelled, and ctrlr exits
        # non-zero to say so. Replacing with it would wipe a half-typed line.
        $picked = [System.IO.File]::ReadAllText($tmp)
        if ($picked.Trim().Length -gt 0) {
            # RevertLine plus Insert rather than Replace: the pair predates
            # PSReadLine 2.0, and leaves the cursor at the end just the same.
            [Microsoft.PowerShell.PSConsoleReadLine]::RevertLine()
            [Microsoft.PowerShell.PSConsoleReadLine]::Insert($picked.TrimEnd("`r", "`n"))
        }
    } finally {
        Remove-Item -LiteralPath $tmp -Force -ErrorAction SilentlyContinue
        try {
            # The child drew over PSReadLine's idea of the screen. Absent on
            # 1.x, where the redraw is skipped rather than raised at the user.
            [Microsoft.PowerShell.PSConsoleReadLine]::InvokePrompt()
        } catch [System.Management.Automation.RuntimeException] {
        }
    }
}
# ctrlr integration end
"#;

/// Warns when Windows would refuse to run the profile ctrlr just wrote.
///
/// Under `Restricted` — the Windows client default — no profile is loaded at
/// all, so `ctrlr init` reports success and nothing works afterwards: no
/// Ctrl+R, no run log, and a red error at every shell start that does not
/// mention ctrlr. `AllSigned` rejects it too, since the profile is unsigned.
///
/// Only on Windows: PowerShell on unix ignores execution policy entirely.
pub fn execution_policy_hint() -> Option<String> {
    if !cfg!(windows) {
        return None;
    }
    // A spawn, but only from `ctrlr init` and the install popup - never on the
    // launch path.
    let out = std::process::Command::new("powershell")
        .args(["-NoProfile", "-Command", "Get-ExecutionPolicy"])
        .output()
        .ok()?;
    hint_for_policy(&String::from_utf8_lossy(&out.stdout))
}

fn hint_for_policy(policy: &str) -> Option<String> {
    let policy = policy.trim();
    if !policy.eq_ignore_ascii_case("Restricted") && !policy.eq_ignore_ascii_case("AllSigned") {
        return None;
    }
    Some(format!(
        "\n\u{26a0}\u{fe0f} PowerShell will not load the profile: execution policy is {policy}.\n\
         Until that changes, Ctrl+R and directory tracking stay off.\n\n\
         To allow your own profile while still requiring signatures on\n\
         downloaded scripts:\n    \
         Set-ExecutionPolicy -Scope CurrentUser RemoteSigned\n\n\
         That covers your user only and needs no administrator rights."
    ))
}

#[cfg(test)]
mod tests {
    use super::hint_for_policy;
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

    #[test]
    fn test_generate_binds_ctrl_r() {
        let script = generate();
        assert!(script.contains("Set-PSReadLineKeyHandler -Key 'Ctrl+r'"));
        assert!(script.contains("ctrlr --output-file $tmp"));
    }

    /// ctrlr writes an empty file to mean "cancelled"; replacing with it would
    /// wipe whatever the user had already typed.
    #[test]
    fn test_generate_ignores_an_empty_pick() {
        assert!(generate().contains("$picked.Trim().Length -gt 0"));
    }

    /// The binding sits outside the re-entry guard so reloading the profile
    /// restores it after something else claims Ctrl+R.
    #[test]
    fn test_key_handler_is_registered_outside_the_guard() {
        // Everything inside the guard is indented; the binding is not, which
        // is what makes a profile reload restore it.
        let script = generate();
        assert!(script.contains("\nSet-PSReadLineKeyHandler -Key 'Ctrl+r'"));
        assert!(!script.contains("    Set-PSReadLineKeyHandler"));
    }

    #[test]
    fn test_policy_hint_only_for_the_blocking_policies() {
        for blocked in ["Restricted", "AllSigned", "  restricted\r\n"] {
            let hint = hint_for_policy(blocked).expect("should warn");
            assert!(hint.contains("Set-ExecutionPolicy -Scope CurrentUser RemoteSigned"));
        }
        for allowed in ["RemoteSigned", "Unrestricted", "Bypass", "Undefined", ""] {
            assert!(
                hint_for_policy(allowed).is_none(),
                "{allowed} should not warn"
            );
        }
    }

    /// Never RemoteSigned or looser: those would drop the signature check on
    /// downloaded scripts as well.
    #[test]
    fn test_policy_hint_suggests_the_narrow_fix() {
        let hint = hint_for_policy("Restricted").unwrap();
        assert!(!hint.contains("Unrestricted"));
        assert!(!hint.contains("Bypass"));
        assert!(hint.contains("CurrentUser"));
    }
}

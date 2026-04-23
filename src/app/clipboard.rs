use std::io::Write;
use std::process::{Command, Stdio};

#[cfg(target_os = "linux")]
pub fn copy_to_clipboard(text: &str) -> (bool, Option<String>) {
    if try_xclip(text) || try_wl_copy(text) || try_arboard(text) {
        return (true, None);
    }

    (
        false,
        Some("⚠ Clipboard tool not found (install xclip or wl-copy)".into()),
    )
}

#[cfg(target_os = "linux")]
fn try_xclip(text: &str) -> bool {
    let mut child = match Command::new("xclip")
        .args(["-selection", "clipboard", "-i"])
        .stdin(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(_) => return false,
    };
    match child.stdin.as_mut() {
        Some(stdin) => {
            if stdin.write_all(text.as_bytes()).is_err() {
                return false;
            }
        }
        None => return false,
    }
    child.wait().map(|s| s.success()).unwrap_or(false)
}

#[cfg(target_os = "linux")]
fn try_wl_copy(text: &str) -> bool {
    let mut child = match Command::new("wl-copy").stdin(Stdio::piped()).spawn() {
        Ok(c) => c,
        Err(_) => return false,
    };
    match child.stdin.as_mut() {
        Some(stdin) => {
            if stdin.write_all(text.as_bytes()).is_err() {
                return false;
            }
        }
        None => return false,
    }
    child.wait().map(|s| s.success()).unwrap_or(false)
}

#[cfg(target_os = "linux")]
fn try_arboard(text: &str) -> bool {
    match arboard::Clipboard::new() {
        Ok(mut clipboard) => clipboard.set_text(text).is_ok(),
        Err(_) => false,
    }
}

#[cfg(not(target_os = "linux"))]
pub fn copy_to_clipboard(text: &str) -> (bool, Option<String>) {
    match arboard::Clipboard::new() {
        Ok(mut clipboard) => {
            if clipboard.set_text(text).is_ok() {
                return (true, None);
            }
        }
        Err(_) => {}
    }
    (false, Some("⚠ Failed to access clipboard".into()))
}

//! PowerShell profile integration.
//!
//! Placeholder: the hooks and the Ctrl+R widget land in a later change. Both
//! markers are already here so `is_installed` and `strip_integration` work,
//! and `#` is PowerShell's comment character, so they need no special form.

pub const SCRIPT: &str = r#"# ctrlr integration
# Run log and Ctrl+R binding are not wired up yet.
# ctrlr integration end
"#;

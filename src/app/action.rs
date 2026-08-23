#[derive(Debug, Clone, PartialEq, Default)]
pub enum Action {
    #[default]
    None,
    Exit,
    Execute(String),
    /// Suspend the TUI and hand this text to `$VISUAL` / `$EDITOR`. Carried up
    /// to `main.rs::app()` for the same reason as `Execute`: the input layer
    /// has no `Terminal` to suspend.
    OpenEditor(String),
}

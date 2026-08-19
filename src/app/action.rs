#[derive(Debug, Clone, PartialEq, Default)]
pub enum Action {
    #[default]
    None,
    Exit,
    Execute(String),
}

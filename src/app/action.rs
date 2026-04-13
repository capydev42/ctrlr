#[derive(Debug, Clone, PartialEq, Default)]
pub enum Action {
    #[default]
    None,
    #[allow(dead_code)]
    Exit,
    Execute(String),
}

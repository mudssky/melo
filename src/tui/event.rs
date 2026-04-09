/// TUI 中可触发的动作。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    TogglePlayback,
    Next,
    Prev,
    Quit,
    OpenSearch,
    OpenHelp,
}

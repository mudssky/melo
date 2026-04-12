/// TUI 中可触发的动作。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    TogglePlayback,
    Next,
    Prev,
    Quit,
    OpenSearch,
    OpenHelp,
    LoadSelectedPlaylistPreview,
    PlaySelectedPlaylistFromStart,
    PlaySelectedPreviewSong,
    CycleRepeatMode,
    ToggleShuffle,
}

/// TUI 的稳定动作标识。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ActionId {
    FocusNext,
    FocusPrev,
    MoveUp,
    MoveDown,
    JumpTop,
    JumpBottom,
    PageUp,
    PageDown,
    Activate,
    PlaySelection,
    PlayPreviewSelection,
    LoadPreview,
    TogglePlayback,
    Next,
    Prev,
    CycleRepeatMode,
    ToggleShuffle,
    OpenHelp,
    Quit,
}

/// 键盘与鼠标统一归一化后的交互意图。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Intent {
    Action(ActionId),
    SelectPlaylist { index: usize, focus: bool },
    SelectPreview { index: usize, focus: bool },
    ScrollPreview(isize),
    ScrollPlaylist(isize),
}

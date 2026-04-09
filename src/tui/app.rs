use crate::core::model::player::PlayerSnapshot;
use crate::tui::event::Action;

/// TUI 当前视图。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActiveView {
    Songs,
}

/// TUI 当前焦点区域。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FocusArea {
    Content,
}

/// TUI 应用状态。
pub struct App {
    /// 当前播放器快照。
    pub player: PlayerSnapshot,
    /// 当前激活视图。
    pub active_view: ActiveView,
    /// 当前焦点区域。
    pub focus: FocusArea,
}

impl App {
    /// 创建测试用 TUI 状态。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回
    /// - `Self`：测试用 app 状态
    pub fn new_for_test() -> Self {
        Self {
            player: PlayerSnapshot::default(),
            active_view: ActiveView::Songs,
            focus: FocusArea::Content,
        }
    }

    /// 处理键盘事件并映射到动作。
    ///
    /// # 参数
    /// - `key`：键盘事件
    ///
    /// # 返回
    /// - `Option<Action>`：命中的动作
    pub fn handle_key(&mut self, key: crossterm::event::KeyEvent) -> Option<Action> {
        match key.code {
            crossterm::event::KeyCode::Char(' ') => Some(Action::TogglePlayback),
            crossterm::event::KeyCode::Char('>') => Some(Action::Next),
            crossterm::event::KeyCode::Char('<') => Some(Action::Prev),
            crossterm::event::KeyCode::Char('/') => Some(Action::OpenSearch),
            crossterm::event::KeyCode::Char('?') => Some(Action::OpenHelp),
            crossterm::event::KeyCode::Char('q') => Some(Action::Quit),
            _ => None,
        }
    }

    /// 用远端快照刷新本地状态。
    ///
    /// # 参数
    /// - `snapshot`：播放器快照
    ///
    /// # 返回
    /// - 无
    pub fn apply_snapshot(&mut self, snapshot: PlayerSnapshot) {
        self.player = snapshot;
    }
}

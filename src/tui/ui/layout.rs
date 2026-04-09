use ratatui::layout::{Constraint, Direction, Layout, Rect};

/// TUI 主布局区域。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AppLayout {
    /// 侧边栏区域。
    pub sidebar: Rect,
    /// 主内容区域。
    pub content: Rect,
    /// 播放栏区域。
    pub playbar: Rect,
}

/// 拆分 TUI 整体布局。
///
/// # 参数
/// - `area`：可用矩形区域
///
/// # 返回值
/// - `AppLayout`：拆分后的布局结构
pub fn split(area: Rect) -> AppLayout {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(3)])
        .split(area);
    let horizontal = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(24), Constraint::Min(0)])
        .split(vertical[0]);

    AppLayout {
        sidebar: horizontal[0],
        content: horizontal[1],
        playbar: vertical[1],
    }
}

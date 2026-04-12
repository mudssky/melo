use ratatui::layout::{Constraint, Direction, Layout, Rect};

/// TUI 主布局区域。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AppLayout {
    /// 顶部任务栏区域。
    pub task_bar: Option<Rect>,
    /// 侧边栏区域。
    pub sidebar: Rect,
    /// 右侧顶部状态区域。
    pub content_header: Rect,
    /// 右侧主体区域。
    pub content_body: Rect,
    /// 兼容旧渲染逻辑的主内容区域。
    pub content: Rect,
    /// 播放栏区域。
    pub playbar: Rect,
}

/// 拆分 TUI 整体布局。
///
/// # 参数
/// - `area`：可用矩形区域
/// - `show_task_bar`：是否预留顶部任务栏
///
/// # 返回值
/// - `AppLayout`：拆分后的布局结构
pub fn split(area: Rect, show_task_bar: bool) -> AppLayout {
    let constraints = if show_task_bar {
        vec![
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(3),
        ]
    } else {
        vec![Constraint::Min(0), Constraint::Length(3)]
    };
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(area);
    let body = if show_task_bar {
        vertical[1]
    } else {
        vertical[0]
    };
    let horizontal = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(32), Constraint::Min(0)])
        .split(body);
    let right = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(6), Constraint::Min(0)])
        .split(horizontal[1]);

    AppLayout {
        task_bar: show_task_bar.then_some(vertical[0]),
        sidebar: horizontal[0],
        content_header: right[0],
        content_body: right[1],
        content: right[1],
        playbar: *vertical.last().unwrap(),
    }
}

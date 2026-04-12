use ratatui::layout::{Constraint, Direction, Layout, Rect};

/// 返回帮助弹窗的默认文案。
///
/// # 参数
/// - 无
///
/// # 返回值
/// - `Vec<&'static str>`：帮助提示列表
pub fn help_lines() -> Vec<&'static str> {
    vec![
        "Tab 切换焦点",
        "Enter 播放当前选择",
        "r 切换循环模式",
        "s 切换随机播放",
        "Space 播放/暂停",
        "? 打开帮助",
        "q 退出",
    ]
}

/// 计算帮助弹层的居中显示区域。
///
/// # 参数
/// - `area`：当前终端可用区域
///
/// # 返回值
/// - `Rect`：弹层矩形区域
pub fn centered_area(area: Rect) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(20),
            Constraint::Percentage(60),
            Constraint::Percentage(20),
        ])
        .split(area);
    let horizontal = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(15),
            Constraint::Percentage(70),
            Constraint::Percentage(15),
        ])
        .split(vertical[1]);
    horizontal[1]
}

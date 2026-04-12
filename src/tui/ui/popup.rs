use ratatui::layout::{Constraint, Direction, Layout, Rect};

/// 返回帮助弹窗的默认文案。
///
/// # 参数
/// - `keymap`：当前生效的 keymap 解析器
///
/// # 返回值
/// - `Vec<String>`：帮助提示列表
pub fn help_lines_for(keymap: &crate::tui::keymap::Keymap) -> Vec<String> {
    vec![
        format!(
            "{} 切换焦点",
            keymap.describe(crate::tui::event::ActionId::FocusNext)
        ),
        format!(
            "{} 播放当前选择",
            keymap.describe(crate::tui::event::ActionId::Activate)
        ),
        format!(
            "{} 切换循环模式",
            keymap.describe(crate::tui::event::ActionId::CycleRepeatMode)
        ),
        format!(
            "{} 切换随机播放",
            keymap.describe(crate::tui::event::ActionId::ToggleShuffle)
        ),
        format!(
            "{} 播放/暂停",
            keymap.describe(crate::tui::event::ActionId::TogglePlayback)
        ),
        format!(
            "{} 打开帮助",
            keymap.describe(crate::tui::event::ActionId::OpenHelp)
        ),
        format!(
            "{} 退出",
            keymap.describe(crate::tui::event::ActionId::Quit)
        ),
    ]
}

/// 返回默认 keymap 下的帮助文案。
///
/// # 参数
/// - 无
///
/// # 返回值
/// - `Vec<String>`：帮助提示列表
pub fn help_lines() -> Vec<String> {
    let keymap = crate::tui::keymap::Keymap::default();
    help_lines_for(&keymap)
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

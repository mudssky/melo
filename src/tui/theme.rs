use ratatui::style::{Color, Modifier, Style};

/// TUI 主题定义。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Theme {
    /// 标题前缀。
    pub title_prefix: &'static str,
    /// 次级前缀。
    pub muted_prefix: &'static str,
    /// 普通面板边框样式。
    pub pane_border: Style,
    /// 聚焦面板边框样式。
    pub focused_border: Style,
    /// 当前选中行样式。
    pub selected_row: Style,
    /// 当前播放来源行样式。
    pub current_source_row: Style,
    /// 当前选中且也是播放来源时的组合样式。
    pub selected_current_source_row: Style,
}

impl Default for Theme {
    /// 返回默认主题配置。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - `Self`：默认主题
    fn default() -> Self {
        Self {
            title_prefix: "Melo",
            muted_prefix: "Remote",
            pane_border: Style::default().fg(Color::DarkGray),
            focused_border: Style::default().fg(Color::Cyan),
            selected_row: Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
            current_source_row: Style::default().fg(Color::Yellow),
            selected_current_source_row: Style::default()
                .fg(Color::Black)
                .bg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        }
    }
}

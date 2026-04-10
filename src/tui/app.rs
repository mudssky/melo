use ratatui::layout::Rect;

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
    /// # 返回值
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
    /// # 返回值
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
    /// # 返回值
    /// - 无
    pub fn apply_snapshot(&mut self, snapshot: PlayerSnapshot) {
        self.player = snapshot;
    }

    /// 根据当前快照生成底部状态栏文案。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - `String`：底部状态栏文本
    pub fn footer_status(&self) -> String {
        if let Some(error) = &self.player.last_error {
            return format!("ERR {}: {}", error.code, error.message);
        }

        let volume = if self.player.muted {
            "muted".to_string()
        } else {
            self.player.volume_percent.to_string()
        };

        format!(
            "{} | queue={} | prev={} | next={} | vol={} | repeat={} | shuffle={}",
            self.player.playback_state,
            self.player.queue_len,
            self.player.has_prev,
            self.player.has_next,
            volume,
            self.player.repeat_mode,
            self.player.shuffle_enabled
        )
    }

    /// 计算当前屏幕布局。
    ///
    /// # 参数
    /// - `area`：可用矩形区域
    ///
    /// # 返回值
    /// - `AppLayout`：拆分后的 TUI 布局
    pub fn layout(&self, area: Rect) -> crate::tui::ui::layout::AppLayout {
        crate::tui::ui::layout::split(area)
    }

    /// 按显示宽度格式化歌曲标题。
    ///
    /// # 参数
    /// - `title`：原始标题
    /// - `width`：可用宽度
    ///
    /// # 返回值
    /// - `String`：适配宽度后的显示文本
    pub fn format_song_title(&self, title: &str, width: usize) -> String {
        crate::tui::ui::content::render_song_title(title, width)
    }
}

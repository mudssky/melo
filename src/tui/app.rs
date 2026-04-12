use ratatui::layout::Rect;

use crate::core::model::player::PlayerSnapshot;
use crate::tui::event::Action;

/// TUI 当前视图。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActiveView {
    Playlist,
    Songs,
}

/// TUI 当前焦点区域。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FocusArea {
    PlaylistList,
    PlaylistPreview,
}

/// TUI 应用状态。
pub struct App {
    /// 当前播放器快照。
    pub player: PlayerSnapshot,
    /// 当前活动运行时任务。
    pub active_task: Option<crate::core::model::runtime_task::RuntimeTaskSnapshot>,
    /// 当前激活视图。
    pub active_view: ActiveView,
    /// 当前焦点区域。
    pub focus: FocusArea,
    /// 启动来源标签。
    pub source_label: Option<String>,
    /// 启动阶段要展示的一次性提示。
    pub startup_notice: Option<String>,
    /// 是否显示底部快捷键提示。
    pub footer_hints_enabled: bool,
    /// 当前是否打开帮助弹层。
    pub show_help: bool,
    /// 当前歌单浏览快照。
    pub playlist_browser: crate::core::model::tui::PlaylistBrowserSnapshot,
    /// 当前选中的歌单名。
    pub selected_playlist_name: Option<String>,
    /// 当前预览对应的歌单名。
    pub preview_name: Option<String>,
    /// 当前歌单预览标题列表。
    pub preview_titles: Vec<String>,
    /// 当前选中的预览索引。
    pub selected_preview_index: usize,
    /// 当前是否正在加载预览。
    pub preview_loading: bool,
    /// 当前预览错误。
    pub preview_error: Option<String>,
    /// 当前队列标题列表。
    pub queue_titles: Vec<String>,
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
            active_task: None,
            active_view: ActiveView::Playlist,
            focus: FocusArea::PlaylistList,
            source_label: None,
            startup_notice: None,
            footer_hints_enabled: true,
            show_help: false,
            playlist_browser: crate::core::model::tui::PlaylistBrowserSnapshot::default(),
            selected_playlist_name: None,
            preview_name: None,
            preview_titles: Vec::new(),
            selected_preview_index: 0,
            preview_loading: false,
            preview_error: None,
            queue_titles: Vec::new(),
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
            crossterm::event::KeyCode::Up => self.handle_up_key(),
            crossterm::event::KeyCode::Down => self.handle_down_key(),
            crossterm::event::KeyCode::Tab => {
                self.focus = match self.focus {
                    FocusArea::PlaylistList => FocusArea::PlaylistPreview,
                    FocusArea::PlaylistPreview => FocusArea::PlaylistList,
                };
                None
            }
            crossterm::event::KeyCode::Enter => match self.focus {
                FocusArea::PlaylistList => Some(Action::PlaySelectedPlaylistFromStart),
                FocusArea::PlaylistPreview => Some(Action::PlaySelectedPreviewSong),
            },
            crossterm::event::KeyCode::Char(' ') => Some(Action::TogglePlayback),
            crossterm::event::KeyCode::Char('>') => Some(Action::Next),
            crossterm::event::KeyCode::Char('<') => Some(Action::Prev),
            crossterm::event::KeyCode::Char('/') => Some(Action::OpenSearch),
            crossterm::event::KeyCode::Char('r') => Some(Action::CycleRepeatMode),
            crossterm::event::KeyCode::Char('s') => Some(Action::ToggleShuffle),
            crossterm::event::KeyCode::Char('?') => {
                self.show_help = !self.show_help;
                Some(Action::OpenHelp)
            }
            crossterm::event::KeyCode::Esc => {
                if self.show_help {
                    self.show_help = false;
                    None
                } else {
                    None
                }
            }
            crossterm::event::KeyCode::Char('q') => {
                if self.show_help {
                    self.show_help = false;
                    None
                } else {
                    Some(Action::Quit)
                }
            }
            _ => None,
        }
    }

    /// 处理向上移动键。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - `Option<Action>`：是否需要触发新动作
    fn handle_up_key(&mut self) -> Option<Action> {
        match self.focus {
            FocusArea::PlaylistList => self.move_selected_playlist(-1),
            FocusArea::PlaylistPreview => {
                if self.selected_preview_index > 0 {
                    self.selected_preview_index -= 1;
                }
                None
            }
        }
    }

    /// 处理向下移动键。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - `Option<Action>`：是否需要触发新动作
    fn handle_down_key(&mut self) -> Option<Action> {
        match self.focus {
            FocusArea::PlaylistList => self.move_selected_playlist(1),
            FocusArea::PlaylistPreview => {
                if self.selected_preview_index + 1 < self.preview_titles.len() {
                    self.selected_preview_index += 1;
                }
                None
            }
        }
    }

    /// 按偏移量移动歌单选择。
    ///
    /// # 参数
    /// - `delta`：移动偏移，负数表示向上，正数表示向下
    ///
    /// # 返回值
    /// - `Option<Action>`：选择发生变化时返回加载预览动作
    fn move_selected_playlist(&mut self, delta: isize) -> Option<Action> {
        if self.playlist_browser.visible_playlists.is_empty() {
            return None;
        }

        let current_index = self
            .selected_playlist_name
            .as_ref()
            .and_then(|selected| {
                self.playlist_browser
                    .visible_playlists
                    .iter()
                    .position(|playlist| &playlist.name == selected)
            })
            .unwrap_or(0);
        let next_index = if delta.is_negative() {
            current_index.saturating_sub(delta.unsigned_abs())
        } else {
            (current_index + delta as usize).min(
                self.playlist_browser
                    .visible_playlists
                    .len()
                    .saturating_sub(1),
            )
        };

        let next_name = self.playlist_browser.visible_playlists[next_index]
            .name
            .clone();
        if self.selected_playlist_name.as_deref() == Some(next_name.as_str()) {
            return None;
        }

        self.selected_playlist_name = Some(next_name);
        self.preview_error = None;
        self.preview_loading = false;
        self.preview_titles.clear();
        self.selected_preview_index = 0;
        Some(Action::LoadSelectedPlaylistPreview)
    }

    /// 用远端快照刷新本地状态。
    ///
    /// # 参数
    /// - `snapshot`：播放器快照
    ///
    /// # 返回值
    /// - 无
    pub fn apply_snapshot(&mut self, snapshot: PlayerSnapshot) {
        self.queue_titles = snapshot.queue_preview.clone();
        self.player = snapshot;
    }

    /// 用 TUI 聚合快照刷新本地状态。
    ///
    /// # 参数
    /// - `snapshot`：TUI 聚合快照
    ///
    /// # 返回值
    /// - 无
    pub fn apply_tui_snapshot(&mut self, snapshot: crate::core::model::tui::TuiSnapshot) {
        self.apply_snapshot(snapshot.player);
        self.active_task = snapshot.active_task;
        self.playlist_browser = snapshot.playlist_browser;
        self.active_view = ActiveView::Playlist;

        let selected_still_exists = self
            .selected_playlist_name
            .as_ref()
            .is_some_and(|selected| {
                self.playlist_browser
                    .visible_playlists
                    .iter()
                    .any(|playlist| &playlist.name == selected)
            });

        if !selected_still_exists {
            self.selected_playlist_name = self
                .playlist_browser
                .default_selected_playlist
                .clone()
                .or_else(|| {
                    self.playlist_browser
                        .visible_playlists
                        .first()
                        .map(|playlist| playlist.name.clone())
                });
        }
    }

    /// 返回当前选中的歌单名。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - `Option<&str>`：当前选中的歌单名
    pub fn selected_playlist_name(&self) -> Option<&str> {
        self.selected_playlist_name.as_deref()
    }

    /// 返回当前选中的预览索引。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - `usize`：当前选中的预览索引
    pub fn selected_preview_index(&self) -> usize {
        self.selected_preview_index
    }

    /// 写入当前歌单预览。
    ///
    /// # 参数
    /// - `preview`：歌单预览响应
    ///
    /// # 返回值
    /// - 无
    pub fn set_playlist_preview(
        &mut self,
        preview: &crate::api::playlist::PlaylistPreviewResponse,
    ) {
        self.preview_name = Some(preview.name.clone());
        self.preview_titles = preview
            .songs
            .iter()
            .map(|song| song.title.clone())
            .collect();
        self.preview_loading = false;
        self.preview_error = None;
        if self.selected_preview_index >= self.preview_titles.len() {
            self.selected_preview_index = self.preview_titles.len().saturating_sub(1);
        }
    }

    /// 标记歌单预览正在加载。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - 无
    pub fn set_playlist_preview_loading(&mut self) {
        self.preview_loading = true;
        self.preview_error = None;
    }

    /// 写入歌单预览错误。
    ///
    /// # 参数
    /// - `message`：错误信息
    ///
    /// # 返回值
    /// - 无
    pub fn set_playlist_preview_error(&mut self, message: impl Into<String>) {
        self.preview_loading = false;
        self.preview_error = Some(message.into());
        self.preview_titles.clear();
        self.selected_preview_index = 0;
    }

    /// 设置当前启动来源标签。
    ///
    /// # 参数
    /// - `source_label`：来源标签
    ///
    /// # 返回值
    /// - 无
    pub fn set_source_label(&mut self, source_label: impl Into<String>) {
        self.source_label = Some(source_label.into());
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

        let mut status = format!(
            "{} | backend={} | queue={} | prev={} | next={} | vol={} | repeat={} | shuffle={}",
            self.player.playback_state,
            self.player.backend_name,
            self.player.queue_len,
            self.player.has_prev,
            self.player.has_next,
            volume,
            self.player.repeat_mode,
            self.player.shuffle_enabled
        );

        if let Some(source_label) = &self.source_label {
            status.push_str(" | source=");
            status.push_str(source_label);
        }

        if let Some(startup_notice) = &self.startup_notice {
            status.push_str(" | notice=");
            status.push_str(startup_notice);
        }

        if self.footer_hints_enabled {
            status.push_str(" | hints=Space Play/Pause ? Help q Quit");
        }

        status
    }

    /// 生成队列面板要显示的文本行。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - `Vec<String>`：队列面板的可显示文本行
    pub fn render_queue_lines(&self) -> Vec<String> {
        if self.queue_titles.is_empty() {
            return vec!["No tracks loaded".to_string()];
        }

        self.queue_titles
            .iter()
            .enumerate()
            .map(|(index, title)| {
                if self.player.queue_index == Some(index) {
                    format!("> {title}")
                } else {
                    format!("  {title}")
                }
            })
            .collect()
    }

    /// 计算当前屏幕布局。
    ///
    /// # 参数
    /// - `area`：可用矩形区域
    ///
    /// # 返回值
    /// - `AppLayout`：拆分后的 TUI 布局
    pub fn layout(&self, area: Rect) -> crate::tui::ui::layout::AppLayout {
        crate::tui::ui::layout::split(area, self.active_task.is_some())
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

    /// 基于当前活动任务生成顶部任务栏文案。
    ///
    /// # 参数
    /// - `renderer`：运行时模板渲染器
    /// - `settings`：全局配置
    /// - `width`：可用显示宽度
    ///
    /// # 返回值
    /// - `Option<String>`：任务栏文案；无活动任务时返回 `None`
    pub fn task_bar_text(
        &self,
        renderer: &crate::core::runtime_templates::RuntimeTemplateRenderer,
        settings: &crate::core::config::settings::Settings,
        width: usize,
    ) -> Option<String> {
        let task = self.active_task.as_ref()?;
        let key = match task.phase {
            crate::core::model::runtime_task::RuntimeTaskPhase::Completed => {
                crate::core::runtime_templates::RuntimeTemplateKey::TuiScanDone
            }
            crate::core::model::runtime_task::RuntimeTaskPhase::Failed => {
                crate::core::runtime_templates::RuntimeTemplateKey::TuiScanFailed
            }
            _ => crate::core::runtime_templates::RuntimeTemplateKey::TuiScanActive,
        };

        let rendered = renderer.render(
            settings,
            key,
            serde_json::json!({
                "source_label": task.source_label.as_str(),
                "discovered_count": task.discovered_count,
                "indexed_count": task.indexed_count,
                "queued_count": task.queued_count,
                "current_item_name": task.current_item_name.clone(),
                "error_message": task.last_error.clone(),
            }),
        );

        Some(crate::tui::ui::content::render_song_title(&rendered, width))
    }
}

#[cfg(test)]
mod tests;

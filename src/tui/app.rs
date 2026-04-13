use std::collections::BTreeMap;
use std::time::Duration;

use ratatui::layout::Rect;
use ratatui::widgets::ListState;

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

/// 歌单预览区域的歌曲行。
pub struct PreviewSongRow {
    /// 预览歌曲 ID。
    pub song_id: i64,
    /// 预览歌曲标题。
    pub title: String,
}

/// TUI 应用状态。
pub struct App {
    /// 当前播放器快照。
    pub player: PlayerSnapshot,
    /// 当前轻量播放运行时快照。
    pub runtime: crate::core::model::playback_runtime::PlaybackRuntimeSnapshot,
    /// 当前活动运行时任务。
    pub active_task: Option<crate::core::model::runtime_task::RuntimeTaskSnapshot>,
    /// 当前激活视图。
    pub active_view: ActiveView,
    /// 当前焦点区域。
    pub focus: FocusArea,
    /// 调用方 shell 的当前目录。
    pub launch_cwd: Option<String>,
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
    /// 左侧来源列表视口状态。
    pub source_viewport: crate::tui::viewports::ViewportState,
    /// 中部曲目列表视口状态。
    pub track_viewport: crate::tui::viewports::ViewportState,
    /// 歌词列表视口状态。
    pub lyric_viewport: crate::tui::viewports::ViewportState,
    /// 歌词自动跟随状态。
    pub lyric_follow_state: crate::tui::lyrics::LyricFollowState,
    /// 左侧歌单列表的状态。
    pub playlist_state: ListState,
    /// 右侧预览列表的状态。
    pub preview_state: ListState,
    /// 当前选中的歌单名。
    pub selected_playlist_name: Option<String>,
    /// 当前预览对应的歌单名。
    pub preview_name: Option<String>,
    /// 当前歌单预览完整行模型。
    pub preview_songs: Vec<PreviewSongRow>,
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
    /// 当前播放曲目的歌曲 ID。
    pub current_track_song_id: Option<i64>,
    /// 当前播放曲目的歌词文本。
    pub current_track_lyrics: Option<String>,
    /// 当前播放曲目的封面摘要。
    pub current_track_cover_summary: Option<String>,
    /// 曲目低频内容缓存。
    pub track_content_cache: BTreeMap<i64, crate::core::model::track_content::TrackContentSnapshot>,
    /// 当前等待远端确认的运行时动作。
    pending_runtime_action: Option<crate::tui::event::ActionId>,
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
            runtime: crate::core::model::playback_runtime::PlaybackRuntimeSnapshot {
                generation: 0,
                playback_state: "idle".to_string(),
                current_source_ref: None,
                current_song_id: None,
                current_index: None,
                position_seconds: None,
                duration_seconds: None,
                playback_mode: crate::core::model::playback_mode::PlaybackMode::Ordered,
                volume_percent: 100,
                muted: false,
                last_error_code: None,
            },
            active_task: None,
            active_view: ActiveView::Playlist,
            focus: FocusArea::PlaylistList,
            launch_cwd: None,
            source_label: None,
            startup_notice: None,
            footer_hints_enabled: true,
            show_help: false,
            playlist_browser: crate::core::model::tui::PlaylistBrowserSnapshot::default(),
            source_viewport: crate::tui::viewports::ViewportState::new(1),
            track_viewport: crate::tui::viewports::ViewportState::new(1),
            lyric_viewport: crate::tui::viewports::ViewportState::new(1),
            lyric_follow_state: crate::tui::lyrics::LyricFollowState::new(Duration::from_secs(3)),
            playlist_state: ListState::default(),
            preview_state: ListState::default(),
            selected_playlist_name: None,
            preview_name: None,
            preview_songs: Vec::new(),
            preview_titles: Vec::new(),
            selected_preview_index: 0,
            preview_loading: false,
            preview_error: None,
            queue_titles: Vec::new(),
            current_track_song_id: None,
            current_track_lyrics: None,
            current_track_cover_summary: None,
            track_content_cache: BTreeMap::new(),
            pending_runtime_action: None,
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

    /// 根据稳定动作 ID 处理本地状态变更。
    ///
    /// # 参数
    /// - `action`：要处理的动作 ID
    ///
    /// # 返回值
    /// - `Option<crate::tui::event::Intent>`：如需进一步执行副作用则返回意图
    pub fn handle_action(
        &mut self,
        action: crate::tui::event::ActionId,
    ) -> Option<crate::tui::event::Intent> {
        match action {
            crate::tui::event::ActionId::FocusNext => {
                self.focus = FocusArea::PlaylistPreview;
                None
            }
            crate::tui::event::ActionId::FocusPrev => {
                if self.show_help {
                    self.show_help = false;
                } else {
                    self.focus = FocusArea::PlaylistList;
                }
                None
            }
            crate::tui::event::ActionId::Activate => match self.focus {
                FocusArea::PlaylistList => Some(crate::tui::event::Intent::Action(
                    crate::tui::event::ActionId::PlaySelection,
                )),
                FocusArea::PlaylistPreview => Some(crate::tui::event::Intent::Action(
                    crate::tui::event::ActionId::PlayPreviewSelection,
                )),
            },
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
        match self.select_playlist_index(next_index) {
            Some(crate::tui::event::Intent::Action(crate::tui::event::ActionId::LoadPreview)) => {
                Some(Action::LoadSelectedPlaylistPreview)
            }
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
        let current_song = snapshot.current_song.clone();
        self.current_track_song_id = current_song.as_ref().map(|song| song.song_id);
        self.queue_titles = snapshot.queue_preview.clone();
        self.runtime.generation = snapshot.version;
        self.runtime.playback_state = snapshot.playback_state.clone();
        self.runtime.current_song_id = current_song.as_ref().map(|song| song.song_id);
        self.runtime.current_index = snapshot.queue_index;
        self.runtime.position_seconds = snapshot.position_seconds;
        self.runtime.duration_seconds = current_song.and_then(|song| song.duration_seconds);
        self.runtime.playback_mode = if snapshot.shuffle_enabled {
            crate::core::model::playback_mode::PlaybackMode::Shuffle
        } else if snapshot.repeat_mode == "one" {
            crate::core::model::playback_mode::PlaybackMode::RepeatOne
        } else {
            crate::core::model::playback_mode::PlaybackMode::Ordered
        };
        self.runtime.volume_percent = snapshot.volume_percent;
        self.runtime.muted = snapshot.muted;
        self.runtime.last_error_code = snapshot.last_error.as_ref().map(|error| error.code.clone());
        self.player = snapshot;
    }

    /// 缓存一份曲目低频内容快照。
    ///
    /// # 参数
    /// - `content`：待缓存的曲目内容
    ///
    /// # 返回值
    /// - 无
    pub fn cache_track_content(
        &mut self,
        content: crate::core::model::track_content::TrackContentSnapshot,
    ) {
        self.track_content_cache.insert(content.song_id, content);
    }

    /// 应用一份新的轻量播放运行时快照。
    ///
    /// # 参数
    /// - `runtime`：新的轻量播放运行时快照
    ///
    /// # 返回值
    /// - 无
    pub fn apply_runtime_snapshot(
        &mut self,
        runtime: crate::core::model::playback_runtime::PlaybackRuntimeSnapshot,
    ) {
        self.current_track_song_id = runtime.current_song_id;
        self.runtime = runtime;
    }

    /// 返回当前应高亮的歌词行索引。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - `Option<usize>`：命中时返回歌词行索引
    pub fn current_lyric_index(&self) -> Option<usize> {
        let song_id = self.current_track_song_id?;
        let content = self.track_content_cache.get(&song_id)?;
        let position = self.runtime.position_seconds?;
        content.current_lyric_index(position)
    }

    /// 标记当前存在一个等待远端确认的运行时动作。
    ///
    /// # 参数
    /// - `action`：等待确认的动作
    ///
    /// # 返回值
    /// - 无
    pub fn mark_pending_runtime_action(&mut self, action: crate::tui::event::ActionId) {
        self.pending_runtime_action = Some(action);
    }

    /// 清除当前等待确认的运行时动作。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - 无
    pub fn clear_pending_runtime_action(&mut self) {
        self.pending_runtime_action = None;
    }

    /// 返回当前等待确认的运行时动作。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - `Option<crate::tui::event::ActionId>`：当前等待确认的动作
    pub fn pending_runtime_action(&self) -> Option<crate::tui::event::ActionId> {
        self.pending_runtime_action
    }

    /// 为测试填充一组带当前高亮行的歌词面板数据。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - 无
    pub fn load_fake_lyrics_panel_for_test(&mut self) {
        self.current_track_song_id = Some(7);
        self.lyric_viewport.visible_height = 4;
        self.cache_track_content(crate::core::model::track_content::TrackContentSnapshot {
            song_id: 7,
            title: "Blue Bird".to_string(),
            duration_seconds: Some(212.0),
            artwork: None,
            lyrics: vec![
                crate::core::model::track_content::LyricLine {
                    timestamp_seconds: 0.0,
                    text: "line 0".to_string(),
                },
                crate::core::model::track_content::LyricLine {
                    timestamp_seconds: 1.0,
                    text: "line 1".to_string(),
                },
                crate::core::model::track_content::LyricLine {
                    timestamp_seconds: 2.0,
                    text: "line 2".to_string(),
                },
                crate::core::model::track_content::LyricLine {
                    timestamp_seconds: 3.0,
                    text: "line 3".to_string(),
                },
            ],
            refresh_token: "fake-lyrics".to_string(),
        });
        self.apply_runtime_snapshot(
            crate::core::model::playback_runtime::PlaybackRuntimeSnapshot {
                generation: 1,
                playback_state: "playing".to_string(),
                current_source_ref: Some("Favorites".to_string()),
                current_song_id: Some(7),
                current_index: Some(0),
                position_seconds: Some(2.1),
                duration_seconds: Some(212.0),
                playback_mode: crate::core::model::playback_mode::PlaybackMode::Ordered,
                volume_percent: 100,
                muted: false,
                last_error_code: None,
            },
        );
    }

    /// 为测试填充一组较长标题的曲目预览列表。
    ///
    /// # 参数
    /// - `count`：要生成的预览条目数量
    ///
    /// # 返回值
    /// - 无
    pub fn load_fake_track_list_for_test(&mut self, count: usize) {
        self.preview_songs = (0..count)
            .map(|index| PreviewSongRow {
                song_id: index as i64 + 1,
                title: format!("A very long preview title number {index} for truncation"),
            })
            .collect();
        self.preview_titles = self
            .preview_songs
            .iter()
            .map(|song| song.title.clone())
            .collect();
        self.track_viewport.scroll_top = 0;
    }

    /// 为测试同步预览列表视口。
    ///
    /// # 参数
    /// - `visible_height`：可见高度
    ///
    /// # 返回值
    /// - 无
    pub fn sync_viewports_for_test(&mut self, visible_height: usize) {
        self.track_viewport.visible_height = visible_height;
        self.track_viewport
            .follow_selection(self.selected_preview_index, self.preview_titles.len());
    }

    /// 用 TUI 聚合快照刷新本地状态。
    ///
    /// # 参数
    /// - `snapshot`：TUI 聚合快照
    ///
    /// # 返回值
    /// - 无
    pub fn apply_tui_snapshot(&mut self, snapshot: crate::core::model::tui::TuiSnapshot) {
        let crate::core::model::tui::TuiSnapshot {
            player,
            active_task,
            playlist_browser,
            current_track,
        } = snapshot;

        self.current_track_song_id = current_track.song_id;
        self.current_track_lyrics = current_track.lyrics;
        self.current_track_cover_summary = None;
        self.apply_snapshot(player);
        self.active_task = active_task;
        self.playlist_browser = playlist_browser;
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

        self.playlist_state.select(
            self.playlist_browser
                .visible_playlists
                .iter()
                .position(|playlist| Some(playlist.name.as_str()) == self.selected_playlist_name()),
        );
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

    /// 按索引切换当前歌单选择。
    ///
    /// # 参数
    /// - `index`：目标歌单索引
    ///
    /// # 返回值
    /// - `Option<crate::tui::event::Intent>`：选择变化时返回后续要执行的意图
    pub fn select_playlist_index(&mut self, index: usize) -> Option<crate::tui::event::Intent> {
        let next_name = self
            .playlist_browser
            .visible_playlists
            .get(index)
            .map(|playlist| playlist.name.clone())?;

        self.focus = FocusArea::PlaylistList;
        if self.selected_playlist_name.as_deref() == Some(next_name.as_str()) {
            return None;
        }

        self.selected_playlist_name = Some(next_name);
        self.playlist_state.select(Some(index));
        self.preview_name = None;
        self.preview_error = None;
        self.preview_loading = false;
        self.preview_songs.clear();
        self.preview_titles.clear();
        self.selected_preview_index = 0;
        self.preview_state.select(None);
        Some(crate::tui::event::Intent::Action(
            crate::tui::event::ActionId::LoadPreview,
        ))
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

    /// 按索引切换当前预览选择。
    ///
    /// # 参数
    /// - `index`：目标预览索引
    ///
    /// # 返回值
    /// - 无
    pub fn select_preview_index(&mut self, index: usize) {
        if index < self.preview_titles.len() {
            self.focus = FocusArea::PlaylistPreview;
            self.selected_preview_index = index;
            self.preview_state.select(Some(index));
        }
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
        self.preview_songs = preview
            .songs
            .iter()
            .map(|song| PreviewSongRow {
                song_id: song.id,
                title: song.title.clone(),
            })
            .collect();
        self.preview_titles = self
            .preview_songs
            .iter()
            .map(|song| song.title.clone())
            .collect();
        self.preview_loading = false;
        self.preview_error = None;
        if self.selected_preview_index >= self.preview_titles.len() {
            self.selected_preview_index = self.preview_titles.len().saturating_sub(1);
        }
        self.preview_state
            .select((!self.preview_titles.is_empty()).then_some(self.selected_preview_index));
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
        self.preview_state.select(None);
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
        self.preview_songs.clear();
        self.preview_titles.clear();
        self.selected_preview_index = 0;
        self.preview_state.select(None);
    }

    /// 设置当前启动目录上下文。
    ///
    /// # 参数
    /// - `launch_cwd`：启动时捕获的当前目录
    ///
    /// # 返回值
    /// - 无
    pub fn set_launch_cwd(&mut self, launch_cwd: impl Into<String>) {
        self.launch_cwd = Some(launch_cwd.into());
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

        if let Some(backend_notice) = &self.player.backend_notice {
            status.push_str(" | backend_notice=");
            status.push_str(backend_notice);
        }

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

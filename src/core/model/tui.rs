use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::core::model::player::PlayerSnapshot;
use crate::core::model::runtime_task::RuntimeTaskSnapshot;

/// TUI 首页默认视图类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum TuiViewKind {
    /// 歌单浏览视图。
    Playlist,
}

/// 歌单列表项快照。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, ToSchema)]
pub struct PlaylistListItem {
    /// 歌单名称。
    pub name: String,
    /// 歌单类型。
    pub kind: String,
    /// 歌曲数量。
    pub count: usize,
    /// 是否为当前播放来源。
    pub is_current_playing_source: bool,
    /// 是否为临时歌单。
    pub is_ephemeral: bool,
}

/// TUI 歌单浏览区域快照。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, ToSchema)]
pub struct PlaylistBrowserSnapshot {
    /// 默认打开的视图。
    pub default_view: TuiViewKind,
    /// 默认选中的歌单名称。
    pub default_selected_playlist: Option<String>,
    /// 当前播放来源歌单。
    pub current_playing_playlist: Option<PlaylistListItem>,
    /// 当前可见的歌单列表。
    pub visible_playlists: Vec<PlaylistListItem>,
}

impl Default for PlaylistBrowserSnapshot {
    /// 构造默认歌单浏览快照。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - `Self`：默认歌单浏览快照
    fn default() -> Self {
        Self {
            default_view: TuiViewKind::Playlist,
            default_selected_playlist: None,
            current_playing_playlist: None,
            visible_playlists: Vec::new(),
        }
    }
}

/// 提供给 TUI 的聚合快照。
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, ToSchema)]
pub struct TuiSnapshot {
    /// 当前播放器快照。
    pub player: PlayerSnapshot,
    /// 当前活动运行时任务。
    pub active_task: Option<RuntimeTaskSnapshot>,
    /// 歌单浏览区域快照。
    pub playlist_browser: PlaylistBrowserSnapshot,
}

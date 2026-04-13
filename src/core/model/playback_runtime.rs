use serde::{Deserialize, Serialize};

use crate::core::model::playback_mode::PlaybackMode;

/// daemon 远端播放模式下的轻量运行时快照。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlaybackRuntimeSnapshot {
    /// 当前运行时快照代次。
    pub generation: u64,
    /// 当前播放状态文本。
    pub playback_state: String,
    /// 当前播放来源引用。
    pub current_source_ref: Option<String>,
    /// 当前歌曲 ID。
    pub current_song_id: Option<i64>,
    /// 当前来源内索引。
    pub current_index: Option<usize>,
    /// 当前播放秒数。
    pub position_seconds: Option<f64>,
    /// 当前歌曲总秒数。
    pub duration_seconds: Option<f64>,
    /// 当前用户可见播放模式。
    pub playback_mode: PlaybackMode,
    /// 当前音量百分比。
    pub volume_percent: u8,
    /// 当前是否静音。
    pub muted: bool,
    /// 最近一次错误码。
    pub last_error_code: Option<String>,
}

/// 新客户端初始化所需的低频 bootstrap 快照。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClientBootstrapSnapshot {
    /// 初始化时的播放运行时快照。
    pub runtime: PlaybackRuntimeSnapshot,
    /// 默认播放模式配置。
    pub default_playback_mode: PlaybackMode,
    /// 初始化时当前播放来源引用。
    pub current_source_ref: Option<String>,
}

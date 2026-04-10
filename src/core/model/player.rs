use serde::{Deserialize, Serialize};

/// 播放器生命周期状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub enum PlaybackState {
    /// 当前没有可继续使用的播放上下文。
    Idle,
    /// 当前正在播放有效队列项。
    Playing,
    /// 当前保留播放上下文，但处于暂停状态。
    Paused,
    /// 当前保留播放上下文，但被显式停止。
    Stopped,
    /// 最近一次控制或后端调用失败，需要对外暴露错误。
    Error,
}

impl PlaybackState {
    /// 返回对外契约使用的稳定状态字符串。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - `&'static str`：稳定状态名
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Playing => "playing",
            Self::Paused => "paused",
            Self::Stopped => "stopped",
            Self::Error => "error",
        }
    }
}

/// 对外暴露的播放器错误信息。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct PlayerErrorInfo {
    /// 稳定错误码，供 CLI / TUI / API 统一解释。
    pub code: String,
    /// 面向用户或调用方的错误说明。
    pub message: String,
}

/// 播放队列中的单项。
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct QueueItem {
    /// 歌曲 ID。
    pub song_id: i64,
    /// 文件路径。
    pub path: String,
    /// 标题。
    pub title: String,
    /// 时长。
    pub duration_seconds: Option<f64>,
}

/// 当前正在播放的歌曲摘要。
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct NowPlayingSong {
    /// 歌曲 ID。
    pub song_id: i64,
    /// 标题。
    pub title: String,
    /// 时长。
    pub duration_seconds: Option<f64>,
}

/// 对外暴露的播放器状态快照。
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct PlayerSnapshot {
    /// 播放状态。
    pub playback_state: String,
    /// 当前歌曲。
    pub current_song: Option<NowPlayingSong>,
    /// 队列总长度。
    pub queue_len: usize,
    /// 当前队列索引。
    pub queue_index: Option<usize>,
    /// 是否存在下一首。
    pub has_next: bool,
    /// 是否存在上一首。
    pub has_prev: bool,
    /// 最近一次对外可见错误。
    pub last_error: Option<PlayerErrorInfo>,
    /// 快照版本号，仅在有效状态变更后递增。
    pub version: u64,
    /// 当前播放进度秒数。
    pub position_seconds: Option<f64>,
    /// 当前播放进度占比，范围在 `0.0..=1.0`。
    pub position_fraction: Option<f64>,
}

impl Default for PlayerSnapshot {
    /// 返回空闲态下的默认快照，便于各消费面共享同一基线状态。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - `Self`：默认播放器快照
    fn default() -> Self {
        Self {
            playback_state: PlaybackState::Idle.as_str().to_string(),
            current_song: None,
            queue_len: 0,
            queue_index: None,
            has_next: false,
            has_prev: false,
            last_error: None,
            version: 0,
            position_seconds: None,
            position_fraction: None,
        }
    }
}

use serde::Serialize;

/// 播放队列中的单项。
#[derive(Debug, Clone)]
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
#[derive(Debug, Clone, Serialize)]
pub struct NowPlayingSong {
    /// 歌曲 ID。
    pub song_id: i64,
    /// 标题。
    pub title: String,
    /// 时长。
    pub duration_seconds: Option<f64>,
}

/// 对外暴露的播放器状态快照。
#[derive(Debug, Clone, Default, Serialize)]
pub struct PlayerSnapshot {
    /// 播放状态。
    pub playback_state: String,
    /// 当前歌曲。
    pub current_song: Option<NowPlayingSong>,
    /// 队列总长度。
    pub queue_len: usize,
    /// 当前队列索引。
    pub queue_index: Option<usize>,
}

use tokio::sync::broadcast;

/// 播放后端回传给服务层的运行时事件。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlaybackRuntimeEvent {
    /// 当前 generation 对应的曲目自然播放结束。
    TrackEnded { generation: u64 },
}

/// 播放运行时事件订阅器。
pub type PlaybackRuntimeReceiver = broadcast::Receiver<PlaybackRuntimeEvent>;

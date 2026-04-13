use tokio::sync::broadcast;

/// 播放停止的结构化原因。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackStopReason {
    /// 曲目自然播放结束。
    NaturalEof,
    /// 用户通过应用主动发起停止。
    UserStop,
    /// 用户直接关闭了后端进程或窗口。
    UserClosedBackend,
    /// 后端异常退出或 IPC 中断。
    BackendAborted,
}

/// 播放后端回传给服务层的运行时事件。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlaybackRuntimeEvent {
    /// 当前 generation 对应的播放已停止，并附带停止原因。
    PlaybackStopped {
        generation: u64,
        reason: PlaybackStopReason,
    },
}

/// 播放运行时事件订阅器。
pub type PlaybackRuntimeReceiver = broadcast::Receiver<PlaybackRuntimeEvent>;

#[cfg(test)]
mod tests;

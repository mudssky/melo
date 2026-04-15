/// TUI 本地播放时钟。
#[derive(Debug, Clone)]
pub struct PlaybackClock {
    /// 最近一次从 daemon 收到的位置锚点。
    anchor_position_seconds: Option<f64>,
    /// 收到锚点时的本地单调时钟时间。
    anchor_received_at: Option<std::time::Instant>,
    /// 最近一次收到的播放状态。
    playback_state: String,
    /// 最近一次收到的时长。
    duration_seconds: Option<f64>,
    /// 最近一次收到的代次。
    generation: u64,
}

impl Default for PlaybackClock {
    /// 构造一个空闲态本地播放时钟。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - `Self`：初始化后的本地播放时钟
    fn default() -> Self {
        Self {
            anchor_position_seconds: None,
            anchor_received_at: None,
            playback_state: "idle".to_string(),
            duration_seconds: None,
            generation: 0,
        }
    }
}

impl PlaybackClock {
    /// 用新的 runtime 快照更新本地时间锚点。
    ///
    /// # 参数
    /// - `runtime`：daemon 下发的轻量运行时快照
    /// - `received_at`：本地收到该快照的单调时钟时间
    ///
    /// # 返回值
    /// - 无
    pub fn apply_runtime(
        &mut self,
        runtime: &crate::core::model::playback_runtime::PlaybackRuntimeSnapshot,
        received_at: std::time::Instant,
    ) {
        const DRIFT_THRESHOLD_SECONDS: f64 = 0.75;

        let replace_anchor = match (
            self.display_position(received_at),
            runtime.position_seconds,
            runtime.generation != self.generation,
        ) {
            (_, None, _) => true,
            (None, Some(_), _) => true,
            (_, Some(_), true) => true,
            (Some(current), Some(next), false) => (current - next).abs() >= DRIFT_THRESHOLD_SECONDS,
        };

        if replace_anchor {
            self.anchor_position_seconds = runtime.position_seconds;
            self.anchor_received_at = Some(received_at);
        }

        self.playback_state = runtime.playback_state.clone();
        self.duration_seconds = runtime.duration_seconds;
        self.generation = runtime.generation;
    }

    /// 计算当前时刻应显示的本地播放位置。
    ///
    /// # 参数
    /// - `now`：当前本地单调时钟时间
    ///
    /// # 返回值
    /// - `Option<f64>`：可显示时返回当前秒数
    pub fn display_position(&self, now: std::time::Instant) -> Option<f64> {
        let base = self.anchor_position_seconds?;
        let duration = self.duration_seconds.unwrap_or(f64::MAX);

        if self.playback_state != "playing" {
            return Some(base.min(duration));
        }

        let anchor_received_at = self.anchor_received_at?;
        Some((base + now.duration_since(anchor_received_at).as_secs_f64()).min(duration))
    }
}

#[cfg(test)]
mod tests;

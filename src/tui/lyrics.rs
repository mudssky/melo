/// 歌词跟随模式。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LyricFollowMode {
    /// 自动跟随当前句。
    FollowCurrent,
    /// 用户正在手动浏览歌词。
    ManualBrowse,
    /// 已停止滚动，等待恢复自动跟随。
    ResumePending,
}

/// 歌词自动跟随与手动浏览状态。
#[derive(Debug, Clone)]
pub struct LyricFollowState {
    /// 手动浏览后恢复自动跟随的延迟。
    resume_delay: std::time::Duration,
    /// 当前歌词跟随模式。
    mode: LyricFollowMode,
    /// 最近一次手动滚动发生时间。
    last_manual_at: Option<std::time::Instant>,
}

impl LyricFollowState {
    /// 创建新的歌词跟随状态。
    ///
    /// # 参数
    /// - `resume_delay`：手动浏览后恢复自动跟随的延迟
    ///
    /// # 返回值
    /// - `Self`：初始化后的歌词跟随状态
    pub fn new(resume_delay: std::time::Duration) -> Self {
        Self {
            resume_delay,
            mode: LyricFollowMode::FollowCurrent,
            last_manual_at: None,
        }
    }

    /// 返回当前歌词跟随模式。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - `LyricFollowMode`：当前模式
    pub fn mode(&self) -> LyricFollowMode {
        self.mode.clone()
    }

    /// 记录一次手动滚动。
    ///
    /// # 参数
    /// - `now`：当前时间
    ///
    /// # 返回值
    /// - 无
    pub fn on_manual_scroll(&mut self, now: std::time::Instant) {
        self.mode = LyricFollowMode::ManualBrowse;
        self.last_manual_at = Some(now);
    }

    /// 记录一次手动滚动，暂停自动跟随。
    ///
    /// # 参数
    /// - `now`：当前时间
    ///
    /// # 返回值
    /// - 无
    pub fn pause_for_manual_scroll(&mut self, now: std::time::Instant) {
        self.on_manual_scroll(now);
    }

    /// 判断当前是否处于手动浏览状态。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - `bool`：是否处于手动浏览状态
    pub fn is_manual_browse(&self) -> bool {
        !matches!(self.mode, LyricFollowMode::FollowCurrent)
    }

    /// 判断当前是否已达到恢复自动跟随的时间点。
    ///
    /// # 参数
    /// - `now`：当前时间
    ///
    /// # 返回值
    /// - `bool`：是否应恢复自动跟随
    pub fn should_resume(&self, now: std::time::Instant) -> bool {
        self.last_manual_at
            .is_some_and(|paused_at| now.duration_since(paused_at) >= self.resume_delay)
    }

    /// 按当前时间推进歌词跟随状态机。
    ///
    /// # 参数
    /// - `now`：当前时间
    ///
    /// # 返回值
    /// - 无
    pub fn tick(&mut self, now: std::time::Instant) {
        match self.mode {
            LyricFollowMode::FollowCurrent => {}
            LyricFollowMode::ManualBrowse => {
                self.mode = LyricFollowMode::ResumePending;
            }
            LyricFollowMode::ResumePending => {
                if self.should_resume(now) {
                    self.resume_now();
                }
            }
        }
    }

    /// 立即恢复自动跟随。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - 无
    pub fn resume_now(&mut self) {
        self.mode = LyricFollowMode::FollowCurrent;
        self.last_manual_at = None;
    }
}

#[cfg(test)]
mod tests;

/// 歌词自动跟随与手动浏览状态。
#[derive(Debug, Clone)]
pub struct LyricFollowState {
    resume_delay: std::time::Duration,
    paused_at: Option<std::time::Instant>,
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
            paused_at: None,
        }
    }

    /// 记录一次手动滚动，暂停自动跟随。
    ///
    /// # 参数
    /// - `now`：当前时间
    ///
    /// # 返回值
    /// - 无
    pub fn pause_for_manual_scroll(&mut self, now: std::time::Instant) {
        self.paused_at = Some(now);
    }

    /// 判断当前是否处于手动浏览状态。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - `bool`：是否处于手动浏览状态
    pub fn is_manual_browse(&self) -> bool {
        self.paused_at.is_some()
    }

    /// 判断当前是否已达到恢复自动跟随的时间点。
    ///
    /// # 参数
    /// - `now`：当前时间
    ///
    /// # 返回值
    /// - `bool`：是否应恢复自动跟随
    pub fn should_resume(&self, now: std::time::Instant) -> bool {
        self.paused_at
            .is_some_and(|paused_at| now.duration_since(paused_at) >= self.resume_delay)
    }
}

#[cfg(test)]
mod tests;

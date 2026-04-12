use std::sync::RwLock;

/// 当前播放来源的 daemon 内存态。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayingPlaylistContext {
    /// 当前播放来源名称。
    pub name: String,
    /// 当前播放来源类型。
    pub kind: String,
}

/// 当前播放来源存储。
#[derive(Debug, Default)]
pub struct PlayingPlaylistStore {
    current: RwLock<Option<PlayingPlaylistContext>>,
}

impl PlayingPlaylistStore {
    /// 读取当前播放来源。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - `Option<PlayingPlaylistContext>`：当前播放来源
    pub fn current(&self) -> Option<PlayingPlaylistContext> {
        self.current.read().ok().and_then(|guard| guard.clone())
    }

    /// 设置当前播放来源。
    ///
    /// # 参数
    /// - `context`：新的当前播放来源
    ///
    /// # 返回值
    /// - 无
    pub fn set(&self, context: PlayingPlaylistContext) {
        if let Ok(mut guard) = self.current.write() {
            *guard = Some(context);
        }
    }

    /// 清空当前播放来源。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - 无
    pub fn clear(&self) {
        if let Ok(mut guard) = self.current.write() {
            *guard = None;
        }
    }
}

#[cfg(test)]
mod tests;

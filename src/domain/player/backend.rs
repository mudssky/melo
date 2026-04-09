/// 后端接收到的播放命令。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlaybackCommand {
    Load(std::path::PathBuf),
    Pause,
    Resume,
    Stop,
}

/// 播放后端抽象，便于测试替身与真实音频输出解耦。
pub trait PlaybackBackend: Send + Sync {
    /// 加载并立即播放文件。
    ///
    /// # 参数
    /// - `path`：待播放文件路径
    ///
    /// # 返回
    /// - `MeloResult<()>`：执行结果
    fn load_and_play(&self, path: &std::path::Path) -> crate::core::error::MeloResult<()>;

    /// 暂停当前播放。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回
    /// - `MeloResult<()>`：执行结果
    fn pause(&self) -> crate::core::error::MeloResult<()>;

    /// 恢复播放。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回
    /// - `MeloResult<()>`：执行结果
    fn resume(&self) -> crate::core::error::MeloResult<()>;

    /// 停止播放。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回
    /// - `MeloResult<()>`：执行结果
    fn stop(&self) -> crate::core::error::MeloResult<()>;
}

/// 空实现后端，便于测试 API 宿主等不需要真实声音输出的场景。
#[derive(Default)]
pub struct NoopBackend;

impl PlaybackBackend for NoopBackend {
    fn load_and_play(&self, _path: &std::path::Path) -> crate::core::error::MeloResult<()> {
        Ok(())
    }

    fn pause(&self) -> crate::core::error::MeloResult<()> {
        Ok(())
    }

    fn resume(&self) -> crate::core::error::MeloResult<()> {
        Ok(())
    }

    fn stop(&self) -> crate::core::error::MeloResult<()> {
        Ok(())
    }
}

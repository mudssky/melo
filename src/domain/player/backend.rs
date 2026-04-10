use tokio::sync::broadcast;

use crate::domain::player::runtime::{PlaybackRuntimeEvent, PlaybackRuntimeReceiver};

/// 后端接收到的播放命令。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlaybackCommand {
    Load {
        path: std::path::PathBuf,
        generation: u64,
    },
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
    /// - `generation`：当前播放代次，用于忽略过期结束事件
    ///
    /// # 返回
    /// - `MeloResult<()>`：执行结果
    fn load_and_play(
        &self,
        path: &std::path::Path,
        generation: u64,
    ) -> crate::core::error::MeloResult<()>;

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

    /// 订阅后端运行时事件。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回
    /// - `PlaybackRuntimeReceiver`：运行时事件订阅器
    fn subscribe_runtime_events(&self) -> PlaybackRuntimeReceiver;
}

/// 空实现后端，便于测试 API 宿主等不需要真实声音输出的场景。
#[derive(Default)]
pub struct NoopBackend;

impl PlaybackBackend for NoopBackend {
    fn load_and_play(
        &self,
        _path: &std::path::Path,
        _generation: u64,
    ) -> crate::core::error::MeloResult<()> {
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

    fn subscribe_runtime_events(&self) -> broadcast::Receiver<PlaybackRuntimeEvent> {
        let (_tx, rx) = broadcast::channel(1);
        rx
    }
}

use std::sync::Mutex;

use crate::core::error::{MeloError, MeloResult};
use crate::domain::player::backend::PlaybackBackend;

/// 基于 `rodio` 的真实播放后端。
pub struct RodioBackend {
    sink: rodio::MixerDeviceSink,
    player: Mutex<Option<rodio::Player>>,
}

impl RodioBackend {
    /// 创建新的 `Rodio` 播放后端。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - `MeloResult<Self>`：初始化后的播放后端
    pub fn new() -> MeloResult<Self> {
        let sink = rodio::DeviceSinkBuilder::open_default_sink()
            .map_err(|err| MeloError::Message(err.to_string()))?;
        Ok(Self {
            sink,
            player: Mutex::new(None),
        })
    }
}

impl PlaybackBackend for RodioBackend {
    /// 加载并立即播放给定文件。
    ///
    /// # 参数
    /// - `path`：待播放音频文件路径
    ///
    /// # 返回值
    /// - `MeloResult<()>`：执行结果
    fn load_and_play(&self, path: &std::path::Path) -> MeloResult<()> {
        let file = std::fs::File::open(path).map_err(|err| MeloError::Message(err.to_string()))?;
        let decoder =
            rodio::Decoder::try_from(file).map_err(|err| MeloError::Message(err.to_string()))?;
        let player = rodio::Player::connect_new(self.sink.mixer());
        player.append(decoder);
        player.play();

        let mut current_player = self.player.lock().unwrap();
        if let Some(previous_player) = current_player.take() {
            previous_player.stop();
        }
        *current_player = Some(player);
        Ok(())
    }

    /// 暂停当前播放。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - `MeloResult<()>`：执行结果
    fn pause(&self) -> MeloResult<()> {
        if let Some(player) = self.player.lock().unwrap().as_ref() {
            player.pause();
        }
        Ok(())
    }

    /// 恢复当前播放。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - `MeloResult<()>`：执行结果
    fn resume(&self) -> MeloResult<()> {
        if let Some(player) = self.player.lock().unwrap().as_ref() {
            player.play();
        }
        Ok(())
    }

    /// 停止当前播放。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - `MeloResult<()>`：执行结果
    fn stop(&self) -> MeloResult<()> {
        if let Some(player) = self.player.lock().unwrap().take() {
            player.stop();
        }
        Ok(())
    }
}

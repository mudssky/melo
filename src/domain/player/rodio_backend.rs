use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use tokio::sync::broadcast;

use crate::core::error::{MeloError, MeloResult};
use crate::domain::player::backend::{
    PlaybackBackend, PlaybackSessionHandle, PlaybackStartRequest,
};
use crate::domain::player::runtime::{
    PlaybackRuntimeEvent, PlaybackRuntimeReceiver, PlaybackStopReason,
};

/// 基于 `rodio` 的真实播放后端。
pub struct RodioBackend {
    sink: rodio::MixerDeviceSink,
    player: Arc<Mutex<Option<Arc<rodio::Player>>>>,
    active_generation: Arc<AtomicU64>,
}

struct RodioPlaybackSession {
    player: Arc<Mutex<Option<Arc<rodio::Player>>>>,
    runtime_tx: broadcast::Sender<PlaybackRuntimeEvent>,
    active_generation: Arc<AtomicU64>,
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
            player: Arc::new(Mutex::new(None)),
            active_generation: Arc::new(AtomicU64::new(0)),
        })
    }

    /// 在后台线程等待当前播放器自然结束，并在仍是活跃 generation 时上报停止事件。
    ///
    /// # 参数
    /// - `runtime_tx`：运行时事件发送器
    /// - `active_generation`：当前活跃播放代次
    /// - `generation`：本次播放对应的代次
    /// - `player`：本次播放对应的 `rodio::Player`
    ///
    /// # 返回值
    /// - 无
    fn spawn_track_end_watcher(
        runtime_tx: broadcast::Sender<PlaybackRuntimeEvent>,
        active_generation: Arc<AtomicU64>,
        generation: u64,
        player: Arc<rodio::Player>,
    ) {
        std::thread::spawn(move || {
            player.sleep_until_end();
            let current_generation = active_generation.load(Ordering::SeqCst);
            if should_emit_track_end(current_generation, generation, player.empty()) {
                let _ = runtime_tx.send(PlaybackRuntimeEvent::PlaybackStopped {
                    generation,
                    reason: PlaybackStopReason::NaturalEof,
                });
            }
        });
    }
}

/// 判断一次播放器结束是否应该对外发送自然结束事件。
///
/// # 参数
/// - `active_generation`：当前后端记录的活跃代次
/// - `generation`：结束事件所属的代次
/// - `player_is_empty`：播放器是否已经没有待播音频
///
/// # 返回值
/// - `bool`：是否应该发送结束事件
fn should_emit_track_end(active_generation: u64, generation: u64, player_is_empty: bool) -> bool {
    active_generation == generation && player_is_empty
}

impl PlaybackBackend for RodioBackend {
    /// 返回当前后端名称。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - `&'static str`：后端稳定名称
    fn backend_name(&self) -> &'static str {
        "rodio"
    }

    /// 创建并启动一个新的 `rodio` 播放会话。
    ///
    /// # 参数
    /// - `request`：播放启动参数
    ///
    /// # 返回值
    /// - `MeloResult<Box<dyn PlaybackSessionHandle>>`：单次播放控制句柄
    fn start_session(
        &self,
        request: PlaybackStartRequest,
    ) -> MeloResult<Box<dyn PlaybackSessionHandle>> {
        let file = std::fs::File::open(&request.path)
            .map_err(|err| MeloError::Message(err.to_string()))?;
        let decoder =
            rodio::Decoder::try_from(file).map_err(|err| MeloError::Message(err.to_string()))?;
        let player = Arc::new(rodio::Player::connect_new(self.sink.mixer()));
        player.append(decoder);
        player.play();

        self.active_generation
            .store(request.generation, Ordering::SeqCst);
        let mut current_player = self.player.lock().unwrap();
        if let Some(previous_player) = current_player.replace(Arc::clone(&player)) {
            previous_player.stop();
        }
        drop(current_player);

        let (runtime_tx, _) = broadcast::channel(16);
        Self::spawn_track_end_watcher(
            runtime_tx.clone(),
            Arc::clone(&self.active_generation),
            request.generation,
            player,
        );
        let session = RodioPlaybackSession {
            player: Arc::clone(&self.player),
            runtime_tx,
            active_generation: Arc::clone(&self.active_generation),
        };
        session.set_volume(request.volume_factor)?;
        Ok(Box::new(session))
    }
}

impl PlaybackSessionHandle for RodioPlaybackSession {
    fn pause(&self) -> MeloResult<()> {
        if let Some(player) = self.player.lock().unwrap().as_ref() {
            player.pause();
        }
        Ok(())
    }

    fn resume(&self) -> MeloResult<()> {
        if let Some(player) = self.player.lock().unwrap().as_ref() {
            player.play();
        }
        Ok(())
    }

    fn stop(&self) -> MeloResult<()> {
        self.active_generation.store(0, Ordering::SeqCst);
        if let Some(player) = self.player.lock().unwrap().take() {
            player.stop();
        }
        Ok(())
    }

    fn subscribe_runtime_events(&self) -> PlaybackRuntimeReceiver {
        self.runtime_tx.subscribe()
    }

    fn current_position(&self) -> Option<std::time::Duration> {
        self.player
            .lock()
            .unwrap()
            .as_ref()
            .map(|player| player.get_pos())
    }

    fn set_volume(&self, factor: f32) -> MeloResult<()> {
        if let Some(player) = self.player.lock().unwrap().as_ref() {
            player.set_volume(factor);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;

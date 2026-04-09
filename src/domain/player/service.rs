use std::sync::Arc;

use tokio::sync::Mutex;

use crate::core::error::{MeloError, MeloResult};
use crate::core::model::player::{NowPlayingSong, PlayerSnapshot, QueueItem};
use crate::domain::player::backend::PlaybackBackend;

#[derive(Debug, Default)]
struct PlayerState {
    playback_state: String,
    queue: Vec<QueueItem>,
    current_index: Option<usize>,
}

/// 播放服务，负责维护内存队列与当前播放快照。
pub struct PlayerService {
    backend: Arc<dyn PlaybackBackend>,
    state: Mutex<PlayerState>,
}

impl PlayerService {
    /// 创建新的播放服务。
    ///
    /// # 参数
    /// - `backend`：播放后端
    ///
    /// # 返回
    /// - `Self`：播放服务
    pub fn new(backend: Arc<dyn PlaybackBackend>) -> Self {
        Self {
            backend,
            state: Mutex::new(PlayerState::default()),
        }
    }

    /// 向内存队列追加一首歌。
    ///
    /// # 参数
    /// - `item`：队列项
    ///
    /// # 返回
    /// - `MeloResult<()>`：写入结果
    pub async fn enqueue(&self, item: QueueItem) -> MeloResult<()> {
        let mut state = self.state.lock().await;
        state.queue.push(item);
        Ok(())
    }

    /// 启动播放，如果尚未选中当前项则默认播放队列第一首。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回
    /// - `MeloResult<()>`：执行结果
    pub async fn play(&self) -> MeloResult<()> {
        let mut state = self.state.lock().await;
        if state.current_index.is_none() {
            state.current_index = Some(0);
        }

        let index = state
            .current_index
            .ok_or_else(|| MeloError::Message("当前队列为空".to_string()))?;
        let item = state
            .queue
            .get(index)
            .cloned()
            .ok_or_else(|| MeloError::Message("当前队列为空".to_string()))?;

        self.backend
            .load_and_play(std::path::Path::new(&item.path))?;
        state.playback_state = "playing".to_string();
        Ok(())
    }

    /// 生成当前播放器快照。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回
    /// - `PlayerSnapshot`：当前状态快照
    pub async fn snapshot(&self) -> PlayerSnapshot {
        let state = self.state.lock().await;
        let current_song = state.current_index.and_then(|index| {
            state.queue.get(index).map(|item| NowPlayingSong {
                song_id: item.song_id,
                title: item.title.clone(),
                duration_seconds: item.duration_seconds,
            })
        });

        PlayerSnapshot {
            playback_state: state.playback_state.clone(),
            current_song,
            queue_len: state.queue.len(),
            queue_index: state.current_index,
        }
    }
}

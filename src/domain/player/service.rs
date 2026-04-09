use std::sync::Arc;

use tokio::sync::{Mutex, watch};

use crate::core::error::{MeloError, MeloResult};
use crate::core::model::player::{
    NowPlayingSong, PlaybackState, PlayerErrorInfo, PlayerSnapshot, QueueItem,
};
use crate::domain::player::backend::PlaybackBackend;
use crate::domain::player::queue::PlayerQueue;

/// 播放器内部会话状态，是唯一可写的播放器真相来源。
#[derive(Debug)]
struct PlayerSession {
    playback_state: PlaybackState,
    queue: PlayerQueue,
    last_error: Option<PlayerErrorInfo>,
    version: u64,
}

impl Default for PlayerSession {
    /// 构造空闲态播放器会话。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - `Self`：默认播放器会话
    fn default() -> Self {
        Self {
            playback_state: PlaybackState::Idle,
            queue: PlayerQueue::default(),
            last_error: None,
            version: 0,
        }
    }
}

/// 播放服务，负责维护播放器会话、协调后端并发布统一快照。
pub struct PlayerService {
    backend: Arc<dyn PlaybackBackend>,
    session: Mutex<PlayerSession>,
    snapshot_tx: watch::Sender<PlayerSnapshot>,
}

impl PlayerService {
    /// 创建新的播放服务。
    ///
    /// # 参数
    /// - `backend`：播放后端
    ///
    /// # 返回值
    /// - `Self`：播放服务
    pub fn new(backend: Arc<dyn PlaybackBackend>) -> Self {
        let session = PlayerSession::default();
        let (snapshot_tx, _snapshot_rx) = watch::channel(Self::snapshot_from_session(&session));
        Self {
            backend,
            session: Mutex::new(session),
            snapshot_tx,
        }
    }

    /// 创建播放器快照订阅器。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - `watch::Receiver<PlayerSnapshot>`：快照订阅器
    pub fn subscribe(&self) -> watch::Receiver<PlayerSnapshot> {
        self.snapshot_tx.subscribe()
    }

    /// 向队列尾部追加一首歌，并返回最新快照。
    ///
    /// # 参数
    /// - `item`：待追加队列项
    ///
    /// # 返回值
    /// - `MeloResult<PlayerSnapshot>`：最新快照
    pub async fn append(&self, item: QueueItem) -> MeloResult<PlayerSnapshot> {
        let mut session = self.session.lock().await;
        session.queue.append(item);
        session.last_error = None;
        self.publish_locked(&mut session)
    }

    /// 在指定位置插入一首歌，并返回最新快照。
    ///
    /// # 参数
    /// - `index`：插入位置
    /// - `item`：待插入队列项
    ///
    /// # 返回值
    /// - `MeloResult<PlayerSnapshot>`：最新快照
    pub async fn insert(&self, index: usize, item: QueueItem) -> MeloResult<PlayerSnapshot> {
        let mut session = self.session.lock().await;
        session.queue.insert(index, item)?;
        session.last_error = None;
        self.publish_locked(&mut session)
    }

    /// 向尾部追加一首歌，保留旧接口兼容性。
    ///
    /// # 参数
    /// - `item`：待追加队列项
    ///
    /// # 返回值
    /// - `MeloResult<()>`：执行结果
    pub async fn enqueue(&self, item: QueueItem) -> MeloResult<()> {
        self.append(item).await.map(|_| ())
    }

    /// 启动播放；若当前未选中条目，则默认从队首开始。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - `MeloResult<PlayerSnapshot>`：最新快照
    pub async fn play(&self) -> MeloResult<PlayerSnapshot> {
        let mut session = self.session.lock().await;
        if session.queue.is_empty() {
            return self.fail_locked(
                &mut session,
                "queue_empty",
                "queue is empty",
                MeloError::Message("queue is empty".to_string()),
            );
        }

        if session.queue.current_index().is_none() {
            let _ = session.queue.play_index(0)?;
        }

        let current = session
            .queue
            .current()
            .cloned()
            .ok_or_else(|| MeloError::Message("queue is empty".to_string()))?;
        let current_path = std::path::Path::new(&current.path);
        if !current_path.exists() {
            return self.fail_locked(
                &mut session,
                "track_file_missing",
                "track file is missing",
                MeloError::Message("track file is missing".to_string()),
            );
        }

        if let Err(err) = self.backend.load_and_play(current_path) {
            return self.fail_locked(&mut session, "backend_unavailable", &err.to_string(), err);
        }

        session.playback_state = PlaybackState::Playing;
        session.last_error = None;
        self.publish_locked(&mut session)
    }

    /// 暂停当前播放。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - `MeloResult<PlayerSnapshot>`：最新快照
    pub async fn pause(&self) -> MeloResult<PlayerSnapshot> {
        let mut session = self.session.lock().await;
        if session.playback_state != PlaybackState::Playing {
            return Ok(Self::snapshot_from_session(&session));
        }

        self.backend.pause()?;
        session.playback_state = PlaybackState::Paused;
        self.publish_locked(&mut session)
    }

    /// 在播放与暂停之间切换，其余状态退化为 `play`。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - `MeloResult<PlayerSnapshot>`：最新快照
    pub async fn toggle(&self) -> MeloResult<PlayerSnapshot> {
        let state = self.session.lock().await.playback_state;
        match state {
            PlaybackState::Playing => self.pause().await,
            PlaybackState::Paused => self.resume().await,
            PlaybackState::Idle | PlaybackState::Stopped | PlaybackState::Error => {
                self.play().await
            }
        }
    }

    /// 恢复暂停中的播放。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - `MeloResult<PlayerSnapshot>`：最新快照
    pub async fn resume(&self) -> MeloResult<PlayerSnapshot> {
        let mut session = self.session.lock().await;
        if session.playback_state != PlaybackState::Paused {
            return Ok(Self::snapshot_from_session(&session));
        }

        self.backend.resume()?;
        session.playback_state = PlaybackState::Playing;
        session.last_error = None;
        self.publish_locked(&mut session)
    }

    /// 停止当前播放，但保留队列与当前索引。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - `MeloResult<PlayerSnapshot>`：最新快照
    pub async fn stop(&self) -> MeloResult<PlayerSnapshot> {
        let mut session = self.session.lock().await;
        let target_state = if session.queue.is_empty() {
            PlaybackState::Idle
        } else {
            PlaybackState::Stopped
        };

        if session.playback_state == target_state {
            return Ok(Self::snapshot_from_session(&session));
        }

        self.backend.stop()?;
        session.playback_state = target_state;
        self.publish_locked(&mut session)
    }

    /// 切换到下一首并尝试播放。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - `MeloResult<PlayerSnapshot>`：最新快照
    pub async fn next(&self) -> MeloResult<PlayerSnapshot> {
        let mut session = self.session.lock().await;
        let next_index = match session.queue.current_index() {
            Some(index) if index + 1 < session.queue.len() => index + 1,
            _ => {
                return self.fail_locked(
                    &mut session,
                    "queue_no_next",
                    "queue has no next item",
                    MeloError::Message("queue has no next item".to_string()),
                );
            }
        };
        let _ = session.queue.play_index(next_index)?;
        drop(session);
        self.play().await
    }

    /// 切换到上一首并尝试播放。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - `MeloResult<PlayerSnapshot>`：最新快照
    pub async fn prev(&self) -> MeloResult<PlayerSnapshot> {
        let mut session = self.session.lock().await;
        let prev_index = match session.queue.current_index() {
            Some(index) if index > 0 => index - 1,
            _ => {
                return self.fail_locked(
                    &mut session,
                    "queue_no_prev",
                    "queue has no previous item",
                    MeloError::Message("queue has no previous item".to_string()),
                );
            }
        };
        let _ = session.queue.play_index(prev_index)?;
        drop(session);
        self.play().await
    }

    /// 选择指定队列项并立即播放。
    ///
    /// # 参数
    /// - `index`：目标索引
    ///
    /// # 返回值
    /// - `MeloResult<PlayerSnapshot>`：最新快照
    pub async fn play_index(&self, index: usize) -> MeloResult<PlayerSnapshot> {
        let mut session = self.session.lock().await;
        session
            .queue
            .play_index(index)
            .map_err(|_| MeloError::Message("queue index out of range".to_string()))?;
        drop(session);
        self.play().await
    }

    /// 清空整个播放队列，并将播放器重置为空闲态。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - `MeloResult<PlayerSnapshot>`：最新快照
    pub async fn clear(&self) -> MeloResult<PlayerSnapshot> {
        let mut session = self.session.lock().await;
        session.queue.clear();
        session.playback_state = PlaybackState::Idle;
        session.last_error = None;
        self.backend.stop()?;
        self.publish_locked(&mut session)
    }

    /// 删除指定队列项。
    ///
    /// # 参数
    /// - `index`：待删除索引
    ///
    /// # 返回值
    /// - `MeloResult<PlayerSnapshot>`：最新快照
    pub async fn remove(&self, index: usize) -> MeloResult<PlayerSnapshot> {
        let mut session = self.session.lock().await;
        let _ = session.queue.remove(index)?;
        if session.queue.is_empty() {
            session.playback_state = PlaybackState::Idle;
        }
        session.last_error = None;
        self.publish_locked(&mut session)
    }

    /// 移动指定队列项。
    ///
    /// # 参数
    /// - `from`：源索引
    /// - `to`：目标索引
    ///
    /// # 返回值
    /// - `MeloResult<PlayerSnapshot>`：最新快照
    pub async fn move_item(&self, from: usize, to: usize) -> MeloResult<PlayerSnapshot> {
        let mut session = self.session.lock().await;
        session.queue.move_item(from, to)?;
        session.last_error = None;
        self.publish_locked(&mut session)
    }

    /// 返回当前播放器快照。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - `PlayerSnapshot`：当前快照
    pub async fn snapshot(&self) -> PlayerSnapshot {
        let session = self.session.lock().await;
        Self::snapshot_from_session(&session)
    }

    /// 根据内部会话生成对外快照。
    ///
    /// # 参数
    /// - `session`：播放器会话
    ///
    /// # 返回值
    /// - `PlayerSnapshot`：对外快照
    fn snapshot_from_session(session: &PlayerSession) -> PlayerSnapshot {
        PlayerSnapshot {
            playback_state: session.playback_state.as_str().to_string(),
            current_song: session.queue.current().map(|item| NowPlayingSong {
                song_id: item.song_id,
                title: item.title.clone(),
                duration_seconds: item.duration_seconds,
            }),
            queue_len: session.queue.len(),
            queue_index: session.queue.current_index(),
            has_next: session.queue.has_next(),
            has_prev: session.queue.has_prev(),
            last_error: session.last_error.clone(),
            version: session.version,
        }
    }

    /// 在持锁状态下递增版本并广播最新快照。
    ///
    /// # 参数
    /// - `session`：播放器会话
    ///
    /// # 返回值
    /// - `MeloResult<PlayerSnapshot>`：最新快照
    fn publish_locked(&self, session: &mut PlayerSession) -> MeloResult<PlayerSnapshot> {
        session.version += 1;
        let snapshot = Self::snapshot_from_session(session);
        self.snapshot_tx.send_replace(snapshot.clone());
        Ok(snapshot)
    }

    /// 在持锁状态下记录错误、发布快照并返回失败。
    ///
    /// # 参数
    /// - `session`：播放器会话
    /// - `code`：稳定错误码
    /// - `message`：错误信息
    /// - `err`：原始错误
    ///
    /// # 返回值
    /// - `MeloResult<PlayerSnapshot>`：总是返回错误
    fn fail_locked(
        &self,
        session: &mut PlayerSession,
        code: &str,
        message: &str,
        err: MeloError,
    ) -> MeloResult<PlayerSnapshot> {
        session.playback_state = PlaybackState::Error;
        session.last_error = Some(PlayerErrorInfo {
            code: code.to_string(),
            message: message.to_string(),
        });
        let _ = self.publish_locked(session)?;
        Err(err)
    }
}

#[cfg(test)]
mod tests;

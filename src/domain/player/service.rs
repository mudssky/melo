use std::sync::Arc;

use tokio::sync::{Mutex, watch};

use crate::core::error::{MeloError, MeloResult};
use crate::core::model::player::{
    NowPlayingSong, PlaybackState, PlayerErrorInfo, PlayerSnapshot, QueueItem, RepeatMode,
};
use crate::domain::player::backend::PlaybackBackend;
use crate::domain::player::navigation::PlaybackNavigation;
use crate::domain::player::queue::PlayerQueue;
use crate::domain::player::runtime::PlaybackRuntimeEvent;
use crate::domain::player::session_store::PersistedPlayerSession;

/// 播放器内部会话状态，是唯一可写的播放器真相来源。
#[derive(Debug)]
struct PlayerSession {
    playback_state: PlaybackState,
    queue: PlayerQueue,
    last_error: Option<PlayerErrorInfo>,
    version: u64,
    playback_generation: u64,
    position_seconds: Option<f64>,
    volume_percent: u8,
    muted: bool,
    repeat_mode: RepeatMode,
    shuffle_enabled: bool,
    shuffle_seed: u64,
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
            playback_generation: 0,
            position_seconds: None,
            volume_percent: 100,
            muted: false,
            repeat_mode: RepeatMode::Off,
            shuffle_enabled: false,
            shuffle_seed: 0,
        }
    }
}

/// 播放服务，负责维护播放器会话、协调后端并发布统一快照。
pub struct PlayerService {
    backend: Arc<dyn PlaybackBackend>,
    backend_name: &'static str,
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
        let backend_name = backend.backend_name();
        let (snapshot_tx, _snapshot_rx) =
            watch::channel(Self::snapshot_from_session(&session, backend_name));
        Self {
            backend,
            backend_name,
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

    /// 启动后端运行时事件消费循环。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - 无
    pub fn start_runtime_event_loop(self: &Arc<Self>) {
        let mut receiver = self.backend.subscribe_runtime_events();
        let service = Arc::clone(self);
        tokio::spawn(async move {
            while let Ok(event) = receiver.recv().await {
                service.handle_runtime_event(event).await;
            }
        });
    }

    /// 启动播放进度轮询循环。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - 无
    pub fn start_progress_loop(self: &Arc<Self>) {
        let mut ticker = tokio::time::interval(std::time::Duration::from_millis(500));
        let service = Arc::clone(self);
        tokio::spawn(async move {
            loop {
                ticker.tick().await;
                let _ = service.refresh_progress_once().await;
            }
        });
    }

    /// 基于当前会话构造导航器。
    ///
    /// # 参数
    /// - `session`：播放器会话
    ///
    /// # 返回值
    /// - `PlaybackNavigation`：导航规则计算器
    fn navigation(session: &PlayerSession) -> PlaybackNavigation {
        if session.shuffle_enabled {
            PlaybackNavigation::shuffled(
                session.queue.len(),
                session.queue.current_index(),
                session.shuffle_seed,
            )
        } else {
            PlaybackNavigation::linear(session.queue.len(), session.queue.current_index())
        }
    }

    /// 计算当前会话应下发给后端的音量系数。
    ///
    /// # 参数
    /// - `session`：播放器会话
    ///
    /// # 返回值
    /// - `f32`：后端音量系数
    fn volume_factor(session: &PlayerSession) -> f32 {
        if session.muted {
            0.0
        } else {
            session.volume_percent as f32 / 100.0
        }
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

        let generation = session.playback_generation + 1;
        if let Err(err) = self.backend.load_and_play(current_path, generation) {
            return self.fail_locked(&mut session, "backend_unavailable", &err.to_string(), err);
        }
        if let Err(err) = self.backend.set_volume(Self::volume_factor(&session)) {
            return self.fail_locked(&mut session, "backend_unavailable", &err.to_string(), err);
        }

        session.playback_generation = generation;
        session.playback_state = PlaybackState::Playing;
        session.last_error = None;
        session.position_seconds = Some(0.0);
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
            return Ok(Self::snapshot_from_session(&session, self.backend_name));
        }

        self.backend.pause()?;
        if let Some(position) = self.backend.current_position() {
            session.position_seconds = Some(position.as_secs_f64());
        }
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
            return Ok(Self::snapshot_from_session(&session, self.backend_name));
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
            return Ok(Self::snapshot_from_session(&session, self.backend_name));
        }

        self.backend.stop()?;
        session.playback_state = target_state;
        session.position_seconds = session.queue.current().map(|_| 0.0);
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
        let Some(next_index) =
            Self::navigation(&session).next_index(session.repeat_mode, session.shuffle_enabled)
        else {
            return self.fail_locked(
                &mut session,
                "queue_no_next",
                "queue has no next item",
                MeloError::Message("queue has no next item".to_string()),
            );
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
        let Some(prev_index) =
            Self::navigation(&session).prev_index(session.repeat_mode, session.shuffle_enabled)
        else {
            return self.fail_locked(
                &mut session,
                "queue_no_prev",
                "queue has no previous item",
                MeloError::Message("queue has no previous item".to_string()),
            );
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
        session.position_seconds = None;
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
            session.position_seconds = None;
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
        Self::snapshot_from_session(&session, self.backend_name)
    }

    /// 导出当前播放器会话，用于持久化保存。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - `PersistedPlayerSession`：可持久化的播放器会话
    pub async fn export_persisted_session(&self) -> PersistedPlayerSession {
        let session = self.session.lock().await;
        PersistedPlayerSession {
            playback_state: session.playback_state,
            queue_index: session.queue.current_index(),
            position_seconds: session.position_seconds,
            queue: session.queue.items().to_vec(),
        }
    }

    /// 恢复一份已经持久化的播放器会话。
    ///
    /// # 参数
    /// - `persisted`：待恢复的播放器会话
    ///
    /// # 返回值
    /// - `MeloResult<PlayerSnapshot>`：恢复后的最新快照
    pub async fn restore_persisted_session(
        &self,
        persisted: PersistedPlayerSession,
    ) -> MeloResult<PlayerSnapshot> {
        let mut session = self.session.lock().await;
        session.queue = PlayerQueue::from_items(persisted.queue, persisted.queue_index);
        session.position_seconds = if session.queue.current().is_some() {
            persisted.position_seconds
        } else {
            None
        };
        session.last_error = None;
        session.playback_state = match persisted.playback_state {
            PlaybackState::Playing | PlaybackState::Paused => PlaybackState::Stopped,
            other => other,
        };
        self.publish_locked(&mut session)
    }

    /// 设置播放器音量百分比。
    ///
    /// # 参数
    /// - `volume_percent`：目标音量百分比
    ///
    /// # 返回值
    /// - `MeloResult<PlayerSnapshot>`：更新后的最新快照
    pub async fn set_volume_percent(&self, volume_percent: u8) -> MeloResult<PlayerSnapshot> {
        let mut session = self.session.lock().await;
        let clamped = volume_percent.min(100);
        if session.volume_percent == clamped && !session.muted {
            return Ok(Self::snapshot_from_session(&session, self.backend_name));
        }

        session.volume_percent = clamped;
        session.muted = false;
        self.backend.set_volume(Self::volume_factor(&session))?;
        self.publish_locked(&mut session)
    }

    /// 将播放器切换到静音状态。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - `MeloResult<PlayerSnapshot>`：更新后的最新快照
    pub async fn mute(&self) -> MeloResult<PlayerSnapshot> {
        let mut session = self.session.lock().await;
        if session.muted {
            return Ok(Self::snapshot_from_session(&session, self.backend_name));
        }

        session.muted = true;
        self.backend.set_volume(0.0)?;
        self.publish_locked(&mut session)
    }

    /// 取消播放器静音。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - `MeloResult<PlayerSnapshot>`：更新后的最新快照
    pub async fn unmute(&self) -> MeloResult<PlayerSnapshot> {
        let mut session = self.session.lock().await;
        if !session.muted {
            return Ok(Self::snapshot_from_session(&session, self.backend_name));
        }

        session.muted = false;
        self.backend.set_volume(Self::volume_factor(&session))?;
        self.publish_locked(&mut session)
    }

    /// 设置循环播放模式。
    ///
    /// # 参数
    /// - `repeat_mode`：目标循环模式
    ///
    /// # 返回值
    /// - `MeloResult<PlayerSnapshot>`：更新后的最新快照
    pub async fn set_repeat_mode(&self, repeat_mode: RepeatMode) -> MeloResult<PlayerSnapshot> {
        let mut session = self.session.lock().await;
        if session.repeat_mode == repeat_mode {
            return Ok(Self::snapshot_from_session(&session, self.backend_name));
        }

        session.repeat_mode = repeat_mode;
        self.publish_locked(&mut session)
    }

    /// 设置是否启用随机播放。
    ///
    /// # 参数
    /// - `enabled`：是否启用随机播放
    ///
    /// # 返回值
    /// - `MeloResult<PlayerSnapshot>`：更新后的最新快照
    pub async fn set_shuffle_enabled(&self, enabled: bool) -> MeloResult<PlayerSnapshot> {
        let mut session = self.session.lock().await;
        if session.shuffle_enabled == enabled {
            return Ok(Self::snapshot_from_session(&session, self.backend_name));
        }

        session.shuffle_enabled = enabled;
        if enabled {
            session.shuffle_seed = session.shuffle_seed.wrapping_add(1);
        }
        self.publish_locked(&mut session)
    }

    /// 读取一次后端播放进度，并在有意义变化时发布新快照。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - `MeloResult<Option<PlayerSnapshot>>`：发生有效进度变化时返回新快照
    pub async fn refresh_progress_once(&self) -> MeloResult<Option<PlayerSnapshot>> {
        let mut session = self.session.lock().await;
        if session.playback_state != PlaybackState::Playing {
            return Ok(None);
        }

        let Some(position) = self.backend.current_position() else {
            return Ok(None);
        };
        let position_seconds = position.as_secs_f64();
        let changed = session
            .position_seconds
            .map(|previous| (previous - position_seconds).abs() >= 0.25)
            .unwrap_or(true);
        if !changed {
            return Ok(None);
        }

        session.position_seconds = Some(position_seconds);
        let snapshot = self.publish_locked(&mut session)?;
        Ok(Some(snapshot))
    }

    /// 处理后端推送的运行时事件。
    ///
    /// # 参数
    /// - `event`：后端上报的运行时事件
    ///
    /// # 返回值
    /// - 无
    async fn handle_runtime_event(&self, event: PlaybackRuntimeEvent) {
        match event {
            PlaybackRuntimeEvent::TrackEnded { generation } => {
                let should_advance = {
                    let mut session = self.session.lock().await;
                    if session.playback_state != PlaybackState::Playing {
                        return;
                    }
                    if generation != session.playback_generation {
                        return;
                    }

                    match Self::navigation(&session)
                        .track_end_index(session.repeat_mode, session.shuffle_enabled)
                    {
                        Some(next_index) => {
                            let _ = session.queue.play_index(next_index);
                            true
                        }
                        None => {
                            session.playback_state = PlaybackState::Stopped;
                            session.last_error = None;
                            session.position_seconds = session.queue.current().map(|_| 0.0);
                            let _ = self.publish_locked(&mut session);
                            false
                        }
                    }
                };

                if should_advance {
                    let _ = self.play().await;
                }
            }
        }
    }

    /// 根据内部会话生成对外快照。
    ///
    /// # 参数
    /// - `session`：播放器会话
    ///
    /// # 返回值
    /// - `PlayerSnapshot`：对外快照
    fn snapshot_from_session(session: &PlayerSession, backend_name: &str) -> PlayerSnapshot {
        let current_song = session.queue.current().map(|item| NowPlayingSong {
            song_id: item.song_id,
            title: item.title.clone(),
            duration_seconds: item.duration_seconds,
        });
        let navigation = Self::navigation(session);
        PlayerSnapshot {
            backend_name: backend_name.to_string(),
            playback_state: session.playback_state.as_str().to_string(),
            current_song: current_song.clone(),
            queue_len: session.queue.len(),
            queue_index: session.queue.current_index(),
            has_next: navigation
                .next_index(session.repeat_mode, session.shuffle_enabled)
                .is_some(),
            has_prev: navigation
                .prev_index(session.repeat_mode, session.shuffle_enabled)
                .is_some(),
            last_error: session.last_error.clone(),
            version: session.version,
            position_seconds: session.position_seconds,
            position_fraction: match (
                session.position_seconds,
                current_song.as_ref().and_then(|item| item.duration_seconds),
            ) {
                (Some(position), Some(duration)) if duration > 0.0 => {
                    Some((position / duration).min(1.0))
                }
                _ => None,
            },
            volume_percent: session.volume_percent,
            muted: session.muted,
            repeat_mode: session.repeat_mode.as_str().to_string(),
            shuffle_enabled: session.shuffle_enabled,
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
        let snapshot = Self::snapshot_from_session(session, self.backend_name);
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

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::sync::broadcast;

use crate::core::model::player::{PlaybackState, QueueItem, RepeatMode};
use crate::domain::player::backend::{
    PlaybackBackend, PlaybackCommand, PlaybackSessionHandle, PlaybackStartRequest,
};
use crate::domain::player::runtime::{PlaybackRuntimeEvent, PlaybackStopReason};
use crate::domain::player::service::PlayerService;
use crate::domain::player::session_store::PersistedPlayerSession;

#[derive(Clone)]
struct FakeRuntimeHandle {
    session_channels: Arc<Mutex<HashMap<u64, broadcast::Sender<PlaybackRuntimeEvent>>>>,
}

impl FakeRuntimeHandle {
    /// 向指定播放代次注入一次停止事件。
    ///
    /// # 参数
    /// - `generation`：触发事件的播放代次
    /// - `reason`：停止原因
    ///
    /// # 返回值
    /// - 无
    fn send_stop(&self, generation: u64, reason: PlaybackStopReason) {
        let sender = self
            .session_channels
            .lock()
            .unwrap()
            .get(&generation)
            .cloned();
        let Some(sender) = sender else {
            return;
        };
        let _ = sender.send(PlaybackRuntimeEvent::PlaybackStopped { generation, reason });
    }

    /// 向服务层注入一次“当前曲目自然结束”的运行时事件。
    ///
    /// # 参数
    /// - `generation`：触发事件的播放代次
    ///
    /// # 返回值
    /// - 无
    fn track_ended(&self, generation: u64) {
        self.send_stop(generation, PlaybackStopReason::NaturalEof);
    }
}

struct FakeBackend {
    commands: Arc<Mutex<Vec<PlaybackCommand>>>,
    fail_next: Arc<Mutex<bool>>,
    current_position: Arc<Mutex<Option<Duration>>>,
    stopped_generations: Arc<Mutex<Vec<u64>>>,
    session_channels: Arc<Mutex<HashMap<u64, broadcast::Sender<PlaybackRuntimeEvent>>>>,
}

struct FakeSessionHandle {
    generation: u64,
    commands: Arc<Mutex<Vec<PlaybackCommand>>>,
    stopped_generations: Arc<Mutex<Vec<u64>>>,
    current_position: Arc<Mutex<Option<Duration>>>,
    runtime_tx: broadcast::Sender<PlaybackRuntimeEvent>,
}

impl Default for FakeBackend {
    /// 构造带运行时事件通道的测试后端。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - `Self`：测试后端
    fn default() -> Self {
        Self {
            commands: Arc::new(Mutex::new(Vec::new())),
            fail_next: Arc::new(Mutex::new(false)),
            current_position: Arc::new(Mutex::new(None)),
            stopped_generations: Arc::new(Mutex::new(Vec::new())),
            session_channels: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl FakeBackend {
    /// 返回用于主动推送运行时事件的句柄。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - `FakeRuntimeHandle`：运行时事件句柄
    fn runtime_handle(&self) -> FakeRuntimeHandle {
        FakeRuntimeHandle {
            session_channels: Arc::clone(&self.session_channels),
        }
    }

    /// 设置假后端当前汇报的播放位置。
    ///
    /// # 参数
    /// - `seconds`：当前位置秒数
    ///
    /// # 返回值
    /// - 无
    fn set_position(&self, seconds: f64) {
        *self.current_position.lock().unwrap() = Some(Duration::from_secs_f64(seconds));
    }

    /// 返回被停止过的播放代次列表。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - `Vec<u64>`：已收到 stop 命令的代次
    fn stopped_generations(&self) -> Vec<u64> {
        self.stopped_generations.lock().unwrap().clone()
    }
}

impl PlaybackSessionHandle for FakeSessionHandle {
    fn pause(&self) -> crate::core::error::MeloResult<()> {
        self.commands.lock().unwrap().push(PlaybackCommand::Pause);
        Ok(())
    }

    fn resume(&self) -> crate::core::error::MeloResult<()> {
        self.commands.lock().unwrap().push(PlaybackCommand::Resume);
        Ok(())
    }

    fn stop(&self) -> crate::core::error::MeloResult<()> {
        self.commands.lock().unwrap().push(PlaybackCommand::Stop);
        self.stopped_generations
            .lock()
            .unwrap()
            .push(self.generation);
        Ok(())
    }

    fn subscribe_runtime_events(&self) -> broadcast::Receiver<PlaybackRuntimeEvent> {
        self.runtime_tx.subscribe()
    }

    fn current_position(&self) -> Option<Duration> {
        *self.current_position.lock().unwrap()
    }

    fn set_volume(&self, factor: f32) -> crate::core::error::MeloResult<()> {
        self.commands
            .lock()
            .unwrap()
            .push(PlaybackCommand::SetVolume { factor });
        Ok(())
    }
}

impl PlaybackBackend for FakeBackend {
    fn backend_name(&self) -> &'static str {
        "fake"
    }

    fn start_session(
        &self,
        request: PlaybackStartRequest,
    ) -> crate::core::error::MeloResult<Box<dyn PlaybackSessionHandle>> {
        let mut fail_next = self.fail_next.lock().unwrap();
        if *fail_next {
            *fail_next = false;
            return Err(crate::core::error::MeloError::Message(
                "backend failed".to_string(),
            ));
        }

        self.commands.lock().unwrap().push(PlaybackCommand::Load {
            path: request.path.clone(),
            generation: request.generation,
        });

        let (runtime_tx, _) = broadcast::channel(16);
        self.session_channels
            .lock()
            .unwrap()
            .insert(request.generation, runtime_tx.clone());

        Ok(Box::new(FakeSessionHandle {
            generation: request.generation,
            commands: Arc::clone(&self.commands),
            stopped_generations: Arc::clone(&self.stopped_generations),
            current_position: Arc::clone(&self.current_position),
            runtime_tx,
        }))
    }
}

fn item(song_id: i64, title: &str) -> QueueItem {
    QueueItem {
        song_id,
        path: "tests/fixtures/full_test.mp3".to_string(),
        title: title.to_string(),
        duration_seconds: Some(180.0),
    }
}

#[tokio::test]
async fn play_on_empty_queue_records_queue_empty_error() {
    let service = PlayerService::new(Arc::new(FakeBackend::default()));

    let err = service.play().await.unwrap_err();
    assert!(err.to_string().contains("queue is empty"));

    let snapshot = service.snapshot().await;
    assert_eq!(snapshot.playback_state, PlaybackState::Error.as_str());
    assert_eq!(snapshot.last_error.unwrap().code, "queue_empty");
}

#[tokio::test]
async fn replace_queue_sets_selected_index_and_preserves_repeat_and_shuffle() {
    let backend = Arc::new(FakeBackend::default());
    let service = PlayerService::new(backend);

    service.set_repeat_mode(RepeatMode::All).await.unwrap();
    service.set_shuffle_enabled(true).await.unwrap();
    service
        .replace_queue(vec![item(1, "One"), item(2, "Two"), item(3, "Three")], 1)
        .await
        .unwrap();

    let snapshot = service.snapshot().await;
    assert_eq!(snapshot.queue_len, 3);
    assert_eq!(snapshot.queue_index, Some(1));
    assert_eq!(snapshot.current_song.unwrap().title, "Two");
    assert_eq!(snapshot.repeat_mode, "all");
    assert!(snapshot.shuffle_enabled);
}

#[tokio::test]
async fn toggle_from_paused_resumes_current_track() {
    let backend = Arc::new(FakeBackend::default());
    let service = PlayerService::new(backend.clone());

    service.append(item(1, "One")).await.unwrap();
    service.play().await.unwrap();
    service.pause().await.unwrap();
    service.toggle().await.unwrap();

    let snapshot = service.snapshot().await;
    assert_eq!(snapshot.playback_state, PlaybackState::Playing.as_str());
    assert_eq!(snapshot.queue_index, Some(0));
    assert_eq!(
        backend.commands.lock().unwrap().last(),
        Some(&PlaybackCommand::Resume)
    );
}

#[tokio::test]
async fn runtime_track_end_auto_advances_to_next_item() {
    let backend = Arc::new(FakeBackend::default());
    let runtime = backend.runtime_handle();
    let service = Arc::new(PlayerService::new(backend.clone()));
    service.start_runtime_event_loop();

    service.append(item(1, "One")).await.unwrap();
    service.append(item(2, "Two")).await.unwrap();
    service.play().await.unwrap();

    runtime.track_ended(1);
    tokio::task::yield_now().await;

    let snapshot = service.snapshot().await;
    assert_eq!(snapshot.playback_state, PlaybackState::Playing.as_str());
    assert_eq!(snapshot.queue_index, Some(1));
    assert_eq!(snapshot.current_song.unwrap().title, "Two");
}

#[tokio::test]
async fn stale_runtime_track_end_is_ignored_after_manual_next() {
    let backend = Arc::new(FakeBackend::default());
    let runtime = backend.runtime_handle();
    let service = Arc::new(PlayerService::new(backend));
    service.start_runtime_event_loop();

    service.append(item(1, "One")).await.unwrap();
    service.append(item(2, "Two")).await.unwrap();
    service.play().await.unwrap();
    service.next().await.unwrap();

    runtime.track_ended(1);
    tokio::task::yield_now().await;

    let snapshot = service.snapshot().await;
    assert_eq!(snapshot.queue_index, Some(1));
    assert_eq!(snapshot.current_song.unwrap().title, "Two");
}

#[tokio::test]
async fn queue_tail_track_end_sets_stopped_without_error() {
    let backend = Arc::new(FakeBackend::default());
    let runtime = backend.runtime_handle();
    let service = Arc::new(PlayerService::new(backend));
    service.start_runtime_event_loop();

    service.append(item(1, "One")).await.unwrap();
    service.play().await.unwrap();

    runtime.track_ended(1);
    tokio::task::yield_now().await;

    let snapshot = service.snapshot().await;
    assert_eq!(snapshot.playback_state, PlaybackState::Stopped.as_str());
    assert_eq!(snapshot.queue_index, Some(0));
    assert!(snapshot.last_error.is_none());
}

#[tokio::test]
async fn runtime_user_closed_backend_stops_without_advancing() {
    let backend = Arc::new(FakeBackend::default());
    let runtime = backend.runtime_handle();
    let service = Arc::new(PlayerService::new(backend));
    service.start_runtime_event_loop();

    service.append(item(1, "One")).await.unwrap();
    service.append(item(2, "Two")).await.unwrap();
    service.play().await.unwrap();

    runtime.send_stop(1, PlaybackStopReason::UserClosedBackend);
    tokio::task::yield_now().await;

    let snapshot = service.snapshot().await;
    assert_eq!(snapshot.playback_state, PlaybackState::Stopped.as_str());
    assert_eq!(snapshot.queue_index, Some(0));
    assert_eq!(snapshot.current_song.unwrap().title, "One");
}

#[tokio::test]
async fn runtime_backend_aborted_sets_error_without_auto_next() {
    let backend = Arc::new(FakeBackend::default());
    let runtime = backend.runtime_handle();
    let service = Arc::new(PlayerService::new(backend));
    service.start_runtime_event_loop();

    service.append(item(1, "One")).await.unwrap();
    service.append(item(2, "Two")).await.unwrap();
    service.play().await.unwrap();

    runtime.send_stop(1, PlaybackStopReason::BackendAborted);
    tokio::task::yield_now().await;

    let snapshot = service.snapshot().await;
    assert_eq!(snapshot.playback_state, PlaybackState::Error.as_str());
    assert_eq!(snapshot.last_error.unwrap().code, "backend_aborted");
}

#[tokio::test(start_paused = true)]
async fn progress_tick_updates_snapshot_position_and_fraction() {
    let backend = Arc::new(FakeBackend::default());
    let service = Arc::new(PlayerService::new(backend.clone()));
    service.start_runtime_event_loop();
    service.start_progress_loop();

    service.append(item(1, "One")).await.unwrap();
    service.play().await.unwrap();
    backend.set_position(42.0);

    tokio::time::advance(Duration::from_millis(600)).await;

    let snapshot = service.snapshot().await;
    assert_eq!(snapshot.position_seconds, Some(42.0));
    assert!(snapshot.position_fraction.unwrap() > 0.23);
}

#[tokio::test(start_paused = true)]
async fn unchanged_progress_tick_does_not_bump_version() {
    let backend = Arc::new(FakeBackend::default());
    let service = Arc::new(PlayerService::new(backend.clone()));
    service.start_runtime_event_loop();
    service.start_progress_loop();

    service.append(item(1, "One")).await.unwrap();
    service.play().await.unwrap();
    backend.set_position(5.0);
    tokio::time::advance(Duration::from_millis(600)).await;
    let first = service.snapshot().await;

    backend.set_position(5.0);
    tokio::time::advance(Duration::from_millis(600)).await;
    let second = service.snapshot().await;

    assert_eq!(first.position_seconds, Some(5.0));
    assert_eq!(second.position_seconds, Some(5.0));
    assert_eq!(first.version, second.version);
}

#[tokio::test]
async fn stop_resets_progress_to_zero_for_current_song() {
    let backend = Arc::new(FakeBackend::default());
    let service = Arc::new(PlayerService::new(backend.clone()));

    service.append(item(1, "One")).await.unwrap();
    service.play().await.unwrap();
    backend.set_position(17.0);
    service.refresh_progress_once().await.unwrap();

    let snapshot = service.stop().await.unwrap();
    assert_eq!(snapshot.playback_state, PlaybackState::Stopped.as_str());
    assert_eq!(snapshot.position_seconds, Some(0.0));
    assert_eq!(snapshot.position_fraction, Some(0.0));
}

#[tokio::test]
async fn restore_persisted_playing_session_downgrades_to_stopped() {
    let service = PlayerService::new(Arc::new(FakeBackend::default()));
    let snapshot = service
        .restore_persisted_session(PersistedPlayerSession {
            playback_state: PlaybackState::Playing,
            queue_index: Some(0),
            position_seconds: Some(48.0),
            queue: vec![item(1, "One")],
        })
        .await
        .unwrap();

    assert_eq!(snapshot.playback_state, PlaybackState::Stopped.as_str());
    assert_eq!(snapshot.queue_index, Some(0));
    assert_eq!(snapshot.position_seconds, Some(48.0));
}

#[tokio::test]
async fn set_volume_updates_snapshot_and_backend_once() {
    let backend = Arc::new(FakeBackend::default());
    let service = PlayerService::new(backend.clone());
    service.append(item(1, "One")).await.unwrap();
    service.play().await.unwrap();

    let snapshot = service.set_volume_percent(70).await.unwrap();
    let second = service.set_volume_percent(70).await.unwrap();

    assert_eq!(snapshot.volume_percent, 70);
    assert!(!snapshot.muted);
    assert_eq!(second.version, snapshot.version);
    assert_eq!(
        backend
            .commands
            .lock()
            .unwrap()
            .iter()
            .filter(|cmd| matches!(cmd, PlaybackCommand::SetVolume { .. }))
            .count(),
        1
    );
}

#[tokio::test]
async fn repeat_all_wraps_queue_tail_on_manual_next() {
    let backend = Arc::new(FakeBackend::default());
    let service = PlayerService::new(backend);
    service.append(item(1, "One")).await.unwrap();
    service.append(item(2, "Two")).await.unwrap();
    service.set_repeat_mode(RepeatMode::All).await.unwrap();
    service.play_index(1).await.unwrap();

    let snapshot = service.next().await.unwrap();

    assert_eq!(snapshot.queue_index, Some(0));
    assert_eq!(snapshot.current_song.unwrap().title, "One");
}

#[tokio::test]
async fn mute_preserves_last_non_zero_volume() {
    let backend = Arc::new(FakeBackend::default());
    let service = PlayerService::new(backend);
    service.set_volume_percent(35).await.unwrap();

    let muted = service.mute().await.unwrap();
    let unmuted = service.unmute().await.unwrap();

    assert!(muted.muted);
    assert_eq!(unmuted.volume_percent, 35);
    assert!(!unmuted.muted);
}

#[tokio::test]
async fn next_loads_following_queue_item() {
    let backend = Arc::new(FakeBackend::default());
    let service = PlayerService::new(backend);

    service.append(item(1, "One")).await.unwrap();
    service.append(item(2, "Two")).await.unwrap();
    service.play().await.unwrap();
    service.next().await.unwrap();

    let snapshot = service.snapshot().await;
    assert_eq!(snapshot.queue_index, Some(1));
    assert_eq!(snapshot.current_song.unwrap().title, "Two");
}

#[tokio::test]
async fn backend_failure_sets_error_without_entering_playing() {
    let backend = Arc::new(FakeBackend::default());
    let service = PlayerService::new(backend.clone());
    service.append(item(1, "Broken")).await.unwrap();
    *backend.fail_next.lock().unwrap() = true;

    let err = service.play().await.unwrap_err();
    assert!(err.to_string().contains("backend failed"));

    let snapshot = service.snapshot().await;
    assert_eq!(snapshot.playback_state, PlaybackState::Error.as_str());
    assert_eq!(snapshot.last_error.unwrap().code, "backend_unavailable");
}

#[tokio::test]
async fn repeated_pause_does_not_bump_version_or_backend_commands() {
    let backend = Arc::new(FakeBackend::default());
    let service = PlayerService::new(backend.clone());
    service.append(item(1, "One")).await.unwrap();
    service.play().await.unwrap();

    let paused = service.pause().await.unwrap();
    let paused_again = service.pause().await.unwrap();

    assert_eq!(paused.playback_state, "paused");
    assert_eq!(paused_again.playback_state, "paused");
    assert_eq!(paused_again.version, paused.version);
    assert_eq!(
        backend
            .commands
            .lock()
            .unwrap()
            .iter()
            .filter(|cmd| matches!(cmd, PlaybackCommand::Pause))
            .count(),
        1
    );
}

#[tokio::test]
async fn repeated_stop_does_not_bump_version_or_backend_commands() {
    let backend = Arc::new(FakeBackend::default());
    let service = PlayerService::new(backend.clone());
    service.append(item(1, "One")).await.unwrap();
    service.play().await.unwrap();

    let stopped = service.stop().await.unwrap();
    let stopped_again = service.stop().await.unwrap();

    assert_eq!(stopped.playback_state, "stopped");
    assert_eq!(stopped_again.playback_state, "stopped");
    assert_eq!(stopped_again.version, stopped.version);
    assert_eq!(
        backend
            .commands
            .lock()
            .unwrap()
            .iter()
            .filter(|cmd| matches!(cmd, PlaybackCommand::Stop))
            .count(),
        1
    );
}

#[tokio::test]
async fn snapshot_includes_backend_name() {
    let backend = std::sync::Arc::new(crate::domain::player::backend::NoopBackend);
    let service = crate::domain::player::service::PlayerService::new(backend);

    let snapshot = service.snapshot().await;
    assert_eq!(snapshot.backend_name, "noop");
}

#[tokio::test]
async fn replay_stops_previous_session_before_creating_new_one() {
    let backend = Arc::new(FakeBackend::default());
    let service = Arc::new(PlayerService::new(backend.clone()));

    service.append(item(1, "One")).await.unwrap();
    service.append(item(2, "Two")).await.unwrap();
    service.play().await.unwrap();
    service.next().await.unwrap();

    assert_eq!(backend.stopped_generations(), vec![1]);
}

#[tokio::test]
async fn snapshot_exposes_backend_notice() {
    let backend = Arc::new(FakeBackend::default());
    let service = PlayerService::new_with_notice(
        backend,
        Some("mpv_lib unavailable, fell back to mpv_ipc".to_string()),
    );

    let snapshot = service.snapshot().await;
    assert_eq!(
        snapshot.backend_notice.as_deref(),
        Some("mpv_lib unavailable, fell back to mpv_ipc")
    );
}

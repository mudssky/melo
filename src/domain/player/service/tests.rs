use std::sync::{Arc, Mutex};

use crate::core::model::player::{PlaybackState, QueueItem};
use crate::domain::player::backend::{PlaybackBackend, PlaybackCommand};
use crate::domain::player::service::PlayerService;

#[derive(Default)]
struct FakeBackend {
    commands: Arc<Mutex<Vec<PlaybackCommand>>>,
    fail_next: Arc<Mutex<bool>>,
}

impl PlaybackBackend for FakeBackend {
    fn load_and_play(&self, path: &std::path::Path) -> crate::core::error::MeloResult<()> {
        let mut fail_next = self.fail_next.lock().unwrap();
        if *fail_next {
            *fail_next = false;
            return Err(crate::core::error::MeloError::Message(
                "backend failed".to_string(),
            ));
        }

        self.commands
            .lock()
            .unwrap()
            .push(PlaybackCommand::Load(path.to_path_buf()));
        Ok(())
    }

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
        Ok(())
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

use std::sync::{Arc, Mutex};

use melo::domain::player::backend::{
    PlaybackBackend, PlaybackCommand, PlaybackSessionHandle, PlaybackStartRequest,
};
use melo::domain::player::runtime::PlaybackRuntimeEvent;
use melo::domain::player::service::PlayerService;
use tokio::sync::broadcast;

struct FakeBackend {
    commands: Arc<Mutex<Vec<PlaybackCommand>>>,
}

impl Default for FakeBackend {
    fn default() -> Self {
        Self {
            commands: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

struct FakeSessionHandle {
    commands: Arc<Mutex<Vec<PlaybackCommand>>>,
    runtime_tx: broadcast::Sender<PlaybackRuntimeEvent>,
}

impl PlaybackSessionHandle for FakeSessionHandle {
    fn pause(&self) -> melo::core::error::MeloResult<()> {
        self.commands.lock().unwrap().push(PlaybackCommand::Pause);
        Ok(())
    }

    fn resume(&self) -> melo::core::error::MeloResult<()> {
        self.commands.lock().unwrap().push(PlaybackCommand::Resume);
        Ok(())
    }

    fn stop(&self) -> melo::core::error::MeloResult<()> {
        self.commands.lock().unwrap().push(PlaybackCommand::Stop);
        Ok(())
    }

    fn subscribe_runtime_events(&self) -> broadcast::Receiver<PlaybackRuntimeEvent> {
        self.runtime_tx.subscribe()
    }

    fn current_position(&self) -> Option<std::time::Duration> {
        None
    }

    fn set_volume(&self, factor: f32) -> melo::core::error::MeloResult<()> {
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
    ) -> melo::core::error::MeloResult<Box<dyn PlaybackSessionHandle>> {
        self.commands.lock().unwrap().push(PlaybackCommand::Load {
            path: request.path,
            generation: request.generation,
        });
        let (runtime_tx, _) = broadcast::channel(16);
        Ok(Box::new(FakeSessionHandle {
            commands: Arc::clone(&self.commands),
            runtime_tx,
        }))
    }
}

#[test]
fn production_backend_type_is_rodio_backend() {
    fn assert_backend_type<T: PlaybackBackend>() {}

    assert_backend_type::<melo::domain::player::rodio_backend::RodioBackend>();
    let _constructor: fn() -> melo::core::error::MeloResult<
        melo::domain::player::rodio_backend::RodioBackend,
    > = melo::domain::player::rodio_backend::RodioBackend::new;
}

#[tokio::test]
async fn player_service_loads_first_queue_item_on_play() {
    let backend = Arc::new(FakeBackend::default());
    let service = PlayerService::new(backend.clone());

    service
        .enqueue(melo::core::model::player::QueueItem {
            song_id: 1,
            path: "tests/fixtures/full_test.mp3".into(),
            title: "Blue Bird".into(),
            duration_seconds: Some(212.0),
        })
        .await
        .unwrap();

    service.play().await.unwrap();

    let snapshot = service.snapshot().await;
    assert_eq!(snapshot.playback_state, "playing");
    assert_eq!(snapshot.queue_index, Some(0));
    assert_eq!(snapshot.current_song.unwrap().title, "Blue Bird");
    assert_eq!(
        backend
            .commands
            .lock()
            .unwrap()
            .iter()
            .find(|command| matches!(command, PlaybackCommand::Load { .. })),
        Some(&PlaybackCommand::Load {
            path: std::path::PathBuf::from("tests/fixtures/full_test.mp3"),
            generation: 1,
        })
    );
}

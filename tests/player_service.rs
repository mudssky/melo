use std::sync::{Arc, Mutex};

use melo::domain::player::backend::{PlaybackBackend, PlaybackCommand};
use melo::domain::player::service::PlayerService;

#[derive(Default)]
struct FakeBackend {
    commands: Arc<Mutex<Vec<PlaybackCommand>>>,
}

impl PlaybackBackend for FakeBackend {
    fn load_and_play(&self, path: &std::path::Path) -> melo::core::error::MeloResult<()> {
        self.commands
            .lock()
            .unwrap()
            .push(PlaybackCommand::Load(path.to_path_buf()));
        Ok(())
    }

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
}

#[tokio::test]
async fn player_service_loads_first_queue_item_on_play() {
    let backend = Arc::new(FakeBackend::default());
    let service = PlayerService::new(backend.clone());

    service
        .enqueue(melo::core::model::player::QueueItem {
            song_id: 1,
            path: "D:/Music/blue-bird.flac".into(),
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
    assert_eq!(backend.commands.lock().unwrap().len(), 1);
}

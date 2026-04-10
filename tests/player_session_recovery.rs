use std::sync::Arc;

use tempfile::tempdir;

use melo::core::config::settings::Settings;
use melo::core::db::bootstrap::DatabaseBootstrap;
use melo::core::db::connection::connect;
use melo::domain::player::backend::NoopBackend;
use melo::domain::player::session_store::{PersistedPlayerSession, PlayerSessionStore};

#[tokio::test]
async fn app_state_restores_last_session_from_store() {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("melo.db");
    let settings = Settings::for_test(db_path.clone());
    DatabaseBootstrap::new(&settings).init().await.unwrap();
    let db = connect(&settings).await.unwrap();
    let store = Arc::new(PlayerSessionStore::new(db));

    store
        .save(&PersistedPlayerSession {
            playback_state: melo::core::model::player::PlaybackState::Playing,
            queue_index: Some(0),
            position_seconds: Some(12.0),
            queue: vec![melo::core::model::player::QueueItem {
                song_id: 1,
                path: "tests/fixtures/full_test.mp3".into(),
                title: "Blue Bird".into(),
                duration_seconds: Some(212.0),
            }],
        })
        .await
        .unwrap();

    let state =
        melo::daemon::app::AppState::with_backend_and_session_store(Arc::new(NoopBackend), store)
            .await
            .unwrap();

    let snapshot = state.player.snapshot().await;
    assert_eq!(snapshot.playback_state, "stopped");
    assert_eq!(snapshot.queue_len, 1);
    assert_eq!(snapshot.position_seconds, Some(12.0));
}

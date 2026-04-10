use tempfile::tempdir;

use crate::core::config::settings::Settings;
use crate::core::db::bootstrap::DatabaseBootstrap;
use crate::core::db::connection::connect;
use crate::core::model::player::{PlaybackState, QueueItem};
use crate::domain::player::session_store::{PersistedPlayerSession, PlayerSessionStore};

fn item(song_id: i64, title: &str) -> QueueItem {
    QueueItem {
        song_id,
        path: format!("tests/fixtures/{title}.mp3"),
        title: title.to_string(),
        duration_seconds: Some(212.0),
    }
}

#[tokio::test]
async fn session_store_round_trips_queue_index_and_position() {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("melo.db");
    let settings = Settings::for_test(db_path.clone());
    DatabaseBootstrap::new(&settings).init().await.unwrap();
    let db = connect(&settings).await.unwrap();
    let store = PlayerSessionStore::new(db);

    let session = PersistedPlayerSession {
        playback_state: PlaybackState::Stopped,
        queue_index: Some(1),
        position_seconds: Some(48.5),
        queue: vec![item(1, "One"), item(2, "Two")],
    };

    store.save(&session).await.unwrap();
    let restored = store.load().await.unwrap().unwrap();

    assert_eq!(restored.playback_state, PlaybackState::Stopped);
    assert_eq!(restored.queue_index, Some(1));
    assert_eq!(restored.position_seconds, Some(48.5));
    assert_eq!(restored.queue.len(), 2);
    assert_eq!(restored.queue[1].title, "Two");
}

#[tokio::test]
async fn position_only_changes_under_one_second_do_not_force_write() {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("melo.db");
    let settings = Settings::for_test(db_path.clone());
    DatabaseBootstrap::new(&settings).init().await.unwrap();
    let db = connect(&settings).await.unwrap();
    let store = PlayerSessionStore::new(db);

    let before = PersistedPlayerSession {
        playback_state: PlaybackState::Playing,
        queue_index: Some(0),
        position_seconds: Some(10.0),
        queue: vec![item(1, "One")],
    };
    let after = PersistedPlayerSession {
        playback_state: PlaybackState::Playing,
        queue_index: Some(0),
        position_seconds: Some(10.4),
        queue: vec![item(1, "One")],
    };

    assert!(!store.should_persist(Some(&before), &after));
}

use std::time::Duration;

use crate::core::model::runtime_task::RuntimeTaskPhase;
use crate::daemon::tasks::RuntimeTaskStore;

#[tokio::test(start_paused = true)]
async fn runtime_task_store_tracks_active_scan_progress() {
    let store = RuntimeTaskStore::new();
    let mut receiver = store.subscribe();
    let handle = store.start_scan("D:/Music/Aimer".to_string(), 3);

    handle.mark_prewarming(Some("01-Blue Bird.flac".to_string()));
    receiver.changed().await.unwrap();
    let snapshot = receiver.borrow().clone().unwrap();

    assert_eq!(snapshot.phase, RuntimeTaskPhase::Prewarming);
    assert_eq!(snapshot.discovered_count, 3);
    assert_eq!(
        snapshot.current_item_name.as_deref(),
        Some("01-Blue Bird.flac")
    );
}

#[tokio::test(start_paused = true)]
async fn runtime_task_store_clears_completed_snapshot_after_delay() {
    let store = RuntimeTaskStore::new();
    let mut receiver = store.subscribe();
    let handle = store.start_scan("D:/Music/Aimer".to_string(), 2);

    handle.mark_completed(2);
    receiver.changed().await.unwrap();
    assert!(receiver.borrow().is_some());

    tokio::time::advance(Duration::from_secs(3)).await;
    receiver.changed().await.unwrap();
    assert!(receiver.borrow().is_none());
}

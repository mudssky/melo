use std::fs;

#[test]
fn daemon_runtime_meta_live_uses_current_process_started_at_text() {
    let runtime = super::DaemonRuntimeMeta::live("noop").unwrap();

    assert_eq!(runtime.started_at, super::current_process_started_at_text());
}

#[tokio::test]
async fn daemon_app_state_new_bootstraps_database_before_services_use_it() {
    let temp = tempfile::tempdir().unwrap();
    let config_path = temp.path().join("config.toml");
    fs::write(
        &config_path,
        r#"
[database]
path = "runtime/melo.db"
"#,
    )
    .unwrap();

    unsafe {
        std::env::set_var("MELO_CONFIG_PATH", &config_path);
    }
    let state = crate::daemon::app::AppState::new().await;
    unsafe {
        std::env::remove_var("MELO_CONFIG_PATH");
    }

    assert!(state.is_ok());
    assert!(temp.path().join("runtime").exists());
    assert!(temp.path().join("runtime/melo.db").exists());
}

#[tokio::test]
async fn app_state_tui_snapshot_includes_active_runtime_task() {
    let state = crate::daemon::app::AppState::for_test().await;
    let handle = state
        .runtime_tasks()
        .start_scan("D:/Music/Aimer".to_string(), 4);
    handle.mark_indexing(1, 1, Some("track-01.flac".to_string()));

    let snapshot = state.tui_snapshot().await.unwrap();

    assert_eq!(snapshot.player.backend_name, "noop");
    assert_eq!(
        snapshot.active_task.unwrap().current_item_name.as_deref(),
        Some("track-01.flac")
    );
}

#[tokio::test]
async fn tui_snapshot_includes_current_track_detail_when_queue_has_song() {
    let state = crate::daemon::app::AppState::for_test().await;
    state
        .player
        .append(crate::core::model::player::QueueItem {
            song_id: 7,
            path: "tests/fixtures/full_test.mp3".into(),
            title: "Blue Bird".into(),
            duration_seconds: Some(212.0),
        })
        .await
        .unwrap();
    state.set_current_playlist_context("Favorites", "static");
    state.player.play().await.unwrap();

    let snapshot = state.tui_snapshot().await.unwrap();
    assert_eq!(snapshot.current_track.song_id, Some(7));
    assert_eq!(snapshot.current_track.title.as_deref(), Some("Blue Bird"));
}

#[tokio::test]
async fn player_service_keeps_backend_notice_from_factory_resolution() {
    let backend = std::sync::Arc::new(crate::domain::player::backend::NoopBackend);
    let service = crate::domain::player::service::PlayerService::new_with_notice(
        backend,
        Some("mpv_lib unavailable, fell back to mpv_ipc".to_string()),
    );

    let snapshot = service.snapshot().await;
    assert_eq!(
        snapshot.backend_notice.as_deref(),
        Some("mpv_lib unavailable, fell back to mpv_ipc")
    );
}

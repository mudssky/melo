use std::fs;

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

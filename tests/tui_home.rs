use melo::core::config::settings::Settings;
use melo::core::db::bootstrap::DatabaseBootstrap;
use melo::daemon::app::AppState;

#[tokio::test(flavor = "multi_thread")]
async fn direct_open_updates_tui_home_default_selected_playlist() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::write(temp.path().join("01-first.flac"), b"audio").unwrap();

    let mut settings = Settings::for_test(temp.path().join("melo.db"));
    settings.open.max_depth = 1;
    settings.open.prewarm_limit = 1;
    DatabaseBootstrap::new(&settings).init().await.unwrap();

    let state = AppState::for_test_with_settings(settings.clone()).await;
    let response = state
        .open_target(melo::domain::open::service::OpenRequest {
            target: temp.path().to_string_lossy().to_string(),
            mode: "cwd_dir".to_string(),
        })
        .await
        .unwrap();

    let snapshot = state.tui_snapshot().await.unwrap();
    assert_eq!(response.playlist_name, temp.path().to_string_lossy());
    assert_eq!(
        snapshot
            .playlist_browser
            .default_selected_playlist
            .as_deref(),
        Some(temp.path().to_string_lossy().as_ref())
    );
    assert_eq!(
        snapshot.playlist_browser.default_view,
        melo::core::model::tui::TuiViewKind::Playlist
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn active_playback_session_keeps_current_playlist_as_default_selection() {
    let state = melo::daemon::app::AppState::for_test().await;
    state.set_current_playlist_context("Favorites", "static");
    state
        .player
        .append(melo::core::model::player::QueueItem {
            song_id: 1,
            path: "tests/fixtures/full_test.mp3".into(),
            title: "Blue Bird".into(),
            duration_seconds: Some(212.0),
        })
        .await
        .unwrap();
    state.player.play().await.unwrap();

    let snapshot = state.tui_snapshot().await.unwrap();
    assert_eq!(
        snapshot
            .playlist_browser
            .default_selected_playlist
            .as_deref(),
        Some("Favorites")
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn tui_home_snapshot_carries_lyrics_and_artwork_context_for_current_song() {
    let temp = tempfile::tempdir().unwrap();
    let audio = temp.path().join("01-first.flac");
    let cover = temp.path().join("cover.jpg");
    std::fs::write(&audio, b"audio").unwrap();
    std::fs::write(&cover, b"jpg").unwrap();
    std::fs::write(temp.path().join("01-first.lrc"), "[00:00.00]hello").unwrap();

    let mut settings = Settings::for_test(temp.path().join("melo.db"));
    settings.open.max_depth = 1;
    settings.open.prewarm_limit = 1;
    DatabaseBootstrap::new(&settings).init().await.unwrap();

    let state = AppState::for_test_with_settings(settings.clone()).await;
    state
        .open_target(melo::domain::open::service::OpenRequest {
            target: temp.path().to_string_lossy().to_string(),
            mode: "cwd_dir".to_string(),
        })
        .await
        .unwrap();

    let snapshot = state.tui_snapshot().await.unwrap();
    assert!(
        snapshot
            .current_track
            .lyrics
            .as_deref()
            .unwrap()
            .contains("hello")
    );
    assert!(
        snapshot
            .current_track
            .artwork
            .as_ref()
            .unwrap()
            .source_path
            .is_some()
    );
}

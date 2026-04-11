use assert_cmd::Command;
use predicates::prelude::*;
use tokio::net::TcpListener;

#[tokio::test(flavor = "multi_thread")]
async fn status_command_prints_json_snapshot() {
    let app = melo::daemon::app::test_router().await;
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let mut cmd = Command::cargo_bin("melo").unwrap();
    cmd.env("MELO_BASE_URL", format!("http://{addr}"));
    cmd.arg("status");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("playback_state"));
}

#[tokio::test(flavor = "multi_thread")]
async fn status_command_prints_progress_fields() {
    let state = melo::daemon::app::AppState::for_test().await;
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
    let app = melo::daemon::server::router(state);
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let mut cmd = Command::cargo_bin("melo").unwrap();
    cmd.env("MELO_BASE_URL", format!("http://{addr}"));
    cmd.arg("status");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("position_seconds"))
        .stdout(predicate::str::contains("position_fraction"));
}

#[tokio::test(flavor = "multi_thread")]
async fn tui_client_receives_initial_player_snapshot() {
    let state = melo::daemon::app::AppState::for_test().await;
    state
        .player
        .append(melo::core::model::player::QueueItem {
            song_id: 7,
            path: "tests/fixtures/full_test.mp3".into(),
            title: "Blue Bird".into(),
            duration_seconds: Some(212.0),
        })
        .await
        .unwrap();
    state.player.play().await.unwrap();
    let app = melo::daemon::server::router(state);
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let client = melo::tui::client::TuiClient::new(format!("http://{addr}"));
    let snapshot = client.next_snapshot().await.unwrap();

    assert_eq!(snapshot.playback_state, "playing");
    assert_eq!(snapshot.current_song.unwrap().title, "Blue Bird");
}

#[tokio::test(flavor = "multi_thread")]
async fn queue_show_prints_snapshot_navigation_flags() {
    let state = melo::daemon::app::AppState::for_test().await;
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
    let app = melo::daemon::server::router(state);
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let mut cmd = Command::cargo_bin("melo").unwrap();
    cmd.env("MELO_BASE_URL", format!("http://{addr}"));
    cmd.arg("queue").arg("show");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("has_next"))
        .stdout(predicate::str::contains("queue_len"));
}

#[tokio::test(flavor = "multi_thread")]
async fn player_mode_show_prints_repeat_and_shuffle_fields() {
    let app = melo::daemon::app::test_router().await;
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let mut cmd = Command::cargo_bin("melo").unwrap();
    cmd.env("MELO_BASE_URL", format!("http://{addr}"));
    cmd.arg("player").arg("mode").arg("show");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("repeat_mode"))
        .stdout(predicate::str::contains("shuffle_enabled"));
}

#[test]
fn db_path_command_prints_sqlite_location() {
    let mut cmd = Command::cargo_bin("melo").unwrap();
    cmd.arg("db").arg("path");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains(".db"));
}

#[tokio::test(flavor = "multi_thread")]
async fn status_command_uses_registered_daemon_url() {
    let state = melo::daemon::app::AppState::for_test().await;
    state
        .player
        .append(melo::core::model::player::QueueItem {
            song_id: 99,
            path: "tests/fixtures/full_test.mp3".into(),
            title: "Registry Only Song".into(),
            duration_seconds: Some(212.0),
        })
        .await
        .unwrap();
    state.player.play().await.unwrap();
    let app = melo::daemon::server::router(state);
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let temp = tempfile::tempdir().unwrap();
    let state_file = temp.path().join("daemon.json");

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    std::fs::write(
        &state_file,
        serde_json::json!({
            "base_url": format!("http://{addr}"),
            "pid": std::process::id(),
            "started_at": "2026-04-11T13:30:00Z",
            "version": env!("CARGO_PKG_VERSION"),
            "backend": "rodio",
            "host": "127.0.0.1",
            "port": addr.port()
        })
        .to_string(),
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("melo").unwrap();
    cmd.env("MELO_DAEMON_STATE_FILE", &state_file);
    cmd.arg("status");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Registry Only Song"));
}

#[tokio::test(flavor = "multi_thread")]
async fn explicit_open_command_prints_stable_error_body() {
    let app = melo::daemon::app::test_router().await;
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let mut cmd = Command::cargo_bin("melo").unwrap();
    cmd.env("MELO_BASE_URL", format!("http://{addr}"));
    cmd.arg("cover.jpg");

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("unsupported_open_format"));
}

#[tokio::test(flavor = "multi_thread")]
async fn daemon_stop_command_shuts_down_registered_server() {
    let state = melo::daemon::app::AppState::for_test().await;
    let app = melo::daemon::server::router(state.clone());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let temp = tempfile::tempdir().unwrap();
    let state_file = temp.path().join("daemon.json");

    std::fs::write(
        &state_file,
        serde_json::json!({
            "base_url": format!("http://{addr}"),
            "pid": std::process::id(),
            "started_at": "2026-04-11T18:00:00Z",
            "version": env!("CARGO_PKG_VERSION"),
            "backend": "noop",
            "host": "127.0.0.1",
            "port": addr.port()
        })
        .to_string(),
    )
    .unwrap();

    let shutdown_state = state.clone();
    let server = tokio::spawn(async move {
        axum::serve(listener, app)
            .with_graceful_shutdown(async move {
                shutdown_state.wait_for_shutdown().await;
            })
            .await
            .unwrap();
    });

    let mut cmd = Command::cargo_bin("melo").unwrap();
    cmd.env("MELO_DAEMON_STATE_FILE", &state_file);
    cmd.arg("daemon").arg("stop");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("stopped"));

    server.await.unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn daemon_status_command_prints_registered_metadata() {
    let temp = tempfile::tempdir().unwrap();
    let state_file = temp.path().join("daemon.json");
    std::fs::write(
        &state_file,
        serde_json::json!({
            "base_url": "http://127.0.0.1:38123",
            "pid": 4242,
            "started_at": "2026-04-11T18:30:00Z",
            "version": env!("CARGO_PKG_VERSION"),
            "backend": "mpv",
            "host": "127.0.0.1",
            "port": 38123
        })
        .to_string(),
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("melo").unwrap();
    cmd.env("MELO_DAEMON_STATE_FILE", &state_file);
    cmd.arg("daemon").arg("status");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("\"backend\": \"mpv\""))
        .stdout(predicate::str::contains("\"port\": 38123"));
}

#[tokio::test(flavor = "multi_thread")]
async fn daemon_status_command_reports_missing_registration() {
    let temp = tempfile::tempdir().unwrap();
    let state_file = temp.path().join("daemon.json");

    let mut cmd = Command::cargo_bin("melo").unwrap();
    cmd.env("MELO_DAEMON_STATE_FILE", &state_file);
    cmd.arg("daemon").arg("status");

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("daemon_not_running"));
}

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

#[test]
fn db_path_command_prints_sqlite_location() {
    let mut cmd = Command::cargo_bin("melo").unwrap();
    cmd.arg("db").arg("path");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains(".db"));
}

use assert_cmd::Command;
use predicates::prelude::*;
use std::process::Stdio;
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
async fn status_command_prints_backend_notice_field_when_present() {
    let backend = std::sync::Arc::new(melo::domain::player::backend::NoopBackend);
    let service = melo::domain::player::service::PlayerService::new_with_notice(
        backend,
        Some("mpv_lib unavailable, fell back to mpv_ipc".to_string()),
    );
    let snapshot = service.snapshot().await;

    let output = serde_json::to_string(&snapshot).unwrap();
    assert!(output.contains("backend_notice"));
    assert!(output.contains("fell back to mpv_ipc"));
}

#[tokio::test(flavor = "multi_thread")]
async fn tui_client_receives_initial_tui_snapshot() {
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
    let handle = state
        .runtime_tasks()
        .start_scan("D:/Music/Aimer".to_string(), 3);
    handle.mark_indexing(1, 1, Some("Blue Bird.flac".to_string()));
    let app = melo::daemon::server::router(state);
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let client = melo::tui::client::TuiClient::new(format!("http://{addr}"));
    let snapshot = client.next_snapshot().await.unwrap();

    assert_eq!(snapshot.player.playback_state, "playing");
    assert_eq!(snapshot.player.current_song.unwrap().title, "Blue Bird");
    assert_eq!(snapshot.active_task.unwrap().indexed_count, 1);
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

#[test]
fn daemon_run_reports_database_prepare_failure_when_path_is_invalid() {
    let temp = tempfile::tempdir().unwrap();
    let state_file = temp.path().join("daemon.json");
    let config_path = temp.path().join("config.toml");
    std::fs::write(
        &config_path,
        r#"
[database]
path = "bad<>/melo.db"

[daemon]
host = "127.0.0.1"
base_port = 65529
port_search_limit = 0
"#,
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("melo").unwrap();
    cmd.env("MELO_CONFIG_PATH", &config_path);
    cmd.env("MELO_DAEMON_STATE_FILE", &state_file);
    cmd.arg("daemon").arg("run");

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("failed to prepare database"));
}

#[test]
fn daemon_run_writes_json_logs_to_daemon_file() {
    let temp = tempfile::tempdir().unwrap();
    let state_file = temp.path().join("daemon.json");
    let config_path = temp.path().join("config.toml");
    std::fs::write(
        &config_path,
        r#"
[database]
path = "bad<>/melo.db"

[logging]
level = "info"
file_format = "json"

[logging.daemon]
file_enabled = true
file_path = "daemon.log"
"#,
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("melo").unwrap();
    cmd.env("MELO_CONFIG_PATH", &config_path);
    cmd.env("MELO_DAEMON_STATE_FILE", &state_file);
    cmd.arg("daemon").arg("run");
    cmd.assert().failure();

    let contents = std::fs::read_to_string(temp.path().join("daemon.log")).unwrap();
    let line = contents.lines().last().unwrap();
    let json: serde_json::Value = serde_json::from_str(line).unwrap();
    assert_eq!(json["component"], "daemon");
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
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let registration = state.daemon_registration(addr);
    let app = melo::daemon::server::router(state);
    let temp = tempfile::tempdir().unwrap();
    let state_file = temp.path().join("daemon.json");

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    std::fs::write(&state_file, serde_json::to_string(&registration).unwrap()).unwrap();

    let mut cmd = Command::cargo_bin("melo").unwrap();
    cmd.env("MELO_DAEMON_STATE_FILE", &state_file);
    cmd.arg("status");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Registry Only Song"));
}

#[tokio::test(flavor = "multi_thread")]
async fn status_command_shows_friendly_hint_when_daemon_is_unavailable() {
    let temp = tempfile::tempdir().unwrap();
    let state_file = temp.path().join("missing-daemon.json");

    let mut cmd = Command::cargo_bin("melo").unwrap();
    cmd.env("MELO_DAEMON_STATE_FILE", &state_file);
    cmd.arg("status");

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("daemon"))
        .stderr(predicate::str::contains("melo daemon start"))
        .stderr(predicate::str::contains("api_error").not());
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
async fn verbose_explicit_open_prints_stage_logs_before_business_error() {
    let app = melo::daemon::app::test_router().await;
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let mut cmd = Command::cargo_bin("melo").unwrap();
    cmd.env("MELO_BASE_URL", format!("http://{addr}"));
    cmd.arg("--verbose").arg("cover.jpg");

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("[cli]"))
        .stderr(predicate::str::contains("loading_settings"))
        .stderr(predicate::str::contains("opening_explicit_target"));
}

#[test]
fn verbose_default_launch_prints_daemon_prepare_failure_excerpt() {
    let temp = tempfile::tempdir().unwrap();
    let state_file = temp.path().join("daemon.json");
    let config_path = temp.path().join("config.toml");
    std::fs::write(
        &config_path,
        r#"
[database]
path = "bad<>/melo.db"

[daemon]
host = "127.0.0.1"
base_port = 65529
port_search_limit = 0

[open]
scan_current_dir = false
"#,
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("melo").unwrap();
    cmd.env("MELO_CONFIG_PATH", &config_path);
    cmd.env("MELO_DAEMON_STATE_FILE", &state_file);
    cmd.arg("--verbose");

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("[cli]"))
        .stderr(predicate::str::contains("starting_daemon"))
        .stderr(predicate::str::contains("[daemon]"))
        .stderr(predicate::str::contains("failed to prepare database"));
}

#[tokio::test(flavor = "multi_thread")]
async fn verbose_flag_can_disable_terminal_prefixes() {
    let app = melo::daemon::app::test_router().await;
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let mut cmd = Command::cargo_bin("melo").unwrap();
    cmd.env("MELO_BASE_URL", format!("http://{addr}"));
    cmd.arg("--verbose").arg("--no-log-prefix").arg("cover.jpg");

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("opening_explicit_target"))
        .stderr(predicate::str::contains("[cli]").not())
        .stderr(predicate::str::contains("[daemon]").not());
}

#[tokio::test(flavor = "multi_thread")]
async fn daemon_stop_command_shuts_down_registered_server() {
    let state = melo::daemon::app::AppState::for_test().await;
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let registration = state.daemon_registration(addr);
    let app = melo::daemon::server::router(state.clone());
    let temp = tempfile::tempdir().unwrap();
    let state_file = temp.path().join("daemon.json");

    std::fs::write(&state_file, serde_json::to_string(&registration).unwrap()).unwrap();

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
async fn daemon_start_command_reuses_running_instance() {
    let state = melo::daemon::app::AppState::for_test().await;
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let registration = state.daemon_registration(addr);
    let app = melo::daemon::server::router(state);
    let temp = tempfile::tempdir().unwrap();
    let state_file = temp.path().join("daemon.json");

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    std::fs::write(&state_file, serde_json::to_string(&registration).unwrap()).unwrap();

    let mut cmd = Command::cargo_bin("melo").unwrap();
    cmd.env("MELO_DAEMON_STATE_FILE", &state_file);
    cmd.arg("daemon").arg("start");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("already_running"))
        .stdout(predicate::str::contains("test-instance-1"));
}

#[tokio::test]
async fn daemon_stop_command_clears_stale_registration_without_failing() {
    let temp = tempfile::tempdir().unwrap();
    let state_file = temp.path().join("daemon.json");
    std::fs::write(
        &state_file,
        serde_json::json!({
            "instance_id": "stale-instance",
            "base_url": "http://127.0.0.1:65530",
            "pid": 999999,
            "started_at": "2026-04-11T00:00:00Z",
            "version": env!("CARGO_PKG_VERSION"),
            "backend": "noop",
            "host": "127.0.0.1",
            "port": 65530,
            "log_path": temp.path().join("daemon.log").to_string_lossy().to_string()
        })
        .to_string(),
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("melo").unwrap();
    cmd.env("MELO_DAEMON_STATE_FILE", &state_file);
    cmd.arg("daemon").arg("stop");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("stale_registration_cleared"));
}

#[tokio::test(flavor = "multi_thread")]
async fn play_command_still_controls_a_healthy_daemon_after_autostart_refactor() {
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
    cmd.arg("play");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("playback_state"));
}

#[tokio::test(flavor = "multi_thread")]
async fn daemon_status_command_supports_json_and_verbose_views() {
    let state = melo::daemon::app::AppState::for_test().await;
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let registration = state.daemon_registration(addr);
    let app = melo::daemon::server::router(state);
    let temp = tempfile::tempdir().unwrap();
    let state_file = temp.path().join("daemon.json");
    let log_file = temp.path().join("daemon.log");
    std::fs::write(&log_file, "line 1\nline 2\nline 3\n").unwrap();
    std::fs::write(&state_file, serde_json::to_string(&registration).unwrap()).unwrap();

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let mut json_cmd = Command::cargo_bin("melo").unwrap();
    json_cmd.env("MELO_DAEMON_STATE_FILE", &state_file);
    json_cmd.arg("daemon").arg("status").arg("--json");
    json_cmd
        .assert()
        .success()
        .stdout(predicate::str::contains("\"state\": \"Running\""))
        .stdout(predicate::str::contains(
            "\"instance_id\": \"test-instance-1\"",
        ));

    let mut verbose_cmd = Command::cargo_bin("melo").unwrap();
    verbose_cmd.env("MELO_DAEMON_STATE_FILE", &state_file);
    verbose_cmd.arg("daemon").arg("status").arg("--verbose");
    verbose_cmd
        .assert()
        .success()
        .stdout(predicate::str::contains("registration_path"))
        .stdout(predicate::str::contains("log_path"));
}

#[tokio::test(flavor = "multi_thread")]
async fn daemon_log_level_override_reports_scope_limit_when_daemon_is_already_running() {
    let app = melo::daemon::app::test_router().await;
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let mut cmd = Command::cargo_bin("melo").unwrap();
    cmd.env("MELO_BASE_URL", format!("http://{addr}"));
    cmd.arg("--verbose")
        .arg("--daemon-log-level")
        .arg("trace")
        .arg("play");

    cmd.assert().failure().stderr(predicate::str::contains(
        "daemon_log_level_override_not_applied_to_running_daemon",
    ));
}

#[tokio::test]
async fn daemon_logs_command_reads_requested_tail_count() {
    let temp = tempfile::tempdir().unwrap();
    let state_file = temp.path().join("daemon.json");
    let log_file = temp.path().join("daemon.log");
    std::fs::write(&log_file, "one\ntwo\nthree\nfour\n").unwrap();
    std::fs::write(
        &state_file,
        serde_json::json!({
            "instance_id": "test-instance-1",
            "base_url": "http://127.0.0.1:65530",
            "pid": std::process::id(),
            "started_at": "2026-04-11T00:00:00Z",
            "version": env!("CARGO_PKG_VERSION"),
            "backend": "noop",
            "host": "127.0.0.1",
            "port": 65530,
            "log_path": log_file.to_string_lossy().to_string()
        })
        .to_string(),
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("melo").unwrap();
    cmd.env("MELO_DAEMON_STATE_FILE", &state_file);
    cmd.arg("daemon")
        .arg("logs")
        .arg("--snapshot")
        .arg("--tail")
        .arg("2");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("three"))
        .stdout(predicate::str::contains("four"))
        .stdout(predicate::str::contains("one").not());
}

#[tokio::test]
async fn daemon_logs_snapshot_prints_existing_tail_without_waiting() {
    let temp = tempfile::tempdir().unwrap();
    let state_file = temp.path().join("daemon.json");
    let log_file = temp.path().join("daemon.log");
    std::fs::write(&log_file, "one\ntwo\nthree\n").unwrap();
    std::fs::write(
        &state_file,
        serde_json::json!({
            "instance_id": "test-instance-1",
            "base_url": "http://127.0.0.1:65530",
            "pid": std::process::id(),
            "started_at": "2026-04-12T00:00:00Z",
            "version": env!("CARGO_PKG_VERSION"),
            "backend": "noop",
            "host": "127.0.0.1",
            "port": 65530,
            "log_path": log_file.to_string_lossy().to_string()
        })
        .to_string(),
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("melo").unwrap();
    cmd.env("MELO_DAEMON_STATE_FILE", &state_file);
    cmd.arg("daemon")
        .arg("logs")
        .arg("--snapshot")
        .arg("--tail")
        .arg("2");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("two"))
        .stdout(predicate::str::contains("three"));
}

#[tokio::test(flavor = "multi_thread")]
async fn daemon_docs_print_outputs_local_docs_url() {
    let state = melo::daemon::app::AppState::for_test().await;
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let registration = state.daemon_registration(addr);
    let app = melo::daemon::server::router(state);
    let temp = tempfile::tempdir().unwrap();
    let state_file = temp.path().join("daemon.json");
    std::fs::write(&state_file, serde_json::to_string(&registration).unwrap()).unwrap();

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let mut cmd = Command::cargo_bin("melo").unwrap();
    cmd.env("MELO_DAEMON_STATE_FILE", &state_file);
    cmd.arg("daemon").arg("docs").arg("--print");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("/api/docs/"));
}

#[tokio::test(flavor = "multi_thread")]
async fn daemon_run_serves_docs_to_loopback_clients_in_local_mode() {
    let temp = tempfile::tempdir().unwrap();
    let state_file = temp.path().join("daemon.json");
    let config_path = temp.path().join("config.toml");
    std::fs::write(
        &config_path,
        r#"
[database]
path = "docs-access.db"

[daemon]
host = "127.0.0.1"
base_port = 38641
port_search_limit = 0
docs = "local"
"#,
    )
    .unwrap();

    let daemon_log = temp.path().join("daemon-stderr.log");
    let daemon_out = temp.path().join("daemon-stdout.log");
    let mut daemon = std::process::Command::new(assert_cmd::cargo::cargo_bin("melo"))
        .env("MELO_CONFIG_PATH", &config_path)
        .env("MELO_DAEMON_STATE_FILE", &state_file)
        .arg("daemon")
        .arg("run")
        .stdout(std::fs::File::create(&daemon_out).map(Stdio::from).unwrap())
        .stderr(std::fs::File::create(&daemon_log).map(Stdio::from).unwrap())
        .spawn()
        .unwrap();

    let mut base_url = None;
    for _ in 0..40 {
        if let Ok(json) = std::fs::read_to_string(&state_file)
            && let Ok(value) = serde_json::from_str::<serde_json::Value>(&json)
            && let Some(url) = value.get("base_url").and_then(|item| item.as_str())
        {
            let client = reqwest::Client::builder()
                .timeout(std::time::Duration::from_millis(500))
                .build()
                .unwrap();
            if client
                .get(format!("{url}/api/system/health"))
                .send()
                .await
                .ok()
                .is_some_and(|response| response.status().is_success())
            {
                base_url = Some(url.to_string());
                break;
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;
    }

    let base_url = base_url.expect("daemon should become healthy before docs request");
    let docs_response = reqwest::Client::new()
        .get(format!("{base_url}/api/docs/"))
        .send()
        .await;

    let _ = reqwest::Client::new()
        .post(format!("{base_url}/api/system/shutdown"))
        .send()
        .await;
    let _ = daemon.kill();
    let _ = daemon.wait();

    assert!(
        docs_response
            .as_ref()
            .ok()
            .is_some_and(|response| response.status() == reqwest::StatusCode::OK),
        "docs should be reachable from loopback client, stderr: {}",
        std::fs::read_to_string(&daemon_log).unwrap_or_default()
    );
}

#[tokio::test]
async fn daemon_doctor_and_ps_commands_report_stale_registration() {
    let temp = tempfile::tempdir().unwrap();
    let state_file = temp.path().join("daemon.json");
    let log_file = temp.path().join("daemon.log");
    std::fs::write(
        &state_file,
        serde_json::json!({
            "instance_id": "stale-instance",
            "base_url": "http://127.0.0.1:65530",
            "pid": 999999,
            "started_at": "2026-04-11T00:00:00Z",
            "version": env!("CARGO_PKG_VERSION"),
            "backend": "noop",
            "host": "127.0.0.1",
            "port": 65530,
            "log_path": log_file.to_string_lossy().to_string()
        })
        .to_string(),
    )
    .unwrap();

    let mut doctor = Command::cargo_bin("melo").unwrap();
    doctor.env("MELO_DAEMON_STATE_FILE", &state_file);
    doctor.arg("daemon").arg("doctor").arg("--json");
    doctor
        .assert()
        .success()
        .stdout(predicate::str::contains("\"conclusion\": \"FAIL\""))
        .stdout(predicate::str::contains("\"code\": \"health\""));

    let mut ps = Command::cargo_bin("melo").unwrap();
    ps.env("MELO_DAEMON_STATE_FILE", &state_file);
    ps.arg("daemon").arg("ps");
    ps.assert()
        .success()
        .stdout(predicate::str::contains("registered_pid"))
        .stdout(predicate::str::contains("actual_pid"));
}

#[tokio::test(flavor = "multi_thread")]
async fn daemon_status_without_registration_returns_not_running_summary() {
    let temp = tempfile::tempdir().unwrap();
    let state_file = temp.path().join("daemon.json");

    let mut cmd = Command::cargo_bin("melo").unwrap();
    cmd.env("MELO_DAEMON_STATE_FILE", &state_file);
    cmd.arg("daemon").arg("status");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("NotRunning"));
}

#[test]
fn launch_cwd_text_is_public_for_quit_boundary_regressions() {
    let text = melo::cli::run::launch_cwd_text(std::path::Path::new("D:/Music/Aimer"));
    assert_eq!(text, "D:/Music/Aimer");
}

#[tokio::test(flavor = "multi_thread")]
async fn default_launch_open_request_keeps_real_daemon_healthy_with_libmpv() {
    let temp = tempfile::tempdir().unwrap();
    let state_file = temp.path().join("daemon.json");
    let config_path = temp.path().join("config.toml");
    std::fs::write(
        &config_path,
        r#"
[database]
path = "cli-remote.db"

[daemon]
host = "127.0.0.1"
base_port = 38640
port_search_limit = 0
docs = "disabled"

[player]
backend = "auto"

[open]
scan_current_dir = true
max_depth = 2
prewarm_limit = 20
background_jobs = 4
"#,
    )
    .unwrap();

    let daemon_log = temp.path().join("daemon-stderr.log");
    let daemon_out = temp.path().join("daemon-stdout.log");
    let mut daemon = std::process::Command::new(assert_cmd::cargo::cargo_bin("melo"))
        .env("MELO_CONFIG_PATH", &config_path)
        .env("MELO_DAEMON_STATE_FILE", &state_file)
        .arg("daemon")
        .arg("run")
        .stdout(std::fs::File::create(&daemon_out).map(Stdio::from).unwrap())
        .stderr(std::fs::File::create(&daemon_log).map(Stdio::from).unwrap())
        .spawn()
        .unwrap();

    let mut base_url = None;
    for _ in 0..40 {
        if let Ok(json) = std::fs::read_to_string(&state_file)
            && let Ok(value) = serde_json::from_str::<serde_json::Value>(&json)
            && let Some(url) = value.get("base_url").and_then(|item| item.as_str())
        {
            let client = reqwest::Client::builder()
                .timeout(std::time::Duration::from_millis(500))
                .build()
                .unwrap();
            if client
                .get(format!("{url}/api/system/health"))
                .send()
                .await
                .ok()
                .is_some_and(|response| response.status().is_success())
            {
                base_url = Some(url.to_string());
                break;
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;
    }

    let base_url = base_url.expect("daemon should become healthy before open");
    let open_response = reqwest::Client::new()
        .post(format!("{base_url}/api/open"))
        .json(&serde_json::json!({
            "target": std::env::current_dir().unwrap().to_string_lossy().to_string(),
            "mode": "cwd_dir"
        }))
        .send()
        .await;

    let health_after_open = reqwest::Client::builder()
        .timeout(std::time::Duration::from_millis(500))
        .build()
        .unwrap()
        .get(format!("{base_url}/api/system/health"))
        .send()
        .await;

    let _ = reqwest::Client::new()
        .post(format!("{base_url}/api/system/shutdown"))
        .send()
        .await;
    let _ = daemon.kill();
    let _ = daemon.wait();

    assert!(
        open_response
            .as_ref()
            .ok()
            .is_some_and(|response| response.status().is_success()),
        "open request should succeed, stderr: {}",
        std::fs::read_to_string(&daemon_log).unwrap_or_default()
    );
    assert!(
        health_after_open
            .ok()
            .is_some_and(|response| response.status().is_success()),
        "daemon should stay healthy after open, stderr: {}",
        std::fs::read_to_string(&daemon_log).unwrap_or_default()
    );
}

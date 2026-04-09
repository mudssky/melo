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

#[test]
fn db_path_command_prints_sqlite_location() {
    let mut cmd = Command::cargo_bin("melo").unwrap();
    cmd.arg("db").arg("path");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains(".db"));
}

use std::path::PathBuf;

use crate::daemon::process::{daemon_bind_addr, daemon_command};

#[test]
fn daemon_bind_addr_uses_meolo_base_url_port() {
    let addr = daemon_bind_addr("http://127.0.0.1:38123").unwrap();
    assert_eq!(addr.port(), 38123);
}

#[test]
fn daemon_command_uses_hidden_run_subcommand() {
    let command = daemon_command(PathBuf::from("melo.exe"));
    let args = command
        .get_args()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect::<Vec<_>>();

    assert_eq!(args, vec!["daemon".to_string(), "run".to_string()]);
}

#[tokio::test]
async fn next_bind_addr_skips_busy_base_port() {
    let busy = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let busy_port = busy.local_addr().unwrap().port();

    let addr = crate::daemon::process::next_bind_addr("127.0.0.1", busy_port, 4)
        .await
        .unwrap();

    assert_eq!(addr.ip().to_string(), "127.0.0.1");
    assert_ne!(addr.port(), busy_port);
}

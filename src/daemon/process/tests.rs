use std::path::PathBuf;

use crate::daemon::process::{DaemonLaunchOverrides, daemon_bind_addr, daemon_command};

#[test]
fn daemon_bind_addr_uses_meolo_base_url_port() {
    let addr = daemon_bind_addr("http://127.0.0.1:38123").unwrap();
    assert_eq!(addr.port(), 38123);
}

#[test]
fn daemon_command_uses_hidden_run_subcommand() {
    let command = daemon_command(PathBuf::from("melo.exe"), &DaemonLaunchOverrides::default());
    let args = command
        .get_args()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect::<Vec<_>>();

    assert_eq!(args, vec!["daemon".to_string(), "run".to_string()]);
}

#[test]
fn daemon_command_propagates_runtime_logging_env() {
    let command = daemon_command(
        PathBuf::from("melo.exe"),
        &DaemonLaunchOverrides {
            daemon_log_level: Some("trace".to_string()),
            command_id: Some("command-1".to_string()),
        },
    );
    let envs = command
        .get_envs()
        .map(|(key, value)| {
            (
                key.to_string_lossy().into_owned(),
                value.map(|item| item.to_string_lossy().into_owned()),
            )
        })
        .collect::<Vec<_>>();

    assert!(envs.iter().any(|(key, value)| {
        key == "MELO_DAEMON_LOG_LEVEL_OVERRIDE" && value.as_deref() == Some("trace")
    }));
    assert!(
        envs.iter()
            .any(|(key, value)| key == "MELO_COMMAND_ID" && value.as_deref() == Some("command-1"))
    );
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

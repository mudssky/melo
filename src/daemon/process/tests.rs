use std::path::PathBuf;

use crate::daemon::process::{daemon_bind_addr, daemon_command};

#[test]
fn daemon_bind_addr_uses_meolo_base_url_port() {
    let addr = daemon_bind_addr("http://127.0.0.1:38123").unwrap();
    assert_eq!(addr.port(), 38123);
}

#[test]
fn daemon_command_uses_current_exe_and_daemon_subcommand() {
    let command = daemon_command(PathBuf::from("melo.exe"), "http://127.0.0.1:38123");
    let args = command
        .get_args()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect::<Vec<_>>();

    assert_eq!(args, vec!["daemon".to_string()]);
}

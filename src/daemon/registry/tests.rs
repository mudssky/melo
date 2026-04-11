use std::path::PathBuf;

use crate::daemon::registry::{DaemonRegistration, runtime_paths_from_env};

#[test]
fn runtime_paths_share_state_and_log_directory() {
    let root = PathBuf::from("C:/Temp/melo-tests");
    let paths = runtime_paths_from_env(Some(root.clone()), None, None).unwrap();
    assert_eq!(paths.state_file, root.join("daemon.json"));
    assert_eq!(paths.log_file, root.join("daemon.log"));
}

#[test]
fn runtime_paths_default_under_localappdata() {
    let paths = runtime_paths_from_env(
        None,
        Some(PathBuf::from("C:/Users/test/AppData/Local")),
        None,
    )
    .unwrap();
    assert_eq!(
        paths.state_file,
        PathBuf::from("C:/Users/test/AppData/Local")
            .join("melo")
            .join("daemon.json")
    );
    assert_eq!(
        paths.log_file,
        PathBuf::from("C:/Users/test/AppData/Local")
            .join("melo")
            .join("daemon.log")
    );
}

#[test]
fn registration_round_trips_identity_and_log_metadata() {
    let registration = DaemonRegistration {
        instance_id: "test-instance".to_string(),
        base_url: "http://127.0.0.1:38123".to_string(),
        pid: 4242,
        started_at: "2026-04-11T13:00:00Z".to_string(),
        version: "0.1.0".to_string(),
        backend: "mpv".to_string(),
        host: "127.0.0.1".to_string(),
        port: 38123,
        log_path: "C:/Users/test/AppData/Local/melo/daemon.log".to_string(),
    };

    let json = serde_json::to_string(&registration).unwrap();
    let decoded: DaemonRegistration = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.instance_id, "test-instance");
    assert_eq!(
        decoded.log_path,
        "C:/Users/test/AppData/Local/melo/daemon.log"
    );
}

use std::path::PathBuf;

use crate::daemon::registry::{DaemonRegistration, state_file_path_from_env};

#[test]
fn state_file_path_prefers_explicit_override() {
    let root = PathBuf::from("C:/Temp/melo-tests");
    let path = state_file_path_from_env(Some(root.clone()), None, None).unwrap();
    assert_eq!(path, root.join("daemon.json"));
}

#[test]
fn state_file_path_defaults_under_localappdata() {
    let path = state_file_path_from_env(
        None,
        Some(PathBuf::from("C:/Users/test/AppData/Local")),
        None,
    )
    .unwrap();
    assert_eq!(
        path,
        PathBuf::from("C:/Users/test/AppData/Local")
            .join("melo")
            .join("daemon.json")
    );
}

#[test]
fn registration_round_trips_json() {
    let registration = DaemonRegistration {
        base_url: "http://127.0.0.1:38123".to_string(),
        pid: 4242,
        started_at: "2026-04-11T13:00:00Z".to_string(),
        version: "0.1.0".to_string(),
        backend: "mpv".to_string(),
        host: "127.0.0.1".to_string(),
        port: 38123,
    };

    let json = serde_json::to_string(&registration).unwrap();
    let decoded: DaemonRegistration = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.base_url, "http://127.0.0.1:38123");
    assert_eq!(decoded.backend, "mpv");
}

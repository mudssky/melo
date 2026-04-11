use crate::daemon::observe::{
    DaemonObservation, DaemonState, DoctorLevel, build_doctor_report, classify_state,
};

fn sample_observation() -> DaemonObservation {
    DaemonObservation {
        state: DaemonState::Running,
        registration_exists: true,
        registration_path: "C:/Temp/melo/daemon.json".to_string(),
        base_url: Some("http://127.0.0.1:38123".to_string()),
        instance_id: Some("instance-a".to_string()),
        pid: Some(4242),
        started_at: Some("2026-04-11T13:00:00Z".to_string()),
        backend: Some("noop".to_string()),
        host: Some("127.0.0.1".to_string()),
        port: Some(38123),
        process_exists: true,
        process_start_time_matches: true,
        actual_pid: Some(4242),
        actual_process_path: Some("C:/cargo/bin/melo.exe".to_string()),
        health_ok: true,
        http_instance_id_matches: Some(true),
        shutdown_requested: false,
        log_path: Some("C:/Temp/melo/daemon.log".to_string()),
        log_readable: true,
    }
}

#[test]
fn classify_running_when_http_and_process_match() {
    let state = classify_state(true, true, Some(true), false);
    assert_eq!(state, DaemonState::Running);
}

#[test]
fn classify_registered_but_unhealthy_when_process_start_mismatches() {
    let state = classify_state(true, false, Some(true), false);
    assert_eq!(state, DaemonState::RegisteredButUnhealthy);
}

#[test]
fn doctor_report_flags_instance_id_mismatch_as_fail() {
    let mut observation = sample_observation();
    observation.health_ok = true;
    observation.http_instance_id_matches = Some(false);
    observation.state = DaemonState::RegisteredButUnhealthy;

    let report = build_doctor_report(&observation);

    assert_eq!(report.conclusion, DoctorLevel::FAIL);
    assert!(
        report
            .checks
            .iter()
            .any(|check| check.code == "instance_id" && check.level == DoctorLevel::FAIL)
    );
}

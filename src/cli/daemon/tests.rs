use crate::cli::daemon::{format_doctor_human, format_status_human};
use crate::daemon::observe::{
    DaemonObservation, DaemonState, DoctorCheck, DoctorLevel, DoctorReport,
};

fn observation() -> DaemonObservation {
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
fn format_status_human_keeps_default_output_compact() {
    let text = format_status_human(&observation(), false);
    assert!(text.contains("state: Running"));
    assert!(!text.contains("registration_path"));
}

#[test]
fn format_doctor_human_prints_check_evidence() {
    let text = format_doctor_human(&DoctorReport {
        conclusion: DoctorLevel::FAIL,
        checks: vec![DoctorCheck {
            code: "health",
            level: DoctorLevel::FAIL,
            summary: "health/status 探测".to_string(),
            evidence: "base_url=http://127.0.0.1:65530".to_string(),
        }],
        observation: observation(),
    });

    assert!(text.contains("conclusion: FAIL"));
    assert!(text.contains("base_url=http://127.0.0.1:65530"));
}

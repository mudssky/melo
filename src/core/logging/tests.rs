use std::sync::{Arc, Mutex};

use crate::core::config::settings::{LoggingFormat, LoggingLevel, Settings};
use crate::core::logging::{
    CliLogOverrides, LogComponent, RuntimeLogContext, init_tracing_for_test,
    resolve_logging_options,
};

#[test]
fn resolve_logging_options_prefers_cli_override_then_component_then_global() {
    let mut settings = Settings::default();
    settings.logging.level = LoggingLevel::Warning;
    settings.logging.prefix_enabled = true;
    settings.logging.cli.level = Some(LoggingLevel::Info);
    settings.logging.cli.prefix_enabled = Some(false);

    let resolved = resolve_logging_options(
        &settings,
        LogComponent::Cli,
        &CliLogOverrides {
            verbose: false,
            log_level: Some(LoggingLevel::Trace),
            no_log_prefix: false,
            daemon_log_level: None,
        },
    );

    assert_eq!(resolved.level, LoggingLevel::Trace);
    assert!(!resolved.prefix_enabled);
}

#[test]
fn resolve_logging_options_blocks_daemon_runtime_override_when_component_disables_it() {
    let mut settings = Settings::default();
    settings.logging.level = LoggingLevel::Warning;
    settings.logging.daemon.level = Some(LoggingLevel::Warning);
    settings.logging.daemon.allow_runtime_level_override = false;

    let resolved = resolve_logging_options(
        &settings,
        LogComponent::Daemon,
        &CliLogOverrides {
            verbose: false,
            log_level: None,
            no_log_prefix: false,
            daemon_log_level: Some(LoggingLevel::Trace),
        },
    );

    assert_eq!(resolved.level, LoggingLevel::Warning);
    assert!(resolved.daemon_runtime_override_blocked);
}

#[test]
fn init_tracing_for_test_writes_json_file_log_with_component_field() {
    let temp = tempfile::tempdir().unwrap();
    let mut settings = Settings::default();
    settings.logging.level = LoggingLevel::Info;
    settings.logging.file_format = LoggingFormat::Json;
    settings.logging.cli.file_enabled = true;
    settings.logging.cli.file_path =
        Some(temp.path().join("cli.log").to_string_lossy().to_string());

    let terminal = Arc::new(Mutex::new(Vec::new()));
    let guard = init_tracing_for_test(
        &settings,
        LogComponent::Cli,
        &CliLogOverrides::default(),
        RuntimeLogContext {
            session_id: "session-1".into(),
            command_id: "command-1".into(),
        },
        terminal,
    );

    tracing::info!(target: "melo::tests", component = "cli", "boot");
    drop(guard);

    let contents = std::fs::read_to_string(temp.path().join("cli.log")).unwrap();
    let line = contents.lines().last().unwrap();
    let json: serde_json::Value = serde_json::from_str(line).unwrap();
    assert_eq!(json["component"], "cli");
    assert_eq!(json["level"], "INFO");
    assert_eq!(json["fields"]["message"], "boot");
}

#[test]
fn daemon_override_notice_reports_running_daemon_scope_limit() {
    let settings = Settings::default();
    let notice = crate::core::logging::daemon_override_notice(
        &settings,
        &CliLogOverrides {
            verbose: true,
            log_level: None,
            no_log_prefix: false,
            daemon_log_level: Some(LoggingLevel::Trace),
        },
        true,
    );

    assert_eq!(
        notice,
        Some("daemon_log_level_override_not_applied_to_running_daemon")
    );
}

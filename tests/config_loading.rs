use std::fs;

use melo::core::config::settings::Settings;
use tempfile::tempdir;

#[test]
fn settings_load_new_player_open_and_ephemeral_fields() {
    let temp = tempdir().unwrap();
    let path = temp.path().join("config.toml");
    fs::write(
        &path,
        r#"
[database]
path = "local/melo.db"

[player]
volume = 55
restore_last_session = false
resume_after_restore = true

[open]
scan_current_dir = false
max_depth = 3
prewarm_limit = 8
background_jobs = 2

[playlists.ephemeral]
default_ttl_seconds = 3600

[playlists.ephemeral.visibility]
path_file = true
path_dir = false
cwd_dir = true
"#,
    )
    .unwrap();

    let settings = Settings::load_from_path(&path).unwrap();

    assert_eq!(settings.player.volume, 55);
    assert!(!settings.player.restore_last_session);
    assert!(settings.player.resume_after_restore);
    assert!(!settings.open.scan_current_dir);
    assert_eq!(settings.open.max_depth, 3);
    assert_eq!(settings.open.prewarm_limit, 8);
    assert_eq!(settings.open.background_jobs, 2);
    assert_eq!(settings.playlists.ephemeral.default_ttl_seconds, 3600);
    assert!(settings.playlists.ephemeral.visibility.path_file);
    assert!(!settings.playlists.ephemeral.visibility.path_dir);
    assert!(settings.playlists.ephemeral.visibility.cwd_dir);
}

#[test]
fn config_example_toml_parses_successfully() {
    assert!(std::path::Path::new("config.example.toml").exists());
    let settings = Settings::load_from_path("config.example.toml").unwrap();
    assert_eq!(settings.player.volume, 100);
    assert_eq!(settings.playlists.ephemeral.default_ttl_seconds, 0);
}

#[test]
fn settings_load_daemon_backend_and_tui_fields() {
    let temp = tempdir().unwrap();
    let path = temp.path().join("config.toml");
    fs::write(
        &path,
        r#"
[database]
path = "local/melo.db"

[daemon]
host = "127.0.0.1"
base_port = 38123
port_search_limit = 12

[player]
backend = "auto"
volume = 70
restore_last_session = true
resume_after_restore = false

[player.mpv]
path = "C:/Tools/mpv.exe"
ipc_dir = "auto"

[tui]
show_footer_hints = false
"#,
    )
    .unwrap();

    let settings = Settings::load_from_path(&path).unwrap();

    assert_eq!(settings.daemon.host, "127.0.0.1");
    assert_eq!(settings.daemon.base_port, 38123);
    assert_eq!(settings.daemon.port_search_limit, 12);
    assert_eq!(settings.player.backend, "auto");
    assert_eq!(settings.player.mpv.path, "C:/Tools/mpv.exe");
    assert_eq!(settings.player.mpv.ipc_dir, "auto");
    assert!(!settings.tui.show_footer_hints);
}

#[test]
fn settings_resolve_database_path_relative_to_config_file_directory() {
    let temp = tempdir().unwrap();
    let config_dir = temp.path().join("melo-root");
    fs::create_dir_all(&config_dir).unwrap();
    let path = config_dir.join("config.toml");
    fs::write(
        &path,
        r#"
[database]
path = "melo.db"
"#,
    )
    .unwrap();

    let settings = Settings::load_from_path(&path).unwrap();

    assert_eq!(
        settings.database.path.as_std_path(),
        config_dir.join("melo.db").as_path()
    );
}

#[test]
fn settings_allow_database_path_override_from_env() {
    let temp = tempdir().unwrap();
    let config_path = temp.path().join("config.toml");
    let db_path = temp.path().join("override.db");
    fs::write(&config_path, "").unwrap();

    unsafe {
        std::env::set_var("MELO_DB_PATH", &db_path);
    }
    let settings = Settings::load_from_path(&config_path).unwrap();
    unsafe {
        std::env::remove_var("MELO_DB_PATH");
    }

    assert_eq!(settings.database.path.as_std_path(), db_path.as_path());
}

#[test]
fn settings_load_runtime_scan_template_overrides() {
    let temp = tempdir().unwrap();
    let path = temp.path().join("config.toml");
    fs::write(
        &path,
        r#"
[database]
path = "local/melo.db"

[templates.runtime.scan]
cli_start = "Start {{ source_label }}"
cli_handoff = "Into TUI"
tui_active = "{{ indexed_count }} / {{ discovered_count }} · {{ current_item_name }}"
tui_done = "Done {{ queued_count }}"
tui_failed = "Failed {{ error_message }}"
"#,
    )
    .unwrap();

    let settings = Settings::load_from_path(&path).unwrap();

    assert_eq!(
        settings.templates.runtime.scan.cli_start.as_deref(),
        Some("Start {{ source_label }}")
    );
    assert_eq!(
        settings.templates.runtime.scan.tui_active.as_deref(),
        Some("{{ indexed_count }} / {{ discovered_count }} · {{ current_item_name }}")
    );
}

#[test]
fn settings_load_logging_defaults_and_component_overrides() {
    let temp = tempdir().unwrap();
    let path = temp.path().join("config.toml");
    fs::write(
        &path,
        r#"
[database]
path = "local/melo.db"

[logging]
level = "warning"
terminal_format = "pretty"
file_format = "json"
prefix_enabled = false
cli_prefix = "term"
daemon_prefix = "svc"

[logging.cli]
file_enabled = true
file_path = "logs/cli.log"

[logging.daemon]
file_enabled = true
file_path = "logs/daemon.log"
allow_runtime_level_override = true
"#,
    )
    .unwrap();

    let settings = Settings::load_from_path(&path).unwrap();

    assert_eq!(settings.logging.level.as_str(), "warning");
    assert_eq!(settings.logging.terminal_format.as_str(), "pretty");
    assert_eq!(settings.logging.file_format.as_str(), "json");
    assert!(!settings.logging.prefix_enabled);
    assert_eq!(settings.logging.cli_prefix, "term");
    assert_eq!(settings.logging.daemon_prefix, "svc");
    assert!(settings.logging.cli.file_enabled);
    assert_eq!(
        settings.logging.cli.file_path.as_deref(),
        Some("logs/cli.log")
    );
    assert!(settings.logging.daemon.file_enabled);
    assert_eq!(
        settings.logging.daemon.file_path.as_deref(),
        Some("logs/daemon.log")
    );
    assert!(settings.logging.daemon.allow_runtime_level_override);
}

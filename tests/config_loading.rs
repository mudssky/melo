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

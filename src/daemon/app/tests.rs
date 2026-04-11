use std::fs;

#[tokio::test]
async fn daemon_app_state_new_bootstraps_database_before_services_use_it() {
    let temp = tempfile::tempdir().unwrap();
    let config_path = temp.path().join("config.toml");
    fs::write(
        &config_path,
        r#"
[database]
path = "runtime/melo.db"
"#,
    )
    .unwrap();

    unsafe {
        std::env::set_var("MELO_CONFIG_PATH", &config_path);
    }
    let state = crate::daemon::app::AppState::new().await;
    unsafe {
        std::env::remove_var("MELO_CONFIG_PATH");
    }

    assert!(state.is_ok());
    assert!(temp.path().join("runtime").exists());
    assert!(temp.path().join("runtime/melo.db").exists());
}

use melo::core::config::settings::Settings;
use melo::core::db::bootstrap::DatabaseBootstrap;
use tempfile::tempdir;

#[tokio::test]
async fn db_init_creates_required_tables() {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("melo.db");

    let settings = Settings::for_test(db_path.clone());
    DatabaseBootstrap::new(&settings).init().await.unwrap();

    let tables = DatabaseBootstrap::table_names(db_path).unwrap();

    assert!(tables.contains(&"artists".to_string()));
    assert!(tables.contains(&"albums".to_string()));
    assert!(tables.contains(&"songs".to_string()));
    assert!(tables.contains(&"playlists".to_string()));
    assert!(tables.contains(&"playlist_entries".to_string()));
    assert!(tables.contains(&"artwork_refs".to_string()));
    assert!(tables.contains(&"migrations".to_string()));
}

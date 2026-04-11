use sea_orm::{ConnectionTrait, Database, Statement};

use melo::core::config::settings::Settings;
use melo::core::db::bootstrap::DatabaseBootstrap;
use tempfile::tempdir;

#[tokio::test]
async fn db_init_runs_seaorm_migrations() {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("melo.db");

    let settings = Settings::for_test(db_path.clone());
    DatabaseBootstrap::new(&settings).init().await.unwrap();

    let database_url = format!(
        "sqlite://{}?mode=rwc",
        db_path.to_string_lossy().replace('\\', "/")
    );
    let connection = Database::connect(&database_url).await.unwrap();
    let rows = connection
        .query_all(Statement::from_string(
            sea_orm::DatabaseBackend::Sqlite,
            "SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name".to_string(),
        ))
        .await
        .unwrap();
    let tables = rows
        .into_iter()
        .map(|row| row.try_get::<String>("", "name").unwrap())
        .collect::<Vec<_>>();

    assert!(tables.contains(&"artists".to_string()));
    assert!(tables.contains(&"albums".to_string()));
    assert!(tables.contains(&"songs".to_string()));
    assert!(tables.contains(&"playlists".to_string()));
    assert!(tables.contains(&"playlist_entries".to_string()));
    assert!(tables.contains(&"artwork_refs".to_string()));
    assert!(tables.contains(&"player_sessions".to_string()));
    assert!(tables.contains(&"player_session_items".to_string()));
    assert!(tables.contains(&"seaql_migrations".to_string()));

    let playlist_columns = connection
        .query_all(Statement::from_string(
            sea_orm::DatabaseBackend::Sqlite,
            "PRAGMA table_info(playlists)".to_string(),
        ))
        .await
        .unwrap()
        .into_iter()
        .map(|row| row.try_get::<String>("", "name").unwrap())
        .collect::<Vec<_>>();

    assert!(playlist_columns.contains(&"kind".to_string()));
    assert!(playlist_columns.contains(&"source_kind".to_string()));
    assert!(playlist_columns.contains(&"source_key".to_string()));
    assert!(playlist_columns.contains(&"visible".to_string()));
    assert!(playlist_columns.contains(&"expires_at".to_string()));
    assert!(playlist_columns.contains(&"last_activated_at".to_string()));
}

# Melo Direct Open And Ephemeral Playlists Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `melo`, `melo <audio-file>`, and `melo <dir>` open directly into a real daemon-backed TUI flow, backed by persistent ephemeral playlists and fully documented configuration.

**Architecture:** Keep the daemon as the playback authority, add a new `domain::open` orchestration layer that classifies paths, reuses scanned library data, and materializes ephemeral playlists, then let CLI dispatch decide whether to remote-control or directly open. Extend the existing `playlists` table instead of inventing a separate temp-playlist schema so reuse, promotion, cleanup, and TUI visibility all share one model.

**Tech Stack:** Rust 2024, Clap 4 derive parsing plus raw argv pre-dispatch, Tokio, Axum, SeaORM + SeaORM Migration, Ratatui + Crossterm, existing player session persistence, existing integration test harness, `pnpm qa`

---

## File structure impact

### Existing files to modify

- Modify: `src/core/config/settings.rs`
- Modify: `src/core/db/entities/mod.rs`
- Modify: `src/core/db/entities/playlists.rs`
- Modify: `src/core/db/migrations/mod.rs`
- Modify: `src/core/db/migrator.rs`
- Modify: `src/domain/mod.rs`
- Modify: `src/domain/library/repository.rs`
- Modify: `src/domain/library/service.rs`
- Modify: `src/domain/playlist/repository.rs`
- Modify: `src/domain/playlist/service.rs`
- Modify: `src/api/mod.rs`
- Modify: `src/daemon/app.rs`
- Modify: `src/daemon/mod.rs`
- Modify: `src/daemon/server.rs`
- Modify: `src/cli/args.rs`
- Modify: `src/cli/client.rs`
- Modify: `src/cli/mod.rs`
- Modify: `src/cli/run.rs`
- Modify: `src/tui/app.rs`
- Modify: `src/tui/mod.rs`
- Modify: `tests/db_bootstrap.rs`
- Modify: `tests/playlist_service.rs`
- Modify: `tests/cli_help.rs`
- Modify: `tests/cli_remote.rs`

### New files to create

- Create: `src/core/db/migrations/m20260411_000003_ephemeral_playlists.rs`
- Create: `src/domain/open/mod.rs`
- Create: `src/domain/open/service.rs`
- Create: `src/domain/open/service/tests.rs`
- Create: `src/api/open.rs`
- Create: `src/daemon/process.rs`
- Create: `src/daemon/process/tests.rs`
- Create: `src/cli/dispatch.rs`
- Create: `src/cli/dispatch/tests.rs`
- Create: `src/tui/run.rs`
- Create: `src/tui/run/tests.rs`
- Create: `tests/config_loading.rs`
- Create: `tests/open_api.rs`
- Create: `tests/ephemeral_playlists.rs`
- Create: `config.example.toml`

### Responsibilities

- `src/core/config/settings.rs`
  Central config model and `load_from_path()` helper for both runtime and example-config validation
- `src/core/db/migrations/m20260411_000003_ephemeral_playlists.rs`
  Add playlist lifecycle/source metadata without rewriting the initial migration
- `src/domain/playlist/repository.rs`
  Persistent upsert/reuse/promote/cleanup queries for `kind = ephemeral`
- `src/domain/open/service.rs`
  Path classification, directory discovery, hot-path library reuse, prewarm, and queue materialization
- `src/api/open.rs`
  Daemon endpoint for direct-open requests
- `src/daemon/process.rs`
  Parse bind address from `MELO_BASE_URL`, run the daemon server, and auto-spawn a child daemon when needed
- `src/cli/dispatch.rs`
  Distinguish `melo`, `melo <path>`, and real subcommands before Clap consumes argv
- `src/tui/run.rs`
  Real terminal UI event loop, startup context display, and graceful quit handling
- `config.example.toml`
  Full, commented example config covering old and new options

---

### Task 1: Extend config loading and playlist schema for ephemeral metadata

**Files:**
- Create: `src/core/db/migrations/m20260411_000003_ephemeral_playlists.rs`
- Create: `tests/config_loading.rs`
- Modify: `src/core/config/settings.rs`
- Modify: `src/core/db/entities/mod.rs`
- Modify: `src/core/db/entities/playlists.rs`
- Modify: `src/core/db/migrations/mod.rs`
- Modify: `src/core/db/migrator.rs`
- Modify: `tests/db_bootstrap.rs`

- [ ] **Step 1: Write the failing config and schema tests**

```rust
// tests/config_loading.rs
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
```

```rust
// append to tests/db_bootstrap.rs
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
```

- [ ] **Step 2: Run the focused tests to verify they fail**

Run: `cargo test --test config_loading settings_load_new_player_open_and_ephemeral_fields -- --nocapture`  
Expected: FAIL because `Settings::load_from_path()` and the new config structs do not exist yet.

Run: `cargo test --test db_bootstrap db_init_runs_seaorm_migrations -- --nocapture`  
Expected: FAIL because the `playlists` table does not yet contain the new metadata columns.

- [ ] **Step 3: Implement config structs, defaults, and the playlist migration**

```rust
// src/core/config/settings.rs
#[derive(Debug, Clone, Deserialize)]
pub struct PlayerSettings {
    pub volume: u8,
    pub restore_last_session: bool,
    pub resume_after_restore: bool,
}

impl Default for PlayerSettings {
    fn default() -> Self {
        Self {
            volume: 100,
            restore_last_session: true,
            resume_after_restore: false,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct OpenSettings {
    pub scan_current_dir: bool,
    pub max_depth: usize,
    pub prewarm_limit: usize,
    pub background_jobs: usize,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct EphemeralVisibilitySettings {
    pub path_file: bool,
    pub path_dir: bool,
    pub cwd_dir: bool,
}
```

```rust
// src/core/config/settings.rs
#[derive(Debug, Clone, Deserialize)]
pub struct EphemeralPlaylistSettings {
    pub default_ttl_seconds: u64,
    #[serde(default)]
    pub visibility: EphemeralVisibilitySettings,
}

impl Default for EphemeralPlaylistSettings {
    fn default() -> Self {
        Self {
            default_ttl_seconds: 0,
            visibility: EphemeralVisibilitySettings {
                path_file: false,
                path_dir: true,
                cwd_dir: true,
            },
        }
    }
}

impl Settings {
    pub fn load_from_path(path: impl AsRef<std::path::Path>) -> MeloResult<Self> {
        config::Config::builder()
            .add_source(config::File::from(path.as_ref()).required(false))
            .set_default("database.path", "local/melo.db")
            .map_err(|err| MeloError::Message(err.to_string()))?
            .set_default("player.volume", 100)
            .map_err(|err| MeloError::Message(err.to_string()))?
            .set_default("player.restore_last_session", true)
            .map_err(|err| MeloError::Message(err.to_string()))?
            .set_default("player.resume_after_restore", false)
            .map_err(|err| MeloError::Message(err.to_string()))?
            .set_default("open.scan_current_dir", true)
            .map_err(|err| MeloError::Message(err.to_string()))?
            .set_default("open.max_depth", 2)
            .map_err(|err| MeloError::Message(err.to_string()))?
            .set_default("open.prewarm_limit", 20)
            .map_err(|err| MeloError::Message(err.to_string()))?
            .set_default("open.background_jobs", 4)
            .map_err(|err| MeloError::Message(err.to_string()))?
            .set_default("playlists.ephemeral.default_ttl_seconds", 0)
            .map_err(|err| MeloError::Message(err.to_string()))?
            .build()
            .map_err(|err| MeloError::Message(err.to_string()))?
            .try_deserialize()
            .map_err(|err| MeloError::Message(err.to_string()))
    }
}
```

```rust
// src/core/db/migrations/m20260411_000003_ephemeral_playlists.rs
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Playlists::Table)
                    .add_column(ColumnDef::new(Playlists::Kind).string().not_null().default("static"))
                    .add_column(ColumnDef::new(Playlists::SourceKind).string())
                    .add_column(ColumnDef::new(Playlists::SourceKey).string())
                    .add_column(ColumnDef::new(Playlists::Visible).boolean().not_null().default(true))
                    .add_column(ColumnDef::new(Playlists::ExpiresAt).string())
                    .add_column(ColumnDef::new(Playlists::LastActivatedAt).string())
                    .to_owned(),
            )
            .await?;
        Ok(())
    }
}
```

```rust
// src/core/db/entities/playlists.rs
pub struct Model {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub kind: String,
    pub source_kind: Option<String>,
    pub source_key: Option<String>,
    pub visible: bool,
    pub expires_at: Option<String>,
    pub last_activated_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}
```

- [ ] **Step 4: Run the focused tests and verify they pass**

Run: `cargo test --test config_loading settings_load_new_player_open_and_ephemeral_fields -- --nocapture`  
Expected: PASS and the new config hierarchy round-trips from TOML.

Run: `cargo test --test db_bootstrap db_init_runs_seaorm_migrations -- --nocapture`  
Expected: PASS and the `playlists` table exposes the new metadata columns.

- [ ] **Step 5: Run project QA before committing**

Run: `pnpm qa`  
Expected: PASS including formatting, clippy, and the full test suite.

- [ ] **Step 6: Commit the config/schema slice**

```bash
git add src/core/config/settings.rs src/core/db/entities/mod.rs src/core/db/entities/playlists.rs src/core/db/migrations/mod.rs src/core/db/migrations/m20260411_000003_ephemeral_playlists.rs src/core/db/migrator.rs tests/config_loading.rs tests/db_bootstrap.rs
git commit -m "feat: add ephemeral playlist config and schema"
```

---

### Task 2: Add repository and service support for ephemeral playlist reuse, promotion, and cleanup

**Files:**
- Create: `tests/ephemeral_playlists.rs`
- Modify: `src/domain/playlist/repository.rs`
- Modify: `src/domain/playlist/service.rs`
- Modify: `tests/playlist_service.rs`

- [ ] **Step 1: Write the failing playlist lifecycle tests**

```rust
// tests/ephemeral_playlists.rs
#[tokio::test]
async fn upsert_ephemeral_reuses_same_source_key() {
    let harness = melo::test_support::TestHarness::new().await;
    harness.seed_song("Blue Bird", "Ikimono-gakari", "Blue Bird", 2008).await;
    harness.seed_song("Brave Shine", "Aimer", "Brave Shine", 2015).await;
    let playlist_service = harness.playlist_service();

    let first = playlist_service
        .upsert_ephemeral(
            "D:/Music/Anime",
            "path_dir",
            "D:/Music/Anime",
            true,
            None,
            &[1, 2],
        )
        .await
        .unwrap();
    let second = playlist_service
        .upsert_ephemeral(
            "D:/Music/Anime",
            "path_dir",
            "D:/Music/Anime",
            true,
            None,
            &[1, 2],
        )
        .await
        .unwrap();

    assert_eq!(first.id, second.id);
}

#[tokio::test]
async fn promote_ephemeral_turns_it_into_static_playlist() {
    let harness = melo::test_support::TestHarness::new().await;
    harness.seed_song("Blue Bird", "Ikimono-gakari", "Blue Bird", 2008).await;
    let playlist_service = harness.playlist_service();

    playlist_service
        .upsert_ephemeral("blue-bird.mp3", "path_file", "D:/Music/blue-bird.mp3", false, None, &[1])
        .await
        .unwrap();

    playlist_service
        .promote_ephemeral("D:/Music/blue-bird.mp3", "Single Favorites")
        .await
        .unwrap();

    let playlists = playlist_service.list_all().await.unwrap();
    assert!(playlists.iter().any(|playlist| {
        playlist.name == "Single Favorites" && playlist.kind == "static"
    }));
}

#[tokio::test]
async fn cleanup_expired_removes_only_elapsed_ephemeral_playlists() {
    let harness = melo::test_support::TestHarness::new().await;
    harness.seed_song("Blue Bird", "Ikimono-gakari", "Blue Bird", 2008).await;
    let playlist_service = harness.playlist_service();

    playlist_service
        .upsert_ephemeral(
            "Expired Dir",
            "cwd_dir",
            "D:/Music/Expired",
            true,
            Some("2000-01-01T00:00:00Z"),
            &[1],
        )
        .await
        .unwrap();

    let deleted = playlist_service.cleanup_expired("2026-04-11T00:00:00Z").await.unwrap();
    assert_eq!(deleted, 1);
}
```

- [ ] **Step 2: Run the focused tests to verify they fail**

Run: `cargo test --test ephemeral_playlists upsert_ephemeral_reuses_same_source_key -- --nocapture`  
Expected: FAIL because `upsert_ephemeral()`, `list_visible()`, and the playlist metadata fields do not exist yet.

Run: `cargo test --test ephemeral_playlists promote_ephemeral_turns_it_into_static_playlist -- --nocapture`  
Expected: FAIL because promotion logic does not exist yet.

- [ ] **Step 3: Implement ephemeral playlist persistence and service wrappers**

```rust
// src/domain/playlist/repository.rs
#[derive(Debug, Clone)]
pub struct StoredPlaylist {
    pub id: i64,
    pub name: String,
    pub kind: String,
    pub visible: bool,
}

impl PlaylistRepository {
    pub async fn upsert_ephemeral(
        &self,
        name: &str,
        source_kind: &str,
        source_key: &str,
        visible: bool,
        expires_at: Option<&str>,
        song_ids: &[i64],
    ) -> MeloResult<StoredPlaylist> {
        let connection = connect(&self.settings).await?;
        let now = crate::core::db::now_text();

        let existing = playlists::Entity::find()
            .filter(playlists::Column::Kind.eq("ephemeral"))
            .filter(playlists::Column::SourceKind.eq(source_kind))
            .filter(playlists::Column::SourceKey.eq(source_key))
            .one(&connection)
            .await
            .map_err(|err| MeloError::Message(err.to_string()))?;

        let playlist_id = if let Some(existing) = existing {
            let id = existing.id;
            let mut model: playlists::ActiveModel = existing.into_active_model();
            model.name = Set(name.to_string());
            model.visible = Set(visible);
            model.expires_at = Set(expires_at.map(ToString::to_string));
            model.last_activated_at = Set(Some(now.clone()));
            model.updated_at = Set(now.clone());
            model.update(&connection).await.map_err(|err| MeloError::Message(err.to_string()))?;

            playlist_entries::Entity::delete_many()
                .filter(playlist_entries::Column::PlaylistId.eq(id))
                .exec(&connection)
                .await
                .map_err(|err| MeloError::Message(err.to_string()))?;
            id
        } else {
            playlists::ActiveModel {
                name: Set(name.to_string()),
                description: Set(None),
                kind: Set("ephemeral".to_string()),
                source_kind: Set(Some(source_kind.to_string())),
                source_key: Set(Some(source_key.to_string())),
                visible: Set(visible),
                expires_at: Set(expires_at.map(ToString::to_string)),
                last_activated_at: Set(Some(now.clone())),
                created_at: Set(now.clone()),
                updated_at: Set(now.clone()),
                ..Default::default()
            }
            .insert(&connection)
            .await
            .map_err(|err| MeloError::Message(err.to_string()))?
            .id
        };

        Ok(StoredPlaylist {
            id: playlist_id,
            name: name.to_string(),
            kind: "ephemeral".to_string(),
            visible,
        })
    }

    pub async fn promote_ephemeral(&self, source_key: &str, new_name: &str) -> MeloResult<()> {
        let connection = connect(&self.settings).await?;
        let playlist = playlists::Entity::find()
            .filter(playlists::Column::Kind.eq("ephemeral"))
            .filter(playlists::Column::SourceKey.eq(source_key))
            .one(&connection)
            .await
            .map_err(|err| MeloError::Message(err.to_string()))?
            .ok_or_else(|| MeloError::Message(format!("未找到临时歌单: {source_key}")))?;

        let mut model: playlists::ActiveModel = playlist.into_active_model();
        model.name = Set(new_name.to_string());
        model.kind = Set("static".to_string());
        model.source_kind = Set(None);
        model.source_key = Set(None);
        model.visible = Set(true);
        model.expires_at = Set(None);
        model.last_activated_at = Set(None);
        model.update(&connection).await.map_err(|err| MeloError::Message(err.to_string()))?;
        Ok(())
    }

    pub async fn cleanup_expired(&self, now_text: &str) -> MeloResult<u64> {
        let connection = connect(&self.settings).await?;
        playlists::Entity::delete_many()
            .filter(playlists::Column::Kind.eq("ephemeral"))
            .filter(playlists::Column::ExpiresAt.is_not_null())
            .filter(playlists::Column::ExpiresAt.lte(now_text))
            .exec(&connection)
            .await
            .map(|result| result.rows_affected)
            .map_err(|err| MeloError::Message(err.to_string()))
    }
}
```

```rust
// src/domain/playlist/service.rs
impl PlaylistService {
    pub async fn upsert_ephemeral(
        &self,
        name: &str,
        source_kind: &str,
        source_key: &str,
        visible: bool,
        expires_at: Option<&str>,
        song_ids: &[i64],
    ) -> MeloResult<crate::domain::playlist::repository::StoredPlaylist> {
        self.repository
            .upsert_ephemeral(name, source_kind, source_key, visible, expires_at, song_ids)
            .await
    }

    pub async fn list_visible(&self) -> MeloResult<Vec<PlaylistSummary>> {
        self.repository.list_visible().await
    }

    pub async fn promote_ephemeral(&self, source_key: &str, new_name: &str) -> MeloResult<()> {
        self.repository.promote_ephemeral(source_key, new_name).await
    }

    pub async fn cleanup_expired(&self, now_text: &str) -> MeloResult<u64> {
        self.repository.cleanup_expired(now_text).await
    }
}
```

- [ ] **Step 4: Run the focused tests and verify they pass**

Run: `cargo test --test ephemeral_playlists upsert_ephemeral_reuses_same_source_key -- --nocapture`  
Expected: PASS and repeated opens reuse one persistent playlist row.

Run: `cargo test --test ephemeral_playlists promote_ephemeral_turns_it_into_static_playlist -- --nocapture`  
Expected: PASS and promoted playlists reappear as `kind == "static"`.

Run: `cargo test --test ephemeral_playlists cleanup_expired_removes_only_elapsed_ephemeral_playlists -- --nocapture`  
Expected: PASS and expired ephemeral rows are deleted without touching other playlists.

- [ ] **Step 5: Run project QA before committing**

Run: `pnpm qa`  
Expected: PASS including formatting, clippy, and the full test suite.

- [ ] **Step 6: Commit the playlist lifecycle slice**

```bash
git add src/domain/playlist/repository.rs src/domain/playlist/service.rs tests/ephemeral_playlists.rs tests/playlist_service.rs
git commit -m "feat: add ephemeral playlist lifecycle support"
```

---

### Task 3: Add the direct-open domain service and daemon API endpoint

**Files:**
- Create: `src/domain/open/mod.rs`
- Create: `src/domain/open/service.rs`
- Create: `src/domain/open/service/tests.rs`
- Create: `src/api/open.rs`
- Create: `tests/open_api.rs`
- Modify: `src/domain/mod.rs`
- Modify: `src/domain/library/repository.rs`
- Modify: `src/domain/library/service.rs`
- Modify: `src/api/mod.rs`
- Modify: `src/daemon/app.rs`
- Modify: `src/daemon/server.rs`

- [ ] **Step 1: Write the failing open-service and API tests**

```rust
// src/domain/open/service/tests.rs
use std::path::PathBuf;

use crate::domain::open::service::{classify_target, discover_audio_paths};

#[test]
fn classify_target_rejects_images() {
    let err = classify_target(&PathBuf::from("cover.jpg")).unwrap_err();
    assert!(err.to_string().contains("unsupported_open_format"));
}

#[test]
fn discover_audio_paths_respects_max_depth() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(temp.path().join("a/b/c")).unwrap();
    std::fs::write(temp.path().join("a").join("one.mp3"), b"audio").unwrap();
    std::fs::write(temp.path().join("a/b").join("two.flac"), b"audio").unwrap();
    std::fs::write(temp.path().join("a/b/c").join("three.ogg"), b"audio").unwrap();

    let found = discover_audio_paths(&temp.path().join("a"), 1).unwrap();
    assert_eq!(found.len(), 2);
}
```

```rust
// tests/open_api.rs
use axum::body::Body;
use axum::http::{Request, StatusCode};
use tower::util::ServiceExt;

#[tokio::test]
async fn open_endpoint_rejects_unsupported_file_types() {
    let harness = melo::test_support::TestHarness::new().await;
    let app = melo::daemon::app::test_router_with_settings(harness.settings.clone()).await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/open")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"target":"cover.jpg","mode":"path_file"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}
```

- [ ] **Step 2: Run the focused tests to verify they fail**

Run: `cargo test classify_target_rejects_images --lib -- --nocapture`  
Expected: FAIL because `domain::open` does not exist yet.

Run: `cargo test --test open_api open_endpoint_rejects_unsupported_file_types -- --nocapture`  
Expected: FAIL because `/api/open` and `test_router_with_settings()` do not exist yet.

- [ ] **Step 3: Implement path classification, discovery, and the daemon open endpoint**

```rust
// src/domain/open/service.rs
use std::path::{Path, PathBuf};

use crate::core::error::{MeloError, MeloResult};
use crate::core::model::player::PlayerSnapshot;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpenTarget {
    AudioFile(PathBuf),
    Directory(PathBuf),
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct OpenRequest {
    pub target: String,
    pub mode: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct OpenResponse {
    pub snapshot: PlayerSnapshot,
    pub playlist_name: String,
    pub source_label: String,
}

pub fn classify_target(path: &Path) -> MeloResult<OpenTarget> {
    if path.is_dir() {
        return Ok(OpenTarget::Directory(path.to_path_buf()));
    }

    match path.extension().and_then(|ext| ext.to_str()).map(|ext| ext.to_lowercase()) {
        Some(ext) if matches!(ext.as_str(), "flac" | "mp3" | "ogg" | "wav") => {
            Ok(OpenTarget::AudioFile(path.to_path_buf()))
        }
        _ => Err(MeloError::Message("unsupported_open_format".to_string())),
    }
}

pub fn discover_audio_paths(root: &Path, max_depth: usize) -> MeloResult<Vec<PathBuf>> {
    Ok(walkdir::WalkDir::new(root)
        .max_depth(max_depth + 1)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
        .filter_map(|entry| {
            let path = entry.into_path();
            let ext = path.extension()?.to_str()?.to_lowercase();
            matches!(ext.as_str(), "flac" | "mp3" | "ogg" | "wav").then_some(path)
        })
        .collect())
}

pub struct OpenService {
    settings: Settings,
    library: LibraryService,
    playlists: PlaylistService,
    player: Arc<PlayerService>,
}

impl OpenService {
    pub async fn open(&self, request: OpenRequest) -> MeloResult<OpenResponse> {
        let target = classify_target(Path::new(&request.target))?;
        let source_label = request.target.clone();
        let audio_paths = match target {
            OpenTarget::AudioFile(path) => vec![path],
            OpenTarget::Directory(path) => discover_audio_paths(&path, self.settings.open.max_depth)?,
        };

        let song_ids = self
            .library
            .ensure_scanned_paths(&audio_paths, self.settings.open.prewarm_limit)
            .await?;

        let visible = matches!(request.mode.as_str(), "path_dir" | "cwd_dir");
        let playlist = self
            .playlists
            .upsert_ephemeral(&request.target, &request.mode, &request.target, visible, None, &song_ids)
            .await?;

        self.player.clear().await?;
        for item in self.library.queue_items_for_song_ids(&song_ids).await? {
            self.player.append(item).await?;
        }
        let snapshot = self.player.play().await?;

        Ok(OpenResponse {
            snapshot,
            playlist_name: playlist.name,
            source_label,
        })
    }
}
```

```rust
// src/api/open.rs
use axum::{Json, extract::State, http::StatusCode};

use crate::daemon::app::AppState;
use crate::domain::open::service::OpenRequest;

pub async fn open(
    State(state): State<AppState>,
    Json(request): Json<OpenRequest>,
) -> Result<Json<crate::domain::open::service::OpenResponse>, (StatusCode, String)> {
    state
        .open_target(request)
        .await
        .map(Json)
        .map_err(|err| (StatusCode::BAD_REQUEST, err.to_string()))
}
```

```rust
// src/daemon/app.rs
#[derive(Clone)]
pub struct AppState {
    pub player: Arc<PlayerService>,
    pub settings: Settings,
    pub open: Arc<crate::domain::open::service::OpenService>,
}

pub async fn test_router_with_settings(settings: Settings) -> axum::Router {
    let state = AppState::for_test_with_settings(settings).await;
    crate::daemon::server::router(state)
}

impl AppState {
    pub async fn open_target(
        &self,
        request: crate::domain::open::service::OpenRequest,
    ) -> crate::core::error::MeloResult<crate::domain::open::service::OpenResponse> {
        self.open.open(request).await
    }
}
```

- [ ] **Step 4: Run the focused tests and verify they pass**

Run: `cargo test classify_target_rejects_images --lib -- --nocapture`  
Expected: PASS and images now return `unsupported_open_format`.

Run: `cargo test discover_audio_paths_respects_max_depth --lib -- --nocapture`  
Expected: PASS and directory discovery stops at the configured depth.

Run: `cargo test --test open_api open_endpoint_rejects_unsupported_file_types -- --nocapture`  
Expected: PASS and the daemon returns `400` for unsupported direct-open targets.

- [ ] **Step 5: Run project QA before committing**

Run: `pnpm qa`  
Expected: PASS including formatting, clippy, and the full test suite.

- [ ] **Step 6: Commit the direct-open service slice**

```bash
git add src/domain/mod.rs src/domain/library/repository.rs src/domain/library/service.rs src/domain/open/mod.rs src/domain/open/service.rs src/domain/open/service/tests.rs src/api/mod.rs src/api/open.rs src/daemon/app.rs src/daemon/server.rs tests/open_api.rs
git commit -m "feat: add daemon direct-open service"
```

---

### Task 4: Add daemon runtime helpers and child auto-start support

**Files:**
- Create: `src/daemon/process.rs`
- Create: `src/daemon/process/tests.rs`
- Modify: `src/daemon/mod.rs`
- Modify: `src/cli/client.rs`
- Modify: `src/cli/run.rs`

- [ ] **Step 1: Write the failing daemon-process helper tests**

```rust
// src/daemon/process/tests.rs
use std::path::PathBuf;

use crate::daemon::process::{daemon_bind_addr, daemon_command};

#[test]
fn daemon_bind_addr_uses_meolo_base_url_port() {
    let addr = daemon_bind_addr("http://127.0.0.1:38123").unwrap();
    assert_eq!(addr.port(), 38123);
}

#[test]
fn daemon_command_uses_current_exe_and_daemon_subcommand() {
    let command = daemon_command(PathBuf::from("melo.exe"), "http://127.0.0.1:38123");
    let args = command
        .get_args()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect::<Vec<_>>();

    assert_eq!(args, vec!["daemon".to_string()]);
}
```

- [ ] **Step 2: Run the focused tests to verify they fail**

Run: `cargo test daemon_bind_addr_uses_meolo_base_url_port --lib -- --nocapture`  
Expected: FAIL because `daemon::process` does not exist yet.

- [ ] **Step 3: Implement daemon bind parsing, child spawn, and health checks**

```rust
// src/daemon/process.rs
use std::net::SocketAddr;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Duration;

use crate::core::error::{MeloError, MeloResult};

pub fn daemon_bind_addr(base_url: &str) -> MeloResult<SocketAddr> {
    let url = reqwest::Url::parse(base_url).map_err(|err| MeloError::Message(err.to_string()))?;
    let host = url.host_str().unwrap_or("127.0.0.1");
    let port = url.port_or_known_default().unwrap_or(8080);
    format!("{host}:{port}")
        .parse()
        .map_err(|err: std::net::AddrParseError| MeloError::Message(err.to_string()))
}

pub fn daemon_command(current_exe: PathBuf, _base_url: &str) -> Command {
    let mut command = Command::new(current_exe);
    command.arg("daemon");
    command.stdin(Stdio::null());
    command.stdout(Stdio::null());
    command.stderr(Stdio::null());
    command
}

pub async fn ensure_running(base_url: &str) -> MeloResult<()> {
    let client = crate::cli::client::ApiClient::new(base_url.to_string());
    if client.health().await.is_ok() {
        return Ok(());
    }

    let current_exe = std::env::current_exe().map_err(|err| MeloError::Message(err.to_string()))?;
    daemon_command(current_exe, base_url)
        .spawn()
        .map_err(|err| MeloError::Message(err.to_string()))?;

    for _ in 0..20 {
        tokio::time::sleep(Duration::from_millis(150)).await;
        if client.health().await.is_ok() {
            return Ok(());
        }
    }

    Err(MeloError::Message("daemon failed to start".to_string()))
}
```

```rust
// src/cli/client.rs
pub async fn health(&self) -> MeloResult<()> {
    let url = format!("{}/api/system/health", self.base_url);
    self.client
        .get(url)
        .send()
        .await
        .map_err(|err| MeloError::Message(err.to_string()))?
        .error_for_status()
        .map_err(|err| MeloError::Message(err.to_string()))?;
    Ok(())
}
```

```rust
// src/cli/run.rs
Some(Command::Daemon) => {
    let base_url =
        std::env::var("MELO_BASE_URL").unwrap_or_else(|_| "http://127.0.0.1:8080".to_string());
    let listener = tokio::net::TcpListener::bind(crate::daemon::process::daemon_bind_addr(&base_url)?)
        .await
        .map_err(|err| crate::core::error::MeloError::Message(err.to_string()))?;
    let state = crate::daemon::app::AppState::new()?;
    axum::serve(listener, crate::daemon::server::router(state))
        .await
        .map_err(|err| crate::core::error::MeloError::Message(err.to_string()))?;
}
```

- [ ] **Step 4: Run the focused tests and verify they pass**

Run: `cargo test daemon_bind_addr_uses_meolo_base_url_port --lib -- --nocapture`  
Expected: PASS and the daemon bind helper respects the client base URL port.

Run: `cargo test daemon_command_uses_current_exe_and_daemon_subcommand --lib -- --nocapture`  
Expected: PASS and the child daemon always launches the same executable with the `daemon` subcommand.

- [ ] **Step 5: Run project QA before committing**

Run: `pnpm qa`  
Expected: PASS including formatting, clippy, and the full test suite.

- [ ] **Step 6: Commit the daemon runtime slice**

```bash
git add src/daemon/mod.rs src/daemon/process.rs src/daemon/process/tests.rs src/cli/client.rs src/cli/run.rs
git commit -m "feat: add daemon autostart helpers"
```

---

### Task 5: Add raw CLI dispatch and a real TUI runtime

**Files:**
- Create: `src/cli/dispatch.rs`
- Create: `src/cli/dispatch/tests.rs`
- Create: `src/tui/run.rs`
- Create: `src/tui/run/tests.rs`
- Modify: `src/cli/mod.rs`
- Modify: `src/cli/run.rs`
- Modify: `src/tui/app.rs`
- Modify: `src/tui/mod.rs`
- Modify: `tests/cli_help.rs`

- [ ] **Step 1: Write the failing dispatch and TUI tests**

```rust
// src/cli/dispatch/tests.rs
use std::ffi::OsString;

use crate::cli::dispatch::{dispatch_args, Dispatch};

#[test]
fn dispatch_without_args_uses_default_launch() {
    assert_eq!(dispatch_args(&[OsString::from("melo")]), Dispatch::DefaultLaunch);
}

#[test]
fn dispatch_with_audio_path_prefers_direct_open() {
    let dispatch = dispatch_args(&[OsString::from("melo"), OsString::from("song.flac")]);
    assert_eq!(dispatch, Dispatch::DirectOpen("song.flac".into()));
}

#[test]
fn dispatch_with_known_subcommand_stays_in_clap_mode() {
    let dispatch = dispatch_args(&[OsString::from("melo"), OsString::from("play")]);
    assert_eq!(dispatch, Dispatch::Clap);
}
```

```rust
// src/tui/run/tests.rs
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[test]
fn app_source_label_is_rendered_in_status_line() {
    let mut app = crate::tui::app::App::new_for_test();
    app.set_source_label("cwd:/music");

    assert!(app.footer_status().contains("cwd:/music"));
}

#[test]
fn quit_key_still_maps_to_quit_action() {
    let mut app = crate::tui::app::App::new_for_test();
    let action = app.handle_key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE));

    assert_eq!(action, Some(crate::tui::event::Action::Quit));
}
```

- [ ] **Step 2: Run the focused tests to verify they fail**

Run: `cargo test dispatch_without_args_uses_default_launch --lib -- --nocapture`  
Expected: FAIL because `cli::dispatch` does not exist yet.

Run: `cargo test app_source_label_is_rendered_in_status_line --lib -- --nocapture`  
Expected: FAIL because `App` does not yet track startup context text.

- [ ] **Step 3: Implement raw argv dispatch and the TUI runtime entry**

```rust
// src/cli/dispatch.rs
use std::ffi::OsString;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Dispatch {
    DefaultLaunch,
    DirectOpen(String),
    Clap,
}

pub fn dispatch_args(args: &[OsString]) -> Dispatch {
    let Some(first) = args.get(1).and_then(|value| value.to_str()) else {
        return Dispatch::DefaultLaunch;
    };

    if matches!(first, "play" | "pause" | "toggle" | "next" | "prev" | "stop" | "status" | "tui" | "daemon" | "player" | "library" | "queue" | "playlist" | "db" | "config" | "-h" | "--help" | "-V" | "--version") {
        return Dispatch::Clap;
    }

    Dispatch::DirectOpen(first.to_string())
}
```

```rust
// src/tui/app.rs
pub struct App {
    pub player: PlayerSnapshot,
    pub active_view: ActiveView,
    pub focus: FocusArea,
    pub source_label: Option<String>,
}

impl App {
    pub fn set_source_label(&mut self, source_label: impl Into<String>) {
        self.source_label = Some(source_label.into());
    }
}
```

```rust
// src/tui/run.rs
pub async fn start(base_url: String, source_label: Option<String>) -> crate::core::error::MeloResult<()> {
    use std::time::Duration;

    crossterm::terminal::enable_raw_mode()
        .map_err(|err| crate::core::error::MeloError::Message(err.to_string()))?;
    let mut stdout = std::io::stdout();
    crossterm::execute!(stdout, crossterm::terminal::EnterAlternateScreen)
        .map_err(|err| crate::core::error::MeloError::Message(err.to_string()))?;

    let backend = ratatui::backend::CrosstermBackend::new(stdout);
    let mut terminal = ratatui::Terminal::new(backend)
        .map_err(|err| crate::core::error::MeloError::Message(err.to_string()))?;

    let client = crate::cli::client::ApiClient::new(base_url.clone());
    let mut app = crate::tui::app::App::new_for_test();
    app.apply_snapshot(client.status().await?);
    if let Some(source_label) = source_label {
        app.set_source_label(source_label);
    }

    loop {
        terminal
            .draw(|frame| {
                let _ = app.layout(frame.area());
            })
            .map_err(|err| crate::core::error::MeloError::Message(err.to_string()))?;

        if crossterm::event::poll(Duration::from_millis(50))
            .map_err(|err| crate::core::error::MeloError::Message(err.to_string()))?
        {
            if let crossterm::event::Event::Key(key) =
                crossterm::event::read().map_err(|err| crate::core::error::MeloError::Message(err.to_string()))?
            {
                if matches!(app.handle_key(key), Some(crate::tui::event::Action::Quit)) {
                    break;
                }
            }
        }
    }

    crossterm::terminal::disable_raw_mode()
        .map_err(|err| crate::core::error::MeloError::Message(err.to_string()))?;
    Ok(())
}
```

```rust
// src/cli/run.rs
match crate::cli::dispatch::dispatch_args(&std::env::args_os().collect::<Vec<_>>()) {
    crate::cli::dispatch::Dispatch::DefaultLaunch => {
        let base_url =
            std::env::var("MELO_BASE_URL").unwrap_or_else(|_| "http://127.0.0.1:8080".to_string());
        crate::daemon::process::ensure_running(&base_url).await?;

        let settings = crate::core::config::settings::Settings::load()?;
        let source_label = if settings.open.scan_current_dir {
            let cwd = std::env::current_dir().map_err(|err| crate::core::error::MeloError::Message(err.to_string()))?;
            let response = crate::cli::client::ApiClient::new(base_url.clone())
                .open_target(cwd.to_string_lossy().to_string(), "cwd_dir")
                .await
                .ok();
            response.map(|opened| opened.source_label)
        } else {
            None
        };

        crate::tui::run::start(base_url, source_label).await?;
    }
    crate::cli::dispatch::Dispatch::DirectOpen(target) => {
        let base_url =
            std::env::var("MELO_BASE_URL").unwrap_or_else(|_| "http://127.0.0.1:8080".to_string());
        crate::daemon::process::ensure_running(&base_url).await?;
        let opened = crate::cli::client::ApiClient::new(base_url.clone())
            .open_target(target, "path_file")
            .await?;
        crate::tui::run::start(base_url, Some(opened.source_label)).await?;
    }
    crate::cli::dispatch::Dispatch::Clap => {
        // 继续走现有 Clap 子命令分发
    }
}
```

- [ ] **Step 4: Run the focused tests and verify they pass**

Run: `cargo test dispatch_without_args_uses_default_launch --lib -- --nocapture`  
Expected: PASS and raw argv dispatch now distinguishes no-arg launch from Clap subcommands.

Run: `cargo test app_source_label_is_rendered_in_status_line --lib -- --nocapture`  
Expected: PASS and the TUI footer can display the active open source.

- [ ] **Step 5: Run project QA before committing**

Run: `pnpm qa`  
Expected: PASS including formatting, clippy, and the full test suite.

- [ ] **Step 6: Commit the CLI/TUI entry slice**

```bash
git add src/cli/mod.rs src/cli/dispatch.rs src/cli/dispatch/tests.rs src/cli/run.rs src/tui/mod.rs src/tui/app.rs src/tui/run.rs src/tui/run/tests.rs tests/cli_help.rs
git commit -m "feat: add direct open cli dispatch and tui runtime"
```

---

### Task 6: Expose playlist cleanup/promotion commands, write `config.example.toml`, and close with end-to-end tests

**Files:**
- Create: `config.example.toml`
- Modify: `src/cli/args.rs`
- Modify: `src/cli/run.rs`
- Modify: `tests/cli_help.rs`
- Modify: `tests/cli_remote.rs`
- Modify: `tests/config_loading.rs`

- [ ] **Step 1: Write the failing help and example-config tests**

```rust
// append to tests/cli_help.rs
#[test]
fn playlist_help_mentions_promote_and_cleanup() {
    let mut cmd = Command::cargo_bin("melo").unwrap();
    cmd.arg("playlist").arg("--help");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("promote"))
        .stdout(predicate::str::contains("cleanup"));
}
```

```rust
// append to tests/config_loading.rs
#[test]
fn config_example_toml_parses_successfully() {
    let settings = Settings::load_from_path("config.example.toml").unwrap();
    assert_eq!(settings.player.volume, 100);
    assert_eq!(settings.playlists.ephemeral.default_ttl_seconds, 0);
}
```

- [ ] **Step 2: Run the focused tests to verify they fail**

Run: `cargo test --test cli_help playlist_help_mentions_promote_and_cleanup -- --nocapture`  
Expected: FAIL because the `playlist` namespace does not advertise the new maintenance commands yet.

Run: `cargo test --test config_loading config_example_toml_parses_successfully -- --nocapture`  
Expected: FAIL because `config.example.toml` does not exist yet.

- [ ] **Step 3: Implement playlist maintenance commands and the full example config**

```rust
// src/cli/args.rs
#[derive(Debug, Subcommand)]
pub enum PlaylistCommand {
    #[command(about = "Promote an ephemeral playlist into a visible static playlist")]
    Promote { source_key: String, new_name: String },
    #[command(about = "Delete expired ephemeral playlists")]
    Cleanup {
        #[arg(long, default_value_t = true)]
        expired: bool,
    },
}

// inside `pub enum Command`
Playlist {
    #[command(subcommand)]
    command: PlaylistCommand,
},
```

```rust
// src/cli/run.rs
Some(Command::Playlist {
    command: PlaylistCommand::Promote { source_key, new_name },
}) => {
    let settings = crate::core::config::settings::Settings::load()?;
    crate::domain::playlist::service::PlaylistService::new(settings)
        .promote_ephemeral(&source_key, &new_name)
        .await?;
    println!("{new_name}");
}
Some(Command::Playlist {
    command: PlaylistCommand::Cleanup { expired: true },
}) => {
    let settings = crate::core::config::settings::Settings::load()?;
    let deleted = crate::domain::playlist::service::PlaylistService::new(settings)
        .cleanup_expired(&crate::core::db::now_text())
        .await?;
    println!("{deleted}");
}
```

```toml
# config.example.toml
# Melo 示例配置。复制为 config.toml 或通过 MELO_CONFIG 指定。

[database]
# SQLite 数据库文件路径。
path = "local/melo.db"

[player]
# 无恢复会话时的默认音量。
volume = 100
# 是否恢复上一次 daemon 会话。
restore_last_session = true
# 恢复后是否自动继续播放。
resume_after_restore = false

[open]
# 裸 `melo` 是否先尝试扫描当前目录。
scan_current_dir = true
# 目录型打开的递归深度。
max_depth = 2
# 进入 TUI 前同步预热的曲目数。
prewarm_limit = 20
# 后台补扫并发度。
background_jobs = 4

[playlists.ephemeral]
# 默认 TTL，单位为秒。`0` 表示永不过期。
default_ttl_seconds = 0

[playlists.ephemeral.visibility]
# 单文件临时歌单是否出现在常规列表中。
path_file = false
# 目录临时歌单是否出现在常规列表中。
path_dir = true
# 当前目录临时歌单是否出现在常规列表中。
cwd_dir = true

[library.organize]
# 文件整理的基础目录。
base_dir = "D:/Library"
# 冲突处理策略。
conflict_policy = "first_match"

[[library.organize.rules]]
name = "default"
priority = 0
template = "{{ artist|sanitize }}/{{ album|default('Unknown Album')|sanitize }}/{{ title|sanitize }}"

[playlists.smart.aimer]
query = "artist:\"Aimer\""
description = "All songs whose artist contains Aimer"
```

- [ ] **Step 4: Run the focused tests and verify they pass**

Run: `cargo test --test cli_help playlist_help_mentions_promote_and_cleanup -- --nocapture`  
Expected: PASS and the `playlist` namespace now advertises the new maintenance commands.

Run: `cargo test --test config_loading config_example_toml_parses_successfully -- --nocapture`  
Expected: PASS and the checked-in example config parses through the same settings loader as runtime code.

- [ ] **Step 5: Run final project QA before committing**

Run: `pnpm qa`  
Expected: PASS including formatting, clippy, and the full test suite.

- [ ] **Step 6: Commit the documentation and CLI finishing slice**

```bash
git add src/cli/args.rs src/cli/run.rs tests/cli_help.rs tests/cli_remote.rs tests/config_loading.rs config.example.toml
git commit -m "feat: add direct open config and playlist maintenance commands"
```

---

## Self-review notes

### Spec coverage

- 顶层 `melo` / `melo <path>` 入口：Task 4 + Task 5
- 临时播放列表数据模型与来源复用：Task 1 + Task 2
- 目录深度扫描、支持格式、图片报错：Task 3
- 默认不过期与 TTL 秒配置：Task 1 + Task 6
- 真实 TUI 落地与来源标签：Task 5
- `config.example.toml` 全覆盖：Task 6
- 清理过期数据与转正式歌单：Task 2 + Task 6

### Placeholder scan

- 没有遗留占位标记
- 每个任务都包含具体测试、命令、文件路径与提交信息
- 没有写“按需处理错误”这类空泛步骤

### Type consistency

- TTL 配置统一使用 `default_ttl_seconds`
- 临时歌单统一使用 `kind = "ephemeral"`
- 直接打开响应统一使用 `OpenResponse`
- CLI 预分发统一使用 `Dispatch`

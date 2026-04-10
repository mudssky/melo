# Melo Playback Session Recovery Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Persist the active playback session so Melo can restore queue, current index, and last known position after the daemon restarts.

**Architecture:** Store one active player session plus ordered queue items in SQLite using SeaORM migrations/entities. `PlayerService` exports and restores domain-friendly session snapshots, while `AppState` owns the save/load orchestration so playback code stays independent from database details.

**Tech Stack:** Rust 2024, SeaORM 1.1, SeaORM Migration, Tokio watch tasks, existing SQLite bootstrap helpers, existing player snapshot/session model

---

## File structure impact

### Existing files to modify

- Modify: `src/core/db/entities/mod.rs`
- Modify: `src/core/db/migrations/mod.rs`
- Modify: `src/core/db/migrator.rs`
- Modify: `src/domain/player/mod.rs`
- Modify: `src/domain/player/service.rs`
- Modify: `src/daemon/app.rs`
- Modify: `tests/db_bootstrap.rs`

### New files to create

- Create: `src/core/db/entities/player_sessions.rs`
- Create: `src/core/db/entities/player_session_items.rs`
- Create: `src/core/db/migrations/m20260410_000002_player_session.rs`
- Create: `src/domain/player/session_store.rs`
- Create: `src/domain/player/session_store/tests.rs`
- Create: `tests/player_session_recovery.rs`

### Responsibilities

- `src/core/db/migrations/m20260410_000002_player_session.rs`
  Define the persistent schema for one active session and its ordered queue items
- `src/core/db/entities/player_sessions.rs`
  SeaORM model for the session header row
- `src/core/db/entities/player_session_items.rs`
  SeaORM model for persisted queue items
- `src/domain/player/session_store.rs`
  Save/load/debounce logic for persisted sessions
- `src/domain/player/service.rs`
  Export and restore domain sessions without database knowledge
- `src/daemon/app.rs`
  Load persisted session on startup and spawn the save loop

---

### Task 1: Add SQLite schema and session store round-trip coverage

**Files:**
- Create: `src/core/db/entities/player_sessions.rs`
- Create: `src/core/db/entities/player_session_items.rs`
- Create: `src/core/db/migrations/m20260410_000002_player_session.rs`
- Create: `src/domain/player/session_store.rs`
- Create: `src/domain/player/session_store/tests.rs`
- Modify: `src/core/db/entities/mod.rs`
- Modify: `src/core/db/migrations/mod.rs`
- Modify: `src/core/db/migrator.rs`
- Modify: `src/domain/player/mod.rs`
- Modify: `tests/db_bootstrap.rs`

- [ ] **Step 1: Write the failing migration and store round-trip tests**

```rust
// append to tests/db_bootstrap.rs
assert!(tables.contains(&"player_sessions".to_string()));
assert!(tables.contains(&"player_session_items".to_string()));
```

```rust
// src/domain/player/session_store/tests.rs
use tempfile::tempdir;

use crate::core::config::settings::Settings;
use crate::core::db::bootstrap::DatabaseBootstrap;
use crate::core::db::connection::connect;
use crate::core::model::player::{PlaybackState, QueueItem};
use crate::domain::player::session_store::{PersistedPlayerSession, PlayerSessionStore};

fn item(song_id: i64, title: &str) -> QueueItem {
    QueueItem {
        song_id,
        path: format!("tests/fixtures/{title}.mp3"),
        title: title.to_string(),
        duration_seconds: Some(212.0),
    }
}

#[tokio::test]
async fn session_store_round_trips_queue_index_and_position() {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("melo.db");
    let settings = Settings::for_test(db_path.clone());
    DatabaseBootstrap::new(&settings).init().await.unwrap();
    let db = connect(&settings).await.unwrap();
    let store = PlayerSessionStore::new(db);

    let session = PersistedPlayerSession {
        playback_state: PlaybackState::Stopped,
        queue_index: Some(1),
        position_seconds: Some(48.5),
        queue: vec![item(1, "One"), item(2, "Two")],
    };

    store.save(&session).await.unwrap();
    let restored = store.load().await.unwrap().unwrap();

    assert_eq!(restored.playback_state, PlaybackState::Stopped);
    assert_eq!(restored.queue_index, Some(1));
    assert_eq!(restored.position_seconds, Some(48.5));
    assert_eq!(restored.queue.len(), 2);
    assert_eq!(restored.queue[1].title, "Two");
}

#[tokio::test]
async fn position_only_changes_under_one_second_do_not_force_write() {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("melo.db");
    let settings = Settings::for_test(db_path.clone());
    DatabaseBootstrap::new(&settings).init().await.unwrap();
    let db = connect(&settings).await.unwrap();
    let store = PlayerSessionStore::new(db);

    let before = PersistedPlayerSession {
        playback_state: PlaybackState::Playing,
        queue_index: Some(0),
        position_seconds: Some(10.0),
        queue: vec![item(1, "One")],
    };
    let after = PersistedPlayerSession {
        playback_state: PlaybackState::Playing,
        queue_index: Some(0),
        position_seconds: Some(10.4),
        queue: vec![item(1, "One")],
    };

    assert!(!store.should_persist(Some(&before), &after));
}
```

- [ ] **Step 2: Run the focused tests to verify they fail**

Run: `cargo test session_store_round_trips_queue_index_and_position --lib -- --nocapture`  
Expected: FAIL because `PlayerSessionStore`, `PersistedPlayerSession`, and the new tables do not exist yet.

Run: `cargo test --test db_bootstrap db_init_runs_seaorm_migrations -- --nocapture`  
Expected: FAIL because the migration list does not yet create `player_sessions` or `player_session_items`.

- [ ] **Step 3: Implement the migration, entities, and persistence store**

```rust
// src/core/db/entities/player_sessions.rs
use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "player_sessions")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub playback_state: String,
    pub queue_index: Option<i64>,
    pub position_seconds: Option<f64>,
    pub updated_at: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
```

```rust
// src/core/db/entities/player_session_items.rs
use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "player_session_items")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub session_id: i64,
    pub position: i64,
    pub song_id: i64,
    pub path: String,
    pub title: String,
    pub duration_seconds: Option<f64>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::player_sessions::Entity",
        from = "Column::SessionId",
        to = "super::player_sessions::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    Session,
}

impl ActiveModelBehavior for ActiveModel {}
```

```rust
// src/core/db/migrations/m20260410_000002_player_session.rs
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .if_not_exists()
                    .table(PlayerSessions::Table)
                    .col(pk_auto(PlayerSessions::Id))
                    .col(string(PlayerSessions::PlaybackState).not_null())
                    .col(big_integer(PlayerSessions::QueueIndex))
                    .col(double(PlayerSessions::PositionSeconds))
                    .col(string(PlayerSessions::UpdatedAt).not_null())
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .if_not_exists()
                    .table(PlayerSessionItems::Table)
                    .col(pk_auto(PlayerSessionItems::Id))
                    .col(big_integer(PlayerSessionItems::SessionId).not_null())
                    .col(big_integer(PlayerSessionItems::Position).not_null())
                    .col(big_integer(PlayerSessionItems::SongId).not_null())
                    .col(string(PlayerSessionItems::Path).not_null())
                    .col(string(PlayerSessionItems::Title).not_null())
                    .col(double(PlayerSessionItems::DurationSeconds))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_player_session_items_session")
                            .from(PlayerSessionItems::Table, PlayerSessionItems::SessionId)
                            .to(PlayerSessions::Table, PlayerSessions::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_player_session_items_session_position")
                    .table(PlayerSessionItems::Table)
                    .col(PlayerSessionItems::SessionId)
                    .col(PlayerSessionItems::Position)
                    .to_owned(),
            )
            .await?;
        Ok(())
    }
}
```

```rust
// src/domain/player/session_store.rs
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set};

use crate::core::error::{MeloError, MeloResult};
use crate::core::model::player::{PlaybackState, QueueItem};

#[derive(Debug, Clone, PartialEq)]
pub struct PersistedPlayerSession {
    pub playback_state: PlaybackState,
    pub queue_index: Option<usize>,
    pub position_seconds: Option<f64>,
    pub queue: Vec<QueueItem>,
}

#[derive(Clone)]
pub struct PlayerSessionStore {
    db: DatabaseConnection,
}

impl PlayerSessionStore {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub fn should_persist(
        &self,
        previous: Option<&PersistedPlayerSession>,
        current: &PersistedPlayerSession,
    ) -> bool {
        let Some(previous) = previous else {
            return true;
        };

        previous.playback_state != current.playback_state
            || previous.queue_index != current.queue_index
            || previous.queue != current.queue
            || match (previous.position_seconds, current.position_seconds) {
                (Some(a), Some(b)) => (a - b).abs() >= 1.0,
                (None, None) => false,
                _ => true,
            }
    }

    pub async fn save(&self, session: &PersistedPlayerSession) -> MeloResult<()> {
        use crate::core::db::entities::{player_session_items, player_sessions};

        player_session_items::Entity::delete_many()
            .exec(&self.db)
            .await
            .map_err(|err| MeloError::Message(err.to_string()))?;
        player_sessions::Entity::delete_many()
            .exec(&self.db)
            .await
            .map_err(|err| MeloError::Message(err.to_string()))?;

        let header = player_sessions::ActiveModel {
            playback_state: Set(session.playback_state.as_str().to_string()),
            queue_index: Set(session.queue_index.map(|value| value as i64)),
            position_seconds: Set(session.position_seconds),
            updated_at: Set("2026-04-10T00:00:00Z".to_string()),
            ..Default::default()
        }
        .insert(&self.db)
        .await
        .map_err(|err| MeloError::Message(err.to_string()))?;

        for (position, item) in session.queue.iter().enumerate() {
            player_session_items::ActiveModel {
                session_id: Set(header.id),
                position: Set(position as i64),
                song_id: Set(item.song_id),
                path: Set(item.path.clone()),
                title: Set(item.title.clone()),
                duration_seconds: Set(item.duration_seconds),
                ..Default::default()
            }
            .insert(&self.db)
            .await
            .map_err(|err| MeloError::Message(err.to_string()))?;
        }

        Ok(())
    }

    pub async fn load(&self) -> MeloResult<Option<PersistedPlayerSession>> {
        use crate::core::db::entities::{player_session_items, player_sessions};

        let Some(header) = player_sessions::Entity::find()
            .one(&self.db)
            .await
            .map_err(|err| MeloError::Message(err.to_string()))?
        else {
            return Ok(None);
        };

        let items = player_session_items::Entity::find()
            .filter(player_session_items::Column::SessionId.eq(header.id))
            .order_by_asc(player_session_items::Column::Position)
            .all(&self.db)
            .await
            .map_err(|err| MeloError::Message(err.to_string()))?;

        let playback_state = match header.playback_state.as_str() {
            "idle" => PlaybackState::Idle,
            "playing" => PlaybackState::Playing,
            "paused" => PlaybackState::Paused,
            "stopped" => PlaybackState::Stopped,
            "error" => PlaybackState::Error,
            _ => PlaybackState::Idle,
        };

        Ok(Some(PersistedPlayerSession {
            playback_state,
            queue_index: header.queue_index.map(|value| value as usize),
            position_seconds: header.position_seconds,
            queue: items
                .into_iter()
                .map(|item| QueueItem {
                    song_id: item.song_id,
                    path: item.path,
                    title: item.title,
                    duration_seconds: item.duration_seconds,
                })
                .collect(),
        }))
    }
}

#[cfg(test)]
mod tests;
```

- [ ] **Step 4: Run the focused tests and verify they pass**

Run: `cargo test session_store_round_trips_queue_index_and_position --lib -- --nocapture`  
Expected: PASS and the store round-trips queue order, index, and position.

Run: `cargo test --test db_bootstrap db_init_runs_seaorm_migrations -- --nocapture`  
Expected: PASS and the bootstrap test sees the two new tables.

- [ ] **Step 5: Run project QA before committing**

Run: `pnpm qa`  
Expected: PASS including formatting, clippy, and the full test suite.

- [ ] **Step 6: Commit the storage foundation slice**

```bash
git add src/core/db/entities/mod.rs src/core/db/entities/player_sessions.rs src/core/db/entities/player_session_items.rs src/core/db/migrations/mod.rs src/core/db/migrations/m20260410_000002_player_session.rs src/core/db/migrator.rs src/domain/player/mod.rs src/domain/player/session_store.rs src/domain/player/session_store/tests.rs tests/db_bootstrap.rs
git commit -m "feat: persist player sessions in sqlite"
```

---

### Task 2: Restore persisted sessions on startup and debounce session saves

**Files:**
- Modify: `src/domain/player/service.rs`
- Modify: `src/daemon/app.rs`
- Create: `tests/player_session_recovery.rs`

- [ ] **Step 1: Write the failing restore and startup tests**

```rust
// append to src/domain/player/service/tests.rs
use crate::domain::player::session_store::PersistedPlayerSession;

#[tokio::test]
async fn restore_persisted_playing_session_downgrades_to_stopped() {
    let service = PlayerService::new(Arc::new(FakeBackend::default()));
    let snapshot = service
        .restore_persisted_session(PersistedPlayerSession {
            playback_state: PlaybackState::Playing,
            queue_index: Some(0),
            position_seconds: Some(48.0),
            queue: vec![item(1, "One")],
        })
        .await
        .unwrap();

    assert_eq!(snapshot.playback_state, PlaybackState::Stopped.as_str());
    assert_eq!(snapshot.queue_index, Some(0));
    assert_eq!(snapshot.position_seconds, Some(48.0));
}
```

```rust
// tests/player_session_recovery.rs
use std::sync::Arc;

use tempfile::tempdir;

use melo::core::config::settings::Settings;
use melo::core::db::bootstrap::DatabaseBootstrap;
use melo::core::db::connection::connect;
use melo::domain::player::backend::NoopBackend;
use melo::domain::player::session_store::{PersistedPlayerSession, PlayerSessionStore};

#[tokio::test]
async fn app_state_restores_last_session_from_store() {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("melo.db");
    let settings = Settings::for_test(db_path.clone());
    DatabaseBootstrap::new(&settings).init().await.unwrap();
    let db = connect(&settings).await.unwrap();
    let store = Arc::new(PlayerSessionStore::new(db));

    store
        .save(&PersistedPlayerSession {
            playback_state: melo::core::model::player::PlaybackState::Playing,
            queue_index: Some(0),
            position_seconds: Some(12.0),
            queue: vec![melo::core::model::player::QueueItem {
                song_id: 1,
                path: "tests/fixtures/full_test.mp3".into(),
                title: "Blue Bird".into(),
                duration_seconds: Some(212.0),
            }],
        })
        .await
        .unwrap();

    let state = melo::daemon::app::AppState::with_backend_and_session_store(
        Arc::new(NoopBackend),
        store,
    )
    .await
    .unwrap();

    let snapshot = state.player.snapshot().await;
    assert_eq!(snapshot.playback_state, "stopped");
    assert_eq!(snapshot.queue_len, 1);
    assert_eq!(snapshot.position_seconds, Some(12.0));
}
```

- [ ] **Step 2: Run the focused tests to verify they fail**

Run: `cargo test restore_persisted_playing_session_downgrades_to_stopped --lib -- --nocapture`  
Expected: FAIL because `restore_persisted_session()` does not exist yet.

Run: `cargo test --test player_session_recovery app_state_restores_last_session_from_store -- --nocapture`  
Expected: FAIL because `AppState::with_backend_and_session_store()` and the startup restore path do not exist yet.

- [ ] **Step 3: Implement service export/restore and the app-owned save loop**

```rust
// src/domain/player/service.rs
use crate::domain::player::session_store::PersistedPlayerSession;

impl PlayerService {
    pub async fn export_persisted_session(&self) -> PersistedPlayerSession {
        let session = self.session.lock().await;
        PersistedPlayerSession {
            playback_state: session.playback_state,
            queue_index: session.queue.current_index(),
            position_seconds: session.position_seconds,
            queue: session.queue.items().to_vec(),
        }
    }

    pub async fn restore_persisted_session(
        &self,
        persisted: PersistedPlayerSession,
    ) -> MeloResult<PlayerSnapshot> {
        let mut session = self.session.lock().await;
        session.queue = PlayerQueue::from_items(persisted.queue, persisted.queue_index);
        session.position_seconds = persisted.position_seconds;
        session.last_error = None;
        session.playback_state = match persisted.playback_state {
            PlaybackState::Playing | PlaybackState::Paused => PlaybackState::Stopped,
            other => other,
        };
        self.publish_locked(&mut session)
    }
}
```

```rust
// src/daemon/app.rs
use crate::domain::player::session_store::PlayerSessionStore;

impl AppState {
    pub async fn with_backend_and_session_store(
        backend: Arc<dyn PlaybackBackend>,
        session_store: Arc<PlayerSessionStore>,
    ) -> MeloResult<Self> {
        let player = Arc::new(PlayerService::new(backend));
        player.start_runtime_event_loop();
        player.start_progress_loop();

        if let Some(persisted) = session_store.load().await? {
            let _ = player.restore_persisted_session(persisted).await?;
        }

        let state = Self {
            player: Arc::clone(&player),
        };
        state.spawn_session_save_loop(session_store);
        Ok(state)
    }

    fn spawn_session_save_loop(&self, session_store: Arc<PlayerSessionStore>) {
        let player = Arc::clone(&self.player);
        tokio::spawn(async move {
            let mut receiver = player.subscribe();
            let mut last_saved = None;
            loop {
                if receiver.changed().await.is_err() {
                    break;
                }

                let current = player.export_persisted_session().await;
                if session_store.should_persist(last_saved.as_ref(), &current) {
                    if session_store.save(&current).await.is_ok() {
                        last_saved = Some(current);
                    }
                }
            }
        });
    }
}
```

- [ ] **Step 4: Run the focused tests and verify they pass**

Run: `cargo test restore_persisted_playing_session_downgrades_to_stopped --lib -- --nocapture`  
Expected: PASS and restored `playing` sessions now downgrade to `stopped`.

Run: `cargo test --test player_session_recovery app_state_restores_last_session_from_store -- --nocapture`  
Expected: PASS and app startup now restores the last persisted session.

- [ ] **Step 5: Run project QA before committing**

Run: `pnpm qa`  
Expected: PASS including formatting, clippy, and the full test suite.

- [ ] **Step 6: Commit the startup restore slice**

```bash
git add src/domain/player/service.rs src/daemon/app.rs tests/player_session_recovery.rs
git commit -m "feat: restore player sessions on startup"
```

---

## Self-review notes

### Spec coverage

- SQLite 会话表与队列项表：Task 1
- round-trip、节流判定、迁移覆盖：Task 1
- service 导出 / 恢复接口：Task 2
- 启动恢复与后台保存循环：Task 2

### Placeholder scan

- 没有遗留占位式描述
- 任务包含具体表名、实体文件、测试代码、命令和提交信息

### Type consistency

- 会话持久化类型统一使用 `PersistedPlayerSession`
- 表名统一使用 `player_sessions` / `player_session_items`
- app 启动入口统一使用 `with_backend_and_session_store()`

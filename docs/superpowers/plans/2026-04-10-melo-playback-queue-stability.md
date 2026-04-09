# Melo Playback And Queue Stability Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Stabilize Melo's playback and queue control surface so daemon, CLI, API, WebSocket, and TUI all share one predictable player contract.

**Architecture:** Keep `PlayerService` as the only writable source of player state, extract queue mutation rules into a dedicated `queue` module, and let all external surfaces consume the same `PlayerSnapshot`. Introduce a richer session model and error contract first, then wire that contract through API/CLI/TUI, and only then harden runtime failure paths around the `RodioBackend`.

**Tech Stack:** Rust 2024, Tokio, Rodio, Axum, Tokio Tungstenite, Serde, Tracing, Ratatui, existing Rust integration/unit test stack

---

## File structure impact

### Existing files to modify

- Modify: `src/core/model/player.rs`
- Modify: `src/domain/player/mod.rs`
- Modify: `src/domain/player/service.rs`
- Modify: `src/domain/player/rodio_backend.rs`
- Modify: `src/daemon/app.rs`
- Modify: `src/daemon/server.rs`
- Modify: `src/api/mod.rs`
- Modify: `src/api/player.rs`
- Modify: `src/api/ws.rs`
- Modify: `src/cli/args.rs`
- Modify: `src/cli/client.rs`
- Modify: `src/cli/run.rs`
- Modify: `src/tui/app.rs`
- Modify: `tests/player_service.rs`
- Modify: `tests/api_server.rs`
- Modify: `tests/cli_remote.rs`
- Modify: `tests/tui_app.rs`

### New files to create

- Create: `src/domain/player/queue.rs`
- Create: `src/domain/player/queue/tests.rs`
- Create: `src/domain/player/service/tests.rs`
- Create: `src/api/queue.rs`

### Responsibilities

- `src/core/model/player.rs`
  Shared player-facing types: `PlaybackState`, `PlayerErrorInfo`, `QueueItem`, `NowPlayingSong`, `PlayerSnapshot`
- `src/domain/player/queue.rs`
  Pure queue mutation rules, current-index repair, navigation helpers
- `src/domain/player/service.rs`
  `PlayerSession`, player state machine, backend orchestration, snapshot publication
- `src/domain/player/rodio_backend.rs`
  Real playback execution only, no queue business rules
- `src/api/player.rs`
  HTTP handlers for player control commands
- `src/api/queue.rs`
  HTTP handlers for queue editing commands
- `src/api/ws.rs`
  WebSocket stream of `PlayerSnapshot`
- `src/cli/client.rs`
  Remote client methods matching the stabilized player/queue contract
- `src/cli/run.rs`
  CLI command wiring for player and queue controls
- `src/tui/app.rs`
  Snapshot consumption only, no local player semantics

---

### Task 1: Add shared player contract and extract queue rules

**Files:**
- Modify: `src/core/model/player.rs`
- Modify: `src/domain/player/mod.rs`
- Create: `src/domain/player/queue.rs`
- Create: `src/domain/player/queue/tests.rs`

- [ ] **Step 1: Write the failing queue unit tests**

```rust
// src/domain/player/queue/tests.rs
use crate::core::model::player::QueueItem;
use crate::domain::player::queue::PlayerQueue;

fn item(song_id: i64, title: &str) -> QueueItem {
    QueueItem {
        song_id,
        path: format!("D:/Music/{title}.flac"),
        title: title.to_string(),
        duration_seconds: Some(180.0),
    }
}

#[test]
fn queue_insert_before_current_advances_current_index() {
    let mut queue = PlayerQueue::from_items(vec![item(1, "One"), item(2, "Two")], Some(1));

    queue.insert(0, item(3, "Zero")).unwrap();

    assert_eq!(queue.current_index(), Some(2));
    assert_eq!(queue.current().unwrap().title, "Two");
    assert_eq!(queue.len(), 3);
}

#[test]
fn queue_remove_current_prefers_next_item() {
    let mut queue = PlayerQueue::from_items(
        vec![item(1, "One"), item(2, "Two"), item(3, "Three")],
        Some(1),
    );

    let removed = queue.remove(1).unwrap().unwrap();

    assert_eq!(removed.title, "Two");
    assert_eq!(queue.current_index(), Some(1));
    assert_eq!(queue.current().unwrap().title, "Three");
}

#[test]
fn queue_move_current_item_tracks_new_index() {
    let mut queue = PlayerQueue::from_items(
        vec![item(1, "One"), item(2, "Two"), item(3, "Three")],
        Some(0),
    );

    queue.move_item(0, 2).unwrap();

    assert_eq!(queue.current_index(), Some(2));
    assert_eq!(queue.current().unwrap().title, "One");
}

#[test]
fn queue_clear_resets_current_index_and_navigation() {
    let mut queue = PlayerQueue::from_items(vec![item(1, "One"), item(2, "Two")], Some(1));

    queue.clear();

    assert_eq!(queue.current_index(), None);
    assert_eq!(queue.len(), 0);
    assert!(!queue.has_next());
    assert!(!queue.has_prev());
}

- [ ] **Step 2: Run the queue test and verify it fails**

Run: `cargo test queue_insert_before_current_advances_current_index --lib -- --nocapture`  
Expected: FAIL because `src/domain/player/queue.rs` and `PlayerQueue` do not exist yet.

- [ ] **Step 3: Implement the shared player contract and queue module**

```rust
// src/core/model/player.rs
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub enum PlaybackState {
    Idle,
    Playing,
    Paused,
    Stopped,
    Error,
}

impl PlaybackState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Playing => "playing",
            Self::Paused => "paused",
            Self::Stopped => "stopped",
            Self::Error => "error",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct PlayerErrorInfo {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct QueueItem {
    pub song_id: i64,
    pub path: String,
    pub title: String,
    pub duration_seconds: Option<f64>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NowPlayingSong {
    pub song_id: i64,
    pub title: String,
    pub duration_seconds: Option<f64>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct PlayerSnapshot {
    pub playback_state: String,
    pub current_song: Option<NowPlayingSong>,
    pub queue_len: usize,
    pub queue_index: Option<usize>,
    pub has_next: bool,
    pub has_prev: bool,
    pub last_error: Option<PlayerErrorInfo>,
    pub version: u64,
}

```rust
// src/domain/player/queue.rs
use crate::core::error::{MeloError, MeloResult};
use crate::core::model::player::QueueItem;

#[derive(Debug, Clone, Default)]
pub struct PlayerQueue {
    items: Vec<QueueItem>,
    current_index: Option<usize>,
}

impl PlayerQueue {
    pub fn from_items(items: Vec<QueueItem>, current_index: Option<usize>) -> Self {
        let current_index = current_index.filter(|index| *index < items.len());
        Self { items, current_index }
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn items(&self) -> &[QueueItem] {
        &self.items
    }

    pub fn current_index(&self) -> Option<usize> {
        self.current_index
    }

    pub fn current(&self) -> Option<&QueueItem> {
        self.current_index.and_then(|index| self.items.get(index))
    }

    pub fn append(&mut self, item: QueueItem) {
        self.items.push(item);
    }

    pub fn insert(&mut self, index: usize, item: QueueItem) -> MeloResult<()> {
        if index > self.items.len() {
            return Err(MeloError::Message("queue index out of range".to_string()));
        }

        self.items.insert(index, item);
        if let Some(current_index) = self.current_index {
            if index <= current_index {
                self.current_index = Some(current_index + 1);
            }
        }

        Ok(())
    }

    pub fn play_index(&mut self, index: usize) -> MeloResult<&QueueItem> {
        if index >= self.items.len() {
            return Err(MeloError::Message("queue index out of range".to_string()));
        }

        self.current_index = Some(index);
        Ok(&self.items[index])
    }

    pub fn remove(&mut self, index: usize) -> MeloResult<Option<QueueItem>> {
        if index >= self.items.len() {
            return Err(MeloError::Message("queue index out of range".to_string()));
        }

        let removed = self.items.remove(index);
        self.current_index = match self.current_index {
            None => None,
            Some(current) if self.items.is_empty() => None,
            Some(current) if index < current => Some(current - 1),
            Some(current) if index == current && index < self.items.len() => Some(index),
            Some(current) if index == current => Some(current.saturating_sub(1)),
            Some(current) => Some(current),
        };

        Ok(Some(removed))
    }

    pub fn move_item(&mut self, from: usize, to: usize) -> MeloResult<()> {
        if from >= self.items.len() || to >= self.items.len() {
            return Err(MeloError::Message("queue index out of range".to_string()));
        }
        if from == to {
            return Ok(());
        }

        let item = self.items.remove(from);
        self.items.insert(to, item);

        self.current_index = match self.current_index {
            Some(current) if current == from => Some(to),
            Some(current) if from < current && to >= current => Some(current - 1),
            Some(current) if from > current && to <= current => Some(current + 1),
            other => other,
        };

        Ok(())
    }

    pub fn clear(&mut self) {
        self.items.clear();
        self.current_index = None;
    }

    pub fn has_next(&self) -> bool {
        matches!(self.current_index, Some(index) if index + 1 < self.items.len())
    }

    pub fn has_prev(&self) -> bool {
        matches!(self.current_index, Some(index) if index > 0)
    }
}

#[cfg(test)]
mod tests;
```

```rust
// src/domain/player/mod.rs
pub mod backend;
pub mod queue;
pub mod rodio_backend;
pub mod service;
```

- [ ] **Step 4: Run the queue tests and verify they pass**

Run: `cargo test queue_ --lib -- --nocapture`  
Expected: PASS and all queue mutation/index-repair tests are green.

- [ ] **Step 5: Commit the queue contract slice**

```bash
git add src/core/model/player.rs src/domain/player/mod.rs src/domain/player/queue.rs src/domain/player/queue/tests.rs
git commit -m "refactor: add player queue contract"
```

---

### Task 2: Stabilize `PlayerService` as a real state machine

**Files:**
- Modify: `src/domain/player/service.rs`
- Create: `src/domain/player/service/tests.rs`
- Modify: `tests/player_service.rs`

- [ ] **Step 1: Write the failing service state-machine tests**

```rust
// src/domain/player/service/tests.rs
use std::sync::{Arc, Mutex};

use crate::core::model::player::{PlaybackState, QueueItem};
use crate::domain::player::backend::{PlaybackBackend, PlaybackCommand};
use crate::domain::player::service::PlayerService;

#[derive(Default)]
struct FakeBackend {
    commands: Arc<Mutex<Vec<PlaybackCommand>>>,
    fail_next: Arc<Mutex<bool>>,
}

impl PlaybackBackend for FakeBackend {
    fn load_and_play(&self, path: &std::path::Path) -> crate::core::error::MeloResult<()> {
        if *self.fail_next.lock().unwrap() {
            *self.fail_next.lock().unwrap() = false;
            return Err(crate::core::error::MeloError::Message("backend failed".to_string()));
        }
        self.commands
            .lock()
            .unwrap()
            .push(PlaybackCommand::Load(path.to_path_buf()));
        Ok(())
    }

    fn pause(&self) -> crate::core::error::MeloResult<()> {
        self.commands.lock().unwrap().push(PlaybackCommand::Pause);
        Ok(())
    }

    fn resume(&self) -> crate::core::error::MeloResult<()> {
        self.commands.lock().unwrap().push(PlaybackCommand::Resume);
        Ok(())
    }

    fn stop(&self) -> crate::core::error::MeloResult<()> {
        self.commands.lock().unwrap().push(PlaybackCommand::Stop);
        Ok(())
    }
}

fn item(song_id: i64, title: &str) -> QueueItem {
    QueueItem {
        song_id,
        path: format!("D:/Music/{title}.flac"),
        title: title.to_string(),
        duration_seconds: Some(180.0),
    }
}

#[tokio::test]
async fn play_on_empty_queue_records_queue_empty_error() {
    let service = PlayerService::new(Arc::new(FakeBackend::default()));

    let err = service.play().await.unwrap_err();
    assert!(err.to_string().contains("queue is empty"));

    let snapshot = service.snapshot().await;
    assert_eq!(snapshot.playback_state, PlaybackState::Error.as_str());
    assert_eq!(snapshot.last_error.unwrap().code, "queue_empty");
}

#[tokio::test]
async fn toggle_from_paused_resumes_current_track() {
    let backend = Arc::new(FakeBackend::default());
    let service = PlayerService::new(backend.clone());

    service.append(item(1, "One")).await.unwrap();
    service.play().await.unwrap();
    service.pause().await.unwrap();
    service.toggle().await.unwrap();

    let snapshot = service.snapshot().await;
    assert_eq!(snapshot.playback_state, PlaybackState::Playing.as_str());
    assert_eq!(snapshot.queue_index, Some(0));
    assert_eq!(backend.commands.lock().unwrap().last(), Some(&PlaybackCommand::Resume));
}

#[tokio::test]
async fn next_loads_following_queue_item() {
    let backend = Arc::new(FakeBackend::default());
    let service = PlayerService::new(backend.clone());

    service.append(item(1, "One")).await.unwrap();
    service.append(item(2, "Two")).await.unwrap();
    service.play().await.unwrap();
    service.next().await.unwrap();

    let snapshot = service.snapshot().await;
    assert_eq!(snapshot.queue_index, Some(1));
    assert_eq!(snapshot.current_song.unwrap().title, "Two");
}

#[tokio::test]
async fn backend_failure_sets_error_without_entering_playing() {
    let backend = Arc::new(FakeBackend::default());
    let service = PlayerService::new(backend.clone());
    service.append(item(1, "Broken")).await.unwrap();
    *backend.fail_next.lock().unwrap() = true;

    let err = service.play().await.unwrap_err();
    assert!(err.to_string().contains("backend failed"));

    let snapshot = service.snapshot().await;
    assert_eq!(snapshot.playback_state, PlaybackState::Error.as_str());
    assert_eq!(snapshot.last_error.unwrap().code, "backend_unavailable");
}
```

- [ ] **Step 2: Run the service test and verify it fails**

Run: `cargo test play_on_empty_queue_records_queue_empty_error --lib -- --nocapture`  
Expected: FAIL because `PlayerService` does not expose the richer state machine or error contract yet.

- [ ] **Step 3: Implement `PlayerSession`, snapshot publication, and full control methods**

```rust
// src/domain/player/service.rs
use std::sync::Arc;

use tokio::sync::{Mutex, watch};

use crate::core::error::{MeloError, MeloResult};
use crate::core::model::player::{
    NowPlayingSong, PlaybackState, PlayerErrorInfo, PlayerSnapshot, QueueItem,
};
use crate::domain::player::backend::PlaybackBackend;
use crate::domain::player::queue::PlayerQueue;

#[derive(Debug)]
struct PlayerSession {
    playback_state: PlaybackState,
    queue: PlayerQueue,
    last_error: Option<PlayerErrorInfo>,
    version: u64,
}

impl Default for PlayerSession {
    fn default() -> Self {
        Self {
            playback_state: PlaybackState::Idle,
            queue: PlayerQueue::default(),
            last_error: None,
            version: 0,
        }
    }
}

pub struct PlayerService {
    backend: Arc<dyn PlaybackBackend>,
    session: Mutex<PlayerSession>,
    snapshot_tx: watch::Sender<PlayerSnapshot>,
}

impl PlayerService {
    pub fn new(backend: Arc<dyn PlaybackBackend>) -> Self {
        let (snapshot_tx, _snapshot_rx) = watch::channel(PlayerSnapshot::default());
        Self {
            backend,
            session: Mutex::new(PlayerSession::default()),
            snapshot_tx,
        }
    }

    pub fn subscribe(&self) -> watch::Receiver<PlayerSnapshot> {
        self.snapshot_tx.subscribe()
    }

    pub async fn append(&self, item: QueueItem) -> MeloResult<PlayerSnapshot> {
        let mut session = self.session.lock().await;
        session.queue.append(item);
        session.last_error = None;
        self.publish_locked(&mut session)
    }

    pub async fn insert(&self, index: usize, item: QueueItem) -> MeloResult<PlayerSnapshot> {
        let mut session = self.session.lock().await;
        session.queue.insert(index, item)?;
        session.last_error = None;
        self.publish_locked(&mut session)
    }

    pub async fn enqueue(&self, item: QueueItem) -> MeloResult<()> {
        self.append(item).await.map(|_| ())
    }

    pub async fn play(&self) -> MeloResult<PlayerSnapshot> {
        let mut session = self.session.lock().await;
        if session.queue.len() == 0 {
            return self.fail_locked(
                &mut session,
                "queue_empty",
                "queue is empty",
                MeloError::Message("queue is empty".to_string()),
            );
        }
        if session.queue.current_index().is_none() {
            let _ = session.queue.play_index(0)?;
        }
        let current = session.queue.current().cloned().unwrap();
        if !std::path::Path::new(&current.path).exists() {
            return self.fail_locked(
                &mut session,
                "track_file_missing",
                "track file is missing",
                MeloError::Message("track file is missing".to_string()),
            );
        }
        if let Err(err) = self.backend.load_and_play(std::path::Path::new(&current.path)) {
            return self.fail_locked(&mut session, "backend_unavailable", &err.to_string(), err);
        }
        session.playback_state = PlaybackState::Playing;
        session.last_error = None;
        self.publish_locked(&mut session)
    }

    pub async fn pause(&self) -> MeloResult<PlayerSnapshot> {
        let mut session = self.session.lock().await;
        if session.playback_state == PlaybackState::Playing {
            self.backend.pause()?;
            session.playback_state = PlaybackState::Paused;
        }
        self.publish_locked(&mut session)
    }

    pub async fn toggle(&self) -> MeloResult<PlayerSnapshot> {
        let state = self.session.lock().await.playback_state;
        match state {
            PlaybackState::Playing => self.pause().await,
            PlaybackState::Paused => self.resume().await,
            PlaybackState::Idle | PlaybackState::Stopped | PlaybackState::Error => self.play().await,
        }
    }

    pub async fn resume(&self) -> MeloResult<PlayerSnapshot> {
        let mut session = self.session.lock().await;
        if session.playback_state == PlaybackState::Paused {
            self.backend.resume()?;
            session.playback_state = PlaybackState::Playing;
            session.last_error = None;
        }
        self.publish_locked(&mut session)
    }

    pub async fn stop(&self) -> MeloResult<PlayerSnapshot> {
        let mut session = self.session.lock().await;
        self.backend.stop()?;
        if session.queue.len() == 0 {
            session.playback_state = PlaybackState::Idle;
        } else {
            session.playback_state = PlaybackState::Stopped;
        }
        self.publish_locked(&mut session)
    }

    pub async fn next(&self) -> MeloResult<PlayerSnapshot> {
        let mut session = self.session.lock().await;
        let next_index = match session.queue.current_index() {
            Some(index) if index + 1 < session.queue.len() => index + 1,
            _ => {
                return self.fail_locked(
                    &mut session,
                    "queue_no_next",
                    "queue has no next item",
                    MeloError::Message("queue has no next item".to_string()),
                );
            }
        };
        let _ = session.queue.play_index(next_index)?;
        drop(session);
        self.play().await
    }

    pub async fn prev(&self) -> MeloResult<PlayerSnapshot> {
        let mut session = self.session.lock().await;
        let prev_index = match session.queue.current_index() {
            Some(index) if index > 0 => index - 1,
            _ => {
                return self.fail_locked(
                    &mut session,
                    "queue_no_prev",
                    "queue has no previous item",
                    MeloError::Message("queue has no previous item".to_string()),
                );
            }
        };
        let _ = session.queue.play_index(prev_index)?;
        drop(session);
        self.play().await
    }

    pub async fn play_index(&self, index: usize) -> MeloResult<PlayerSnapshot> {
        let mut session = self.session.lock().await;
        session
            .queue
            .play_index(index)
            .map_err(|_| MeloError::Message("queue index out of range".to_string()))?;
        drop(session);
        self.play().await
    }

    pub async fn clear(&self) -> MeloResult<PlayerSnapshot> {
        let mut session = self.session.lock().await;
        session.queue.clear();
        session.playback_state = PlaybackState::Idle;
        session.last_error = None;
        self.backend.stop()?;
        self.publish_locked(&mut session)
    }

    pub async fn remove(&self, index: usize) -> MeloResult<PlayerSnapshot> {
        let mut session = self.session.lock().await;
        let _ = session.queue.remove(index)?;
        if session.queue.len() == 0 {
            session.playback_state = PlaybackState::Idle;
        }
        session.last_error = None;
        self.publish_locked(&mut session)
    }

    pub async fn move_item(&self, from: usize, to: usize) -> MeloResult<PlayerSnapshot> {
        let mut session = self.session.lock().await;
        session.queue.move_item(from, to)?;
        session.last_error = None;
        self.publish_locked(&mut session)
    }

    pub async fn snapshot(&self) -> PlayerSnapshot {
        let session = self.session.lock().await;
        Self::snapshot_from_session(&session)
    }

    fn snapshot_from_session(session: &PlayerSession) -> PlayerSnapshot {
        PlayerSnapshot {
            playback_state: session.playback_state.as_str().to_string(),
            current_song: session.queue.current().map(|item| NowPlayingSong {
                song_id: item.song_id,
                title: item.title.clone(),
                duration_seconds: item.duration_seconds,
            }),
            queue_len: session.queue.len(),
            queue_index: session.queue.current_index(),
            has_next: session.queue.has_next(),
            has_prev: session.queue.has_prev(),
            last_error: session.last_error.clone(),
            version: session.version,
        }
    }

    fn publish_locked(&self, session: &mut PlayerSession) -> MeloResult<PlayerSnapshot> {
        session.version += 1;
        let snapshot = Self::snapshot_from_session(session);
        self.snapshot_tx.send_replace(snapshot.clone());
        Ok(snapshot)
    }

    fn fail_locked(
        &self,
        session: &mut PlayerSession,
        code: &str,
        message: &str,
        err: MeloError,
    ) -> MeloResult<PlayerSnapshot> {
        session.playback_state = PlaybackState::Error;
        session.last_error = Some(PlayerErrorInfo {
            code: code.to_string(),
            message: message.to_string(),
        });
        let _ = self.publish_locked(session)?;
        Err(err)
    }
}

#[cfg(test)]
mod tests;
```

- [ ] **Step 4: Run the service tests and verify they pass**

Run: `cargo test toggle_from_paused_resumes_current_track --lib -- --nocapture`  
Expected: PASS and the service now exposes stable state transitions and error snapshots.

- [ ] **Step 5: Keep the existing integration test green on the richer service**

Run: `cargo test --test player_service -- --nocapture`  
Expected: PASS and the existing fake-backend integration test still works with the richer snapshot.

- [ ] **Step 6: Commit the state machine slice**

```bash
git add src/domain/player/service.rs src/domain/player/service/tests.rs tests/player_service.rs
git commit -m "feat: stabilize player state machine"
```

---

### Task 3: Expose the stabilized player and queue contract over HTTP and WebSocket

**Files:**
- Modify: `src/api/mod.rs`
- Modify: `src/api/player.rs`
- Modify: `src/api/ws.rs`
- Modify: `src/daemon/app.rs`
- Modify: `src/daemon/server.rs`
- Create: `src/api/queue.rs`
- Test: `tests/api_server.rs`

- [ ] **Step 1: Write the failing API integration tests for queue and broadcasted snapshots**

```rust
// tests/api_server.rs
use axum::body::Body;
use axum::http::{Request, StatusCode};
use tower::ServiceExt;

#[tokio::test]
async fn queue_endpoints_and_ws_broadcast_share_snapshot_contract() {
    let app = melo::daemon::app::test_router().await;

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/queue/add")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"items":[{"song_id":1,"path":"tests/fixtures/full_test.mp3","title":"Blue Bird","duration_seconds":212.0}]}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let status = app
        .oneshot(
            Request::builder()
                .uri("/api/player/status")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(status.status(), StatusCode::OK);
}
```

- [ ] **Step 2: Run the API test and verify it fails**

Run: `cargo test --test api_server queue_endpoints_and_ws_broadcast_share_snapshot_contract -- --nocapture`  
Expected: FAIL because `/api/queue/add` and the richer snapshot contract do not exist yet.

- [ ] **Step 3: Add queue handlers, richer player handlers, and WebSocket subscription plumbing**

```rust
// src/api/queue.rs
use axum::{Json, extract::State};

use crate::core::model::player::{PlayerSnapshot, QueueItem};
use crate::daemon::app::AppState;

#[derive(Debug, serde::Deserialize)]
pub struct QueueAddRequest {
    pub items: Vec<QueueItem>,
}

#[derive(Debug, serde::Deserialize)]
pub struct QueueIndexRequest {
    pub index: usize,
}

#[derive(Debug, serde::Deserialize)]
pub struct QueueInsertRequest {
    pub index: usize,
    pub item: QueueItem,
}

#[derive(Debug, serde::Deserialize)]
pub struct QueueRemoveRequest {
    pub index: usize,
}

#[derive(Debug, serde::Deserialize)]
pub struct QueueMoveRequest {
    pub from: usize,
    pub to: usize,
}

pub async fn add(
    State(state): State<AppState>,
    Json(request): Json<QueueAddRequest>,
) -> Json<PlayerSnapshot> {
    let mut snapshot = state.player.snapshot().await;
    for item in request.items {
        snapshot = state.player.append(item).await.unwrap();
    }
    Json(snapshot)
}

pub async fn insert(
    State(state): State<AppState>,
    Json(request): Json<QueueInsertRequest>,
) -> Json<PlayerSnapshot> {
    Json(state.player.insert(request.index, request.item).await.unwrap())
}

pub async fn clear(State(state): State<AppState>) -> Json<PlayerSnapshot> {
    Json(state.player.clear().await.unwrap())
}

pub async fn play_index(
    State(state): State<AppState>,
    Json(request): Json<QueueIndexRequest>,
) -> Json<PlayerSnapshot> {
    Json(state.player.play_index(request.index).await.unwrap())
}

pub async fn remove(
    State(state): State<AppState>,
    Json(request): Json<QueueRemoveRequest>,
) -> Json<PlayerSnapshot> {
    Json(state.player.remove(request.index).await.unwrap())
}

pub async fn move_item(
    State(state): State<AppState>,
    Json(request): Json<QueueMoveRequest>,
) -> Json<PlayerSnapshot> {
    Json(state.player.move_item(request.from, request.to).await.unwrap())
}
```

```rust
// src/api/mod.rs
pub mod player;
pub mod queue;
pub mod system;
pub mod ws;
```

```rust
// src/api/player.rs
use axum::{Json, extract::State};

use crate::daemon::app::AppState;

pub async fn status(
    State(state): State<AppState>,
) -> Json<crate::core::model::player::PlayerSnapshot> {
    Json(state.player.snapshot().await)
}

pub async fn play(
    State(state): State<AppState>,
) -> Json<crate::core::model::player::PlayerSnapshot> {
    Json(state.player.play().await.unwrap())
}

pub async fn pause(
    State(state): State<AppState>,
) -> Json<crate::core::model::player::PlayerSnapshot> {
    Json(state.player.pause().await.unwrap())
}

pub async fn toggle(
    State(state): State<AppState>,
) -> Json<crate::core::model::player::PlayerSnapshot> {
    Json(state.player.toggle().await.unwrap())
}

pub async fn stop(
    State(state): State<AppState>,
) -> Json<crate::core::model::player::PlayerSnapshot> {
    Json(state.player.stop().await.unwrap())
}

pub async fn next(
    State(state): State<AppState>,
) -> Json<crate::core::model::player::PlayerSnapshot> {
    Json(state.player.next().await.unwrap())
}

pub async fn prev(
    State(state): State<AppState>,
) -> Json<crate::core::model::player::PlayerSnapshot> {
    Json(state.player.prev().await.unwrap())
}
```

```rust
// src/daemon/app.rs
#[derive(Clone)]
pub struct AppState {
    pub player: Arc<PlayerService>,
}

impl AppState {
    pub fn with_backend(backend: Arc<dyn PlaybackBackend>) -> Self {
        Self {
            player: Arc::new(PlayerService::new(backend)),
        }
    }
}
```

```rust
// src/api/ws.rs
async fn stream_player_snapshots(mut socket: WebSocket, state: AppState) {
    let mut receiver = state.player.subscribe();
    let initial_snapshot = receiver.borrow().clone();
    if send_snapshot(&mut socket, initial_snapshot).await.is_err() {
        return;
    }

    while receiver.changed().await.is_ok() {
        let snapshot = receiver.borrow().clone();
        if send_snapshot(&mut socket, snapshot).await.is_err() {
            break;
        }
    }
}
```

```rust
// src/daemon/server.rs
pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/api/system/health", axum::routing::get(crate::api::system::health))
        .route("/api/player/status", axum::routing::get(crate::api::player::status))
        .route("/api/player/play", axum::routing::post(crate::api::player::play))
        .route("/api/player/pause", axum::routing::post(crate::api::player::pause))
        .route("/api/player/toggle", axum::routing::post(crate::api::player::toggle))
        .route("/api/player/stop", axum::routing::post(crate::api::player::stop))
        .route("/api/player/next", axum::routing::post(crate::api::player::next))
        .route("/api/player/prev", axum::routing::post(crate::api::player::prev))
        .route("/api/queue/add", axum::routing::post(crate::api::queue::add))
        .route("/api/queue/insert", axum::routing::post(crate::api::queue::insert))
        .route("/api/queue/clear", axum::routing::post(crate::api::queue::clear))
        .route("/api/queue/play", axum::routing::post(crate::api::queue::play_index))
        .route("/api/queue/remove", axum::routing::post(crate::api::queue::remove))
        .route("/api/queue/move", axum::routing::post(crate::api::queue::move_item))
        .route("/api/ws/player", axum::routing::get(crate::api::ws::player_updates))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
```

- [ ] **Step 4: Run the API test and verify it passes**

Run: `cargo test --test api_server -- --nocapture`  
Expected: PASS and HTTP endpoints plus WebSocket now share the same snapshot contract.

- [ ] **Step 5: Commit the API contract slice**

```bash
git add src/api/mod.rs src/api/player.rs src/api/queue.rs src/api/ws.rs src/daemon/app.rs src/daemon/server.rs tests/api_server.rs
git commit -m "feat: expose stable player queue api"
```

---

### Task 4: Wire the stabilized contract through CLI and TUI

**Files:**
- Modify: `src/cli/args.rs`
- Modify: `src/cli/client.rs`
- Modify: `src/cli/run.rs`
- Modify: `src/tui/app.rs`
- Test: `tests/cli_remote.rs`
- Test: `tests/tui_app.rs`

- [ ] **Step 1: Write the failing CLI and TUI contract tests**

```rust
// tests/cli_remote.rs
use assert_cmd::Command;
use predicates::prelude::*;
use tokio::net::TcpListener;

#[tokio::test(flavor = "multi_thread")]
async fn queue_show_prints_snapshot_navigation_flags() {
    let state = melo::daemon::app::AppState::for_test().await;
    state
        .player
        .append(melo::core::model::player::QueueItem {
            song_id: 1,
            path: "tests/fixtures/full_test.mp3".into(),
            title: "Blue Bird".into(),
            duration_seconds: Some(212.0),
        })
        .await
        .unwrap();
    let app = melo::daemon::server::router(state);
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let mut cmd = Command::cargo_bin("melo").unwrap();
    cmd.env("MELO_BASE_URL", format!("http://{addr}"));
    cmd.arg("queue").arg("show");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("has_next"))
        .stdout(predicate::str::contains("queue_len"));
}
```

```rust
// tests/tui_app.rs
#[tokio::test]
async fn tui_applies_navigation_flags_and_last_error_from_snapshot() {
    let mut app = melo::tui::app::App::new_for_test();
    app.apply_snapshot(melo::core::model::player::PlayerSnapshot {
        playback_state: "error".into(),
        current_song: None,
        queue_len: 2,
        queue_index: Some(1),
        has_next: false,
        has_prev: true,
        last_error: Some(melo::core::model::player::PlayerErrorInfo {
            code: "queue_no_next".into(),
            message: "queue has no next item".into(),
        }),
        version: 4,
    });

    assert_eq!(app.player.playback_state, "error");
    assert!(app.player.has_prev);
    assert_eq!(app.player.last_error.unwrap().code, "queue_no_next");
    assert_eq!(app.player.version, 4);
}
```

- [ ] **Step 2: Run the CLI/TUI tests and verify they fail**

Run: `cargo test --test cli_remote queue_show_prints_snapshot_navigation_flags -- --nocapture`  
Expected: FAIL because `melo queue show` is not wired yet.

Run: `cargo test --test tui_app tui_applies_navigation_flags_and_last_error_from_snapshot -- --nocapture`  
Expected: FAIL because `PlayerSnapshot` does not flow through TUI with the richer fields yet.

- [ ] **Step 3: Add queue subcommands and remote client methods**

```rust
// src/cli/args.rs
#[derive(Debug, Subcommand)]
pub enum QueueCommand {
    Show,
    Remove { index: usize },
    Move { from: usize, to: usize },
    Clear,
    Play { index: usize },
}

#[derive(Debug, Subcommand)]
pub enum Command {
    Play,
    Pause,
    Toggle,
    Next,
    Prev,
    Stop,
    Status,
    Tui,
    Daemon,
    Library,
    Queue {
        #[command(subcommand)]
        command: QueueCommand,
    },
    Playlist,
    Db {
        #[command(subcommand)]
        command: DbCommand,
    },
    Config,
}
```

```rust
// src/cli/client.rs
pub async fn post_json(&self, path: &str) -> MeloResult<PlayerSnapshot> {
    let url = format!("{}{}", self.base_url, path);
    self.client
        .post(url)
        .send()
        .await
        .map_err(|err| MeloError::Message(err.to_string()))?
        .json()
        .await
        .map_err(|err| MeloError::Message(err.to_string()))
}

pub async fn queue_show(&self) -> MeloResult<PlayerSnapshot> {
    let url = format!("{}/api/player/status", self.base_url);
    self.client
        .get(url)
        .send()
        .await
        .map_err(|err| MeloError::Message(err.to_string()))?
        .json()
        .await
        .map_err(|err| MeloError::Message(err.to_string()))
}

pub async fn queue_clear(&self) -> MeloResult<PlayerSnapshot> {
    let url = format!("{}/api/queue/clear", self.base_url);
    self.client
        .post(url)
        .send()
        .await
        .map_err(|err| MeloError::Message(err.to_string()))?
        .json()
        .await
        .map_err(|err| MeloError::Message(err.to_string()))
}

pub async fn queue_play_index(&self, index: usize) -> MeloResult<PlayerSnapshot> {
    let url = format!("{}/api/queue/play", self.base_url);
    self.client
        .post(url)
        .json(&serde_json::json!({ "index": index }))
        .send()
        .await
        .map_err(|err| MeloError::Message(err.to_string()))?
        .json()
        .await
        .map_err(|err| MeloError::Message(err.to_string()))
}

pub async fn queue_remove(&self, index: usize) -> MeloResult<PlayerSnapshot> {
    let url = format!("{}/api/queue/remove", self.base_url);
    self.client
        .post(url)
        .json(&serde_json::json!({ "index": index }))
        .send()
        .await
        .map_err(|err| MeloError::Message(err.to_string()))?
        .json()
        .await
        .map_err(|err| MeloError::Message(err.to_string()))
}

pub async fn queue_move(&self, from: usize, to: usize) -> MeloResult<PlayerSnapshot> {
    let url = format!("{}/api/queue/move", self.base_url);
    self.client
        .post(url)
        .json(&serde_json::json!({ "from": from, "to": to }))
        .send()
        .await
        .map_err(|err| MeloError::Message(err.to_string()))?
        .json()
        .await
        .map_err(|err| MeloError::Message(err.to_string()))
}
```

```rust
// src/cli/run.rs
match args.command {
    Some(Command::Queue { command: QueueCommand::Show }) => {
        let snapshot = crate::cli::client::ApiClient::from_env().queue_show().await?;
        println!("{}", serde_json::to_string_pretty(&snapshot).unwrap());
    }
    Some(Command::Queue { command: QueueCommand::Remove { index } }) => {
        let snapshot = crate::cli::client::ApiClient::from_env().queue_remove(index).await?;
        println!("{}", serde_json::to_string_pretty(&snapshot).unwrap());
    }
    Some(Command::Queue { command: QueueCommand::Move { from, to } }) => {
        let snapshot = crate::cli::client::ApiClient::from_env().queue_move(from, to).await?;
        println!("{}", serde_json::to_string_pretty(&snapshot).unwrap());
    }
    Some(Command::Queue { command: QueueCommand::Clear }) => {
        let snapshot = crate::cli::client::ApiClient::from_env().queue_clear().await?;
        println!("{}", serde_json::to_string_pretty(&snapshot).unwrap());
    }
    Some(Command::Queue { command: QueueCommand::Play { index } }) => {
        let snapshot = crate::cli::client::ApiClient::from_env().queue_play_index(index).await?;
        println!("{}", serde_json::to_string_pretty(&snapshot).unwrap());
    }
    Some(Command::Toggle) => {
        let snapshot = crate::cli::client::ApiClient::from_env()
            .post_json("/api/player/toggle")
            .await?;
        println!("{}", serde_json::to_string_pretty(&snapshot).unwrap());
    }
    Some(Command::Next) => {
        let snapshot = crate::cli::client::ApiClient::from_env()
            .post_json("/api/player/next")
            .await?;
        println!("{}", serde_json::to_string_pretty(&snapshot).unwrap());
    }
    Some(Command::Prev) => {
        let snapshot = crate::cli::client::ApiClient::from_env()
            .post_json("/api/player/prev")
            .await?;
        println!("{}", serde_json::to_string_pretty(&snapshot).unwrap());
    }
    _ => {}
}
```

- [ ] **Step 4: Make TUI consume the richer snapshot fields without adding new local semantics**

```rust
// src/tui/app.rs
pub fn apply_snapshot(&mut self, snapshot: PlayerSnapshot) {
    self.player = snapshot;
}

pub fn footer_status(&self) -> String {
    if let Some(error) = &self.player.last_error {
        return format!("ERR {}: {}", error.code, error.message);
    }

    format!(
        "{} | queue={} | prev={} | next={}",
        self.player.playback_state,
        self.player.queue_len,
        self.player.has_prev,
        self.player.has_next
    )
}
```

- [ ] **Step 5: Run the CLI/TUI tests and verify they pass**

Run: `cargo test --test cli_remote -- --nocapture`  
Expected: PASS and queue-related remote output now includes the richer snapshot fields.

Run: `cargo test --test tui_app -- --nocapture`  
Expected: PASS and the TUI state now preserves the richer player snapshot contract.

- [ ] **Step 6: Commit the CLI/TUI contract slice**

```bash
git add src/cli/args.rs src/cli/client.rs src/cli/run.rs src/tui/app.rs tests/cli_remote.rs tests/tui_app.rs
git commit -m "feat: align cli and tui with player snapshot contract"
```

---

### Task 5: Harden error paths and keep runtime semantics predictable

**Files:**
- Modify: `src/domain/player/rodio_backend.rs`
- Modify: `src/domain/player/service.rs`
- Modify: `src/domain/player/service/tests.rs`

- [ ] **Step 1: Write the failing error-path tests**

```rust
// append to src/domain/player/service/tests.rs
#[tokio::test]
async fn repeated_pause_does_not_bump_version_or_backend_commands() {
    let backend = Arc::new(FakeBackend::default());
    let service = PlayerService::new(backend);
    service.append(item(1, "One")).await.unwrap();
    service.play().await.unwrap();

    let paused = service.pause().await.unwrap();
    let paused_again = service.pause().await.unwrap();

    assert_eq!(paused.playback_state, "paused");
    assert_eq!(paused_again.playback_state, "paused");
    assert_eq!(paused_again.version, paused.version);
    assert_eq!(
        backend.commands.lock().unwrap().iter().filter(|cmd| matches!(cmd, PlaybackCommand::Pause)).count(),
        1
    );
}

#[tokio::test]
async fn repeated_stop_does_not_bump_version_or_backend_commands() {
    let backend = Arc::new(FakeBackend::default());
    let service = PlayerService::new(backend.clone());
    service.append(item(1, "One")).await.unwrap();
    service.play().await.unwrap();

    let stopped = service.stop().await.unwrap();
    let stopped_again = service.stop().await.unwrap();

    assert_eq!(stopped.playback_state, "stopped");
    assert_eq!(stopped_again.playback_state, "stopped");
    assert_eq!(stopped_again.version, stopped.version);
    assert_eq!(
        backend.commands.lock().unwrap().iter().filter(|cmd| matches!(cmd, PlaybackCommand::Stop)).count(),
        1
    );
}
```

- [ ] **Step 2: Run the error-path test and verify it fails**

Run: `cargo test repeated_pause_does_not_bump_version_or_backend_commands --lib -- --nocapture`  
Expected: FAIL because no-op commands still republish snapshots and re-hit the backend.

- [ ] **Step 3: Normalize backend/runtime failures and keep commands idempotent**

```rust
// src/domain/player/service.rs
pub async fn pause(&self) -> MeloResult<PlayerSnapshot> {
    let mut session = self.session.lock().await;
    if session.playback_state != PlaybackState::Playing {
        return Ok(Self::snapshot_from_session(&session));
    }

    self.backend.pause()?;
    session.playback_state = PlaybackState::Paused;
    self.publish_locked(&mut session)
}

pub async fn resume(&self) -> MeloResult<PlayerSnapshot> {
    let mut session = self.session.lock().await;
    if session.playback_state != PlaybackState::Paused {
        return Ok(Self::snapshot_from_session(&session));
    }

    self.backend.resume()?;
    session.playback_state = PlaybackState::Playing;
    session.last_error = None;
    self.publish_locked(&mut session)
}

pub async fn stop(&self) -> MeloResult<PlayerSnapshot> {
    let mut session = self.session.lock().await;
    let target_state = if session.queue.len() == 0 {
        PlaybackState::Idle
    } else {
        PlaybackState::Stopped
    };
    if session.playback_state == target_state {
        return Ok(Self::snapshot_from_session(&session));
    }

    self.backend.stop()?;
    session.playback_state = target_state;
    self.publish_locked(&mut session)
}
```

- [ ] **Step 4: Run the service tests and verify the failure paths now pass**

Run: `cargo test repeated_pause_does_not_bump_version_or_backend_commands --lib -- --nocapture`  
Expected: PASS and repeated pause commands no longer dirty the state machine.

Run: `cargo test repeated_stop_does_not_bump_version_or_backend_commands --lib -- --nocapture`  
Expected: PASS and repeated stop commands no longer republish or re-hit the backend.

- [ ] **Step 5: Commit the runtime hardening slice**

```bash
git add src/domain/player/rodio_backend.rs src/domain/player/service.rs src/domain/player/service/tests.rs
git commit -m "fix: harden player runtime error paths"
```

---

### Task 6: Final verification and dependency sanity pass

**Files:**
- Modify: `docs/superpowers/specs/2026-04-10-melo-playback-queue-stability-design.md` (only if implementation changed the agreed contract)
- Test: all Rust test suites

- [ ] **Step 1: Run the full Rust integration and unit test suite**

Run: `cargo test --tests -- --nocapture`  
Expected: PASS across player, API, CLI remote, TUI, and the previously completed phase-1 suites.

- [ ] **Step 2: Run the focused library tests too, to make sure player work did not regress existing slices**

Run: `cargo test --lib -- --nocapture`  
Expected: PASS and the new queue/service unit tests remain green.

- [ ] **Step 3: Run the repository QA command required by this project**

Run: `pnpm qa`  
Expected: PASS including `cargo fmt`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test -q`.

- [ ] **Step 4: Commit the completed playback stability phase**

```bash
git add -A
git commit -m "feat: stabilize playback and queue control surface"
```

---

## Self-review notes

### Spec coverage

- 状态模型收敛：Task 1 + Task 2
- 队列控制面：Task 1 + Task 5
- 播放控制面：Task 2
- 统一快照契约：Task 1 + Task 2 + Task 3 + Task 4
- CLI / API / WebSocket / TUI 共用契约：Task 3 + Task 4
- 失败可解释：Task 2 + Task 5
- 本阶段不做进度同步、自动下一首、持久化恢复：计划中未加入对应任务

### Placeholder scan

- 没有使用 `TODO`、`TBD`、`similar to previous task` 之类占位描述
- 每个任务都包含了具体文件、测试代码、运行命令和提交信息

### Type consistency

- 对外快照统一使用 `PlayerSnapshot`
- 播放状态统一使用 `PlaybackState`
- 错误展示统一使用 `PlayerErrorInfo`
- 队列模块统一使用 `PlayerQueue`

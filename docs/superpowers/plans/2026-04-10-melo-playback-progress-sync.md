# Melo Playback Progress Sync Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a stable playback-position contract so `PlayerSnapshot`, HTTP, WebSocket, CLI, and TUI all report the same progress state.

**Architecture:** Extend `PlayerSnapshot` and `PlayerSession` with progress fields while keeping `PlayerService` as the only state writer. `PlaybackBackend` exposes read-only playback position, `RodioBackend` maps that to `rodio::Player::get_pos()`, and a service-owned Tokio interval publishes throttled progress updates only when the value really changes.

**Tech Stack:** Rust 2024, Tokio interval/time control, Rodio 0.22 `Player::get_pos()`, Axum WebSocket, Ratatui playbar rendering, existing Rust unit/integration test stack

---

## File structure impact

### Existing files to modify

- Modify: `src/core/model/player.rs`
- Modify: `src/domain/player/backend.rs`
- Modify: `src/domain/player/rodio_backend.rs`
- Modify: `src/domain/player/service.rs`
- Modify: `src/domain/player/service/tests.rs`
- Modify: `src/daemon/app.rs`
- Modify: `src/tui/ui/playbar.rs`
- Modify: `tests/api_server.rs`
- Modify: `tests/cli_remote.rs`
- Modify: `tests/tui_app.rs`

### Responsibilities

- `src/core/model/player.rs`
  Add progress fields to the shared snapshot contract
- `src/domain/player/backend.rs`
  Expose current playback position as a backend capability
- `src/domain/player/service.rs`
  Own the progress ticker, throttling, and final progress publication on pause/stop/track switch
- `src/tui/ui/playbar.rs`
  Render position and duration from snapshot only

---

### Task 1: Extend the snapshot contract and add throttled progress publication

**Files:**
- Modify: `src/core/model/player.rs`
- Modify: `src/domain/player/backend.rs`
- Modify: `src/domain/player/rodio_backend.rs`
- Modify: `src/domain/player/service.rs`
- Modify: `src/domain/player/service/tests.rs`
- Modify: `src/daemon/app.rs`

- [ ] **Step 1: Write the failing service tests for progress updates**

```rust
// append to src/domain/player/service/tests.rs
use std::time::Duration;

#[derive(Clone)]
struct FakeBackend {
    commands: Arc<Mutex<Vec<PlaybackCommand>>>,
    fail_next: Arc<Mutex<bool>>,
    runtime_tx: broadcast::Sender<PlaybackRuntimeEvent>,
    current_position: Arc<Mutex<Option<Duration>>>,
}

impl Default for FakeBackend {
    fn default() -> Self {
        let (runtime_tx, _) = broadcast::channel(16);
        Self {
            commands: Arc::new(Mutex::new(Vec::new())),
            fail_next: Arc::new(Mutex::new(false)),
            runtime_tx,
            current_position: Arc::new(Mutex::new(None)),
        }
    }
}

impl FakeBackend {
    fn set_position(&self, seconds: f64) {
        *self.current_position.lock().unwrap() = Some(Duration::from_secs_f64(seconds));
    }
}

impl PlaybackBackend for FakeBackend {
    fn load_and_play(
        &self,
        path: &std::path::Path,
        generation: u64,
    ) -> crate::core::error::MeloResult<()> {
        self.commands.lock().unwrap().push(PlaybackCommand::Load {
            path: path.to_path_buf(),
            generation,
        });
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

    fn subscribe_runtime_events(&self) -> broadcast::Receiver<PlaybackRuntimeEvent> {
        self.runtime_tx.subscribe()
    }

    fn current_position(&self) -> Option<Duration> {
        *self.current_position.lock().unwrap()
    }
}

#[tokio::test(start_paused = true)]
async fn progress_tick_updates_snapshot_position_and_fraction() {
    let backend = Arc::new(FakeBackend::default());
    let service = Arc::new(PlayerService::new(backend.clone()));
    service.start_runtime_event_loop();
    service.start_progress_loop();

    service.append(item(1, "One")).await.unwrap();
    service.play().await.unwrap();
    backend.set_position(42.0);

    tokio::time::advance(Duration::from_millis(600)).await;

    let snapshot = service.snapshot().await;
    assert_eq!(snapshot.position_seconds, Some(42.0));
    assert!(snapshot.position_fraction.unwrap() > 0.23);
}

#[tokio::test(start_paused = true)]
async fn unchanged_progress_tick_does_not_bump_version() {
    let backend = Arc::new(FakeBackend::default());
    let service = Arc::new(PlayerService::new(backend.clone()));
    service.start_runtime_event_loop();
    service.start_progress_loop();

    service.append(item(1, "One")).await.unwrap();
    service.play().await.unwrap();
    backend.set_position(5.0);
    tokio::time::advance(Duration::from_millis(600)).await;
    let first = service.snapshot().await;

    backend.set_position(5.0);
    tokio::time::advance(Duration::from_millis(600)).await;
    let second = service.snapshot().await;

    assert_eq!(first.position_seconds, Some(5.0));
    assert_eq!(second.position_seconds, Some(5.0));
    assert_eq!(first.version, second.version);
}

#[tokio::test]
async fn stop_resets_progress_to_zero_for_current_song() {
    let backend = Arc::new(FakeBackend::default());
    let service = Arc::new(PlayerService::new(backend.clone()));

    service.append(item(1, "One")).await.unwrap();
    service.play().await.unwrap();
    backend.set_position(17.0);
    service.refresh_progress_once().await.unwrap();

    let snapshot = service.stop().await.unwrap();
    assert_eq!(snapshot.playback_state, PlaybackState::Stopped.as_str());
    assert_eq!(snapshot.position_seconds, Some(0.0));
    assert_eq!(snapshot.position_fraction, Some(0.0));
}
```

- [ ] **Step 2: Run the focused test to verify it fails**

Run: `cargo test progress_tick_updates_snapshot_position_and_fraction --lib -- --nocapture`  
Expected: FAIL because `current_position`, `position_seconds`, `position_fraction`, `start_progress_loop()`, and `refresh_progress_once()` do not exist yet.

- [ ] **Step 3: Implement snapshot progress fields, backend position query, and service ticker**

```rust
// src/core/model/player.rs
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct PlayerSnapshot {
    pub playback_state: String,
    pub current_song: Option<NowPlayingSong>,
    pub queue_len: usize,
    pub queue_index: Option<usize>,
    pub has_next: bool,
    pub has_prev: bool,
    pub last_error: Option<PlayerErrorInfo>,
    pub version: u64,
    pub position_seconds: Option<f64>,
    pub position_fraction: Option<f64>,
}

impl Default for PlayerSnapshot {
    fn default() -> Self {
        Self {
            playback_state: PlaybackState::Idle.as_str().to_string(),
            current_song: None,
            queue_len: 0,
            queue_index: None,
            has_next: false,
            has_prev: false,
            last_error: None,
            version: 0,
            position_seconds: None,
            position_fraction: None,
        }
    }
}
```

```rust
// src/domain/player/backend.rs
use std::time::Duration;

pub trait PlaybackBackend: Send + Sync {
    fn load_and_play(
        &self,
        path: &std::path::Path,
        generation: u64,
    ) -> crate::core::error::MeloResult<()>;
    fn pause(&self) -> crate::core::error::MeloResult<()>;
    fn resume(&self) -> crate::core::error::MeloResult<()>;
    fn stop(&self) -> crate::core::error::MeloResult<()>;
    fn subscribe_runtime_events(&self) -> PlaybackRuntimeReceiver;
    fn current_position(&self) -> Option<Duration>;
}

impl PlaybackBackend for NoopBackend {
    fn current_position(&self) -> Option<Duration> {
        None
    }
}
```

```rust
// src/domain/player/rodio_backend.rs
impl PlaybackBackend for RodioBackend {
    fn current_position(&self) -> Option<std::time::Duration> {
        self.player
            .lock()
            .unwrap()
            .as_ref()
            .map(|player| player.get_pos())
    }
}
```

```rust
// src/domain/player/service.rs
struct PlayerSession {
    playback_state: PlaybackState,
    queue: PlayerQueue,
    last_error: Option<PlayerErrorInfo>,
    version: u64,
    playback_generation: u64,
    position_seconds: Option<f64>,
}

impl Default for PlayerSession {
    fn default() -> Self {
        Self {
            playback_state: PlaybackState::Idle,
            queue: PlayerQueue::default(),
            last_error: None,
            version: 0,
            playback_generation: 0,
            position_seconds: None,
        }
    }
}

impl PlayerService {
    pub fn start_progress_loop(self: &Arc<Self>) {
        let service = Arc::clone(self);
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(std::time::Duration::from_millis(500));
            loop {
                ticker.tick().await;
                let _ = service.refresh_progress_once().await;
            }
        });
    }

    pub async fn refresh_progress_once(&self) -> MeloResult<Option<PlayerSnapshot>> {
        let mut session = self.session.lock().await;
        if session.playback_state != PlaybackState::Playing {
            return Ok(None);
        }

        let Some(position) = self.backend.current_position() else {
            return Ok(None);
        };
        let position_seconds = position.as_secs_f64();
        let changed = session
            .position_seconds
            .map(|previous| (previous - position_seconds).abs() >= 0.25)
            .unwrap_or(true);
        if !changed {
            return Ok(None);
        }

        session.position_seconds = Some(position_seconds);
        let snapshot = self.publish_locked(&mut session)?;
        Ok(Some(snapshot))
    }

    fn snapshot_from_session(session: &PlayerSession) -> PlayerSnapshot {
        let current_song = session.queue.current().map(|item| NowPlayingSong {
            song_id: item.song_id,
            title: item.title.clone(),
            duration_seconds: item.duration_seconds,
        });
        let position_fraction = match (
            session.position_seconds,
            current_song.as_ref().and_then(|song| song.duration_seconds),
        ) {
            (Some(position), Some(duration)) if duration > 0.0 => Some((position / duration).min(1.0)),
            _ => None,
        };

        PlayerSnapshot {
            playback_state: session.playback_state.as_str().to_string(),
            current_song,
            queue_len: session.queue.len(),
            queue_index: session.queue.current_index(),
            has_next: session.queue.has_next(),
            has_prev: session.queue.has_prev(),
            last_error: session.last_error.clone(),
            version: session.version,
            position_seconds: session.position_seconds,
            position_fraction,
        }
    }

    pub async fn play(&self) -> MeloResult<PlayerSnapshot> {
        let mut session = self.session.lock().await;
        if session.queue.is_empty() {
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
        let generation = session.playback_generation + 1;
        self.backend
            .load_and_play(std::path::Path::new(&current.path), generation)?;

        session.playback_generation = generation;
        session.playback_state = PlaybackState::Playing;
        session.last_error = None;
        session.position_seconds = Some(0.0);
        self.publish_locked(&mut session)
    }

    pub async fn pause(&self) -> MeloResult<PlayerSnapshot> {
        let mut session = self.session.lock().await;
        if session.playback_state != PlaybackState::Playing {
            return Ok(Self::snapshot_from_session(&session));
        }

        self.backend.pause()?;
        if let Some(position) = self.backend.current_position() {
            session.position_seconds = Some(position.as_secs_f64());
        }
        session.playback_state = PlaybackState::Paused;
        self.publish_locked(&mut session)
    }

    pub async fn stop(&self) -> MeloResult<PlayerSnapshot> {
        let mut session = self.session.lock().await;
        let target_state = if session.queue.is_empty() {
            PlaybackState::Idle
        } else {
            PlaybackState::Stopped
        };

        if session.playback_state == target_state {
            return Ok(Self::snapshot_from_session(&session));
        }

        self.backend.stop()?;
        session.playback_state = target_state;
        session.position_seconds = session.queue.current().map(|_| 0.0);
        self.publish_locked(&mut session)
    }
}
```

```rust
// src/daemon/app.rs
pub fn with_backend(backend: Arc<dyn PlaybackBackend>) -> Self {
    let player = Arc::new(PlayerService::new(backend));
    player.start_runtime_event_loop();
    player.start_progress_loop();
    Self { player }
}
```

- [ ] **Step 4: Run the focused library tests and verify they pass**

Run: `cargo test progress_tick_updates_snapshot_position_and_fraction --lib -- --nocapture`  
Expected: PASS and the service now publishes throttled position updates.

Run: `cargo test unchanged_progress_tick_does_not_bump_version --lib -- --nocapture`  
Expected: PASS and unchanged progress no longer dirties the snapshot version.

- [ ] **Step 5: Run project QA before committing**

Run: `pnpm qa`  
Expected: PASS including formatting, clippy, and the full test suite.

- [ ] **Step 6: Commit the shared progress contract slice**

```bash
git add src/core/model/player.rs src/domain/player/backend.rs src/domain/player/rodio_backend.rs src/domain/player/service.rs src/domain/player/service/tests.rs src/daemon/app.rs
git commit -m "feat: add playback progress sync"
```

---

### Task 2: Surface progress through WebSocket, CLI, and TUI

**Files:**
- Modify: `src/tui/ui/playbar.rs`
- Modify: `tests/api_server.rs`
- Modify: `tests/cli_remote.rs`
- Modify: `tests/tui_app.rs`

- [ ] **Step 1: Write the failing integration and UI tests for progress rendering**

```rust
// append to tests/cli_remote.rs
#[tokio::test(flavor = "multi_thread")]
async fn status_command_prints_progress_fields() {
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
    state.player.play().await.unwrap();
    let app = melo::daemon::server::router(state);
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let mut cmd = Command::cargo_bin("melo").unwrap();
    cmd.env("MELO_BASE_URL", format!("http://{addr}"));
    cmd.arg("status");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("position_seconds"))
        .stdout(predicate::str::contains("position_fraction"));
}
```

```rust
// append to tests/tui_app.rs
#[test]
fn playback_label_renders_progress_window() {
    let label = melo::tui::ui::playbar::playback_label(&melo::core::model::player::PlayerSnapshot {
        playback_state: "playing".into(),
        current_song: Some(melo::core::model::player::NowPlayingSong {
            song_id: 1,
            title: "Blue Bird".into(),
            duration_seconds: Some(212.0),
        }),
        queue_len: 1,
        queue_index: Some(0),
        has_next: false,
        has_prev: false,
        last_error: None,
        version: 3,
        position_seconds: Some(72.0),
        position_fraction: Some(72.0 / 212.0),
    });

    assert!(label.contains("01:12"));
    assert!(label.contains("03:32"));
    assert!(label.contains("Blue Bird"));
}
```

- [ ] **Step 2: Run the focused tests to verify they fail**

Run: `cargo test --test cli_remote status_command_prints_progress_fields -- --nocapture`  
Expected: FAIL because progress fields are not yet guaranteed in the remote output path.

Run: `cargo test --test tui_app playback_label_renders_progress_window -- --nocapture`  
Expected: FAIL because the playbar still renders only `state | title`.

- [ ] **Step 3: Implement progress display in the playbar and keep WebSocket consuming the shared snapshot**

```rust
// src/tui/ui/playbar.rs
fn format_mmss(seconds: f64) -> String {
    let rounded = seconds.floor() as u64;
    let minutes = rounded / 60;
    let secs = rounded % 60;
    format!("{minutes:02}:{secs:02}")
}

pub fn playback_label(snapshot: &PlayerSnapshot) -> String {
    let title = snapshot
        .current_song
        .as_ref()
        .map(|song| song.title.as_str())
        .unwrap_or("Nothing Playing");

    let progress = match (
        snapshot.position_seconds,
        snapshot
            .current_song
            .as_ref()
            .and_then(|song| song.duration_seconds),
    ) {
        (Some(position), Some(duration)) => {
            format!("{} / {}", format_mmss(position), format_mmss(duration))
        }
        (Some(position), None) => format!("{} / --:--", format_mmss(position)),
        _ => "--:-- / --:--".to_string(),
    };

    format!("{} | {} | {}", snapshot.playback_state, progress, title)
}
```

```rust
// tests/api_server.rs
#[tokio::test(flavor = "multi_thread")]
async fn websocket_status_contract_includes_progress_fields() {
    let app = melo::daemon::app::test_router().await;
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let (mut stream, _response) = connect_async(format!("ws://{addr}/api/ws/player"))
        .await
        .unwrap();
    let message = stream.next().await.unwrap().unwrap();
    let snapshot: melo::core::model::player::PlayerSnapshot =
        serde_json::from_str(&message.into_text().unwrap()).unwrap();

    assert!(snapshot.position_seconds.is_some() || snapshot.position_seconds.is_none());
    assert!(snapshot.position_fraction.is_some() || snapshot.position_fraction.is_none());
}
```

- [ ] **Step 4: Run the focused integration tests and verify they pass**

Run: `cargo test --test cli_remote status_command_prints_progress_fields -- --nocapture`  
Expected: PASS and CLI JSON includes the new progress fields.

Run: `cargo test --test tui_app playback_label_renders_progress_window -- --nocapture`  
Expected: PASS and the playbar shows `MM:SS / MM:SS`.

Run: `cargo test --test api_server websocket_status_contract_includes_progress_fields -- --nocapture`  
Expected: PASS and the shared snapshot contract reaches WebSocket unchanged.

- [ ] **Step 5: Run project QA before committing**

Run: `pnpm qa`  
Expected: PASS including formatting, clippy, and the full test suite.

- [ ] **Step 6: Commit the progress consumer slice**

```bash
git add src/tui/ui/playbar.rs tests/api_server.rs tests/cli_remote.rs tests/tui_app.rs
git commit -m "feat: expose playback progress across clients"
```

---

## Self-review notes

### Spec coverage

- `PlayerSnapshot` 进度字段：Task 1
- backend 位置查询与 service 节流：Task 1
- pause / stop / 切歌的进度语义：Task 1
- CLI / WebSocket / TUI 统一消费：Task 2

### Placeholder scan

- 没有遗留占位式描述
- 所有任务都给了具体测试、命令和提交信息

### Type consistency

- 进度字段统一使用 `position_seconds` 和 `position_fraction`
- backend 查询接口统一使用 `current_position() -> Option<Duration>`
- 进度刷新入口统一使用 `start_progress_loop()` / `refresh_progress_once()`

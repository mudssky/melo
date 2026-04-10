# Melo Playback Auto Advance Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Teach `PlayerService` to react to real playback-end events so Melo can automatically advance to the next queue item and stop cleanly at queue tail.

**Architecture:** Add a runtime-event contract between `PlaybackBackend` and `PlayerService`, with a generation token to ignore stale end events. `RodioBackend` becomes responsible for emitting `TrackEnded` when the active `rodio::Player` finishes, while `PlayerService` remains the only writable state source and decides whether to advance or stop.

**Tech Stack:** Rust 2024, Tokio broadcast/watch, Rodio 0.22 `Player::sleep_until_end()` / `Player::empty()`, Axum WebSocket, existing Rust unit/integration test stack

---

## File structure impact

### Existing files to modify

- Modify: `src/domain/player/mod.rs`
- Modify: `src/domain/player/backend.rs`
- Modify: `src/domain/player/rodio_backend.rs`
- Modify: `src/domain/player/service.rs`
- Modify: `src/domain/player/service/tests.rs`
- Modify: `src/daemon/app.rs`
- Modify: `tests/api_server.rs`
- Modify: `tests/player_service.rs`

### New files to create

- Create: `src/domain/player/runtime.rs`

### Responsibilities

- `src/domain/player/runtime.rs`
  Shared runtime event types emitted by playback backends
- `src/domain/player/backend.rs`
  Backend trait extensions for runtime-event subscription and generation-aware playback starts
- `src/domain/player/rodio_backend.rs`
  Emit `TrackEnded` when the active `rodio::Player` really finishes
- `src/domain/player/service.rs`
  Maintain `playback_generation`, consume runtime events, auto-advance or stop
- `src/daemon/app.rs`
  Start the service runtime event loop as part of app construction
- `tests/api_server.rs`
  Verify WebSocket and HTTP observe the same auto-advance result

---

### Task 1: Add runtime event contract and service auto-advance state machine

**Files:**
- Create: `src/domain/player/runtime.rs`
- Modify: `src/domain/player/mod.rs`
- Modify: `src/domain/player/backend.rs`
- Modify: `src/domain/player/service.rs`
- Modify: `src/domain/player/service/tests.rs`
- Modify: `src/daemon/app.rs`

- [ ] **Step 1: Write the failing service tests for runtime auto-advance**

```rust
// append to src/domain/player/service/tests.rs
use tokio::sync::broadcast;

use crate::domain::player::runtime::PlaybackRuntimeEvent;

#[derive(Clone)]
struct FakeRuntimeHandle {
    tx: broadcast::Sender<PlaybackRuntimeEvent>,
}

impl FakeRuntimeHandle {
    fn track_ended(&self, generation: u64) {
        let _ = self.tx.send(PlaybackRuntimeEvent::TrackEnded { generation });
    }
}

#[derive(Clone)]
struct FakeBackend {
    commands: Arc<Mutex<Vec<PlaybackCommand>>>,
    fail_next: Arc<Mutex<bool>>,
    runtime_tx: broadcast::Sender<PlaybackRuntimeEvent>,
}

impl Default for FakeBackend {
    fn default() -> Self {
        let (runtime_tx, _) = broadcast::channel(16);
        Self {
            commands: Arc::new(Mutex::new(Vec::new())),
            fail_next: Arc::new(Mutex::new(false)),
            runtime_tx,
        }
    }
}

impl FakeBackend {
    fn runtime_handle(&self) -> FakeRuntimeHandle {
        FakeRuntimeHandle {
            tx: self.runtime_tx.clone(),
        }
    }
}

impl PlaybackBackend for FakeBackend {
    fn load_and_play(
        &self,
        path: &std::path::Path,
        generation: u64,
    ) -> crate::core::error::MeloResult<()> {
        let mut fail_next = self.fail_next.lock().unwrap();
        if *fail_next {
            *fail_next = false;
            return Err(crate::core::error::MeloError::Message(
                "backend failed".to_string(),
            ));
        }

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
}

#[tokio::test]
async fn runtime_track_end_auto_advances_to_next_item() {
    let backend = Arc::new(FakeBackend::default());
    let runtime = backend.runtime_handle();
    let service = Arc::new(PlayerService::new(backend.clone()));
    service.start_runtime_event_loop();

    service.append(item(1, "One")).await.unwrap();
    service.append(item(2, "Two")).await.unwrap();
    service.play().await.unwrap();

    runtime.track_ended(1);
    tokio::task::yield_now().await;

    let snapshot = service.snapshot().await;
    assert_eq!(snapshot.playback_state, PlaybackState::Playing.as_str());
    assert_eq!(snapshot.queue_index, Some(1));
    assert_eq!(snapshot.current_song.unwrap().title, "Two");
}

#[tokio::test]
async fn stale_runtime_track_end_is_ignored_after_manual_next() {
    let backend = Arc::new(FakeBackend::default());
    let runtime = backend.runtime_handle();
    let service = Arc::new(PlayerService::new(backend));
    service.start_runtime_event_loop();

    service.append(item(1, "One")).await.unwrap();
    service.append(item(2, "Two")).await.unwrap();
    service.play().await.unwrap();
    service.next().await.unwrap();

    runtime.track_ended(1);
    tokio::task::yield_now().await;

    let snapshot = service.snapshot().await;
    assert_eq!(snapshot.queue_index, Some(1));
    assert_eq!(snapshot.current_song.unwrap().title, "Two");
}

#[tokio::test]
async fn queue_tail_track_end_sets_stopped_without_error() {
    let backend = Arc::new(FakeBackend::default());
    let runtime = backend.runtime_handle();
    let service = Arc::new(PlayerService::new(backend));
    service.start_runtime_event_loop();

    service.append(item(1, "One")).await.unwrap();
    service.play().await.unwrap();

    runtime.track_ended(1);
    tokio::task::yield_now().await;

    let snapshot = service.snapshot().await;
    assert_eq!(snapshot.playback_state, PlaybackState::Stopped.as_str());
    assert_eq!(snapshot.queue_index, Some(0));
    assert!(snapshot.last_error.is_none());
}
```

- [ ] **Step 2: Run the new runtime test to verify it fails**

Run: `cargo test runtime_track_end_auto_advances_to_next_item --lib -- --nocapture`  
Expected: FAIL because `PlaybackRuntimeEvent`, generation-aware `load_and_play`, and `PlayerService::start_runtime_event_loop()` do not exist yet.

- [ ] **Step 3: Implement runtime event types, generation-aware backend contract, and service event handling**

```rust
// src/domain/player/runtime.rs
use tokio::sync::broadcast;

/// 播放后端回传给服务层的运行时事件。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlaybackRuntimeEvent {
    /// 当前 generation 对应的曲目自然播放结束。
    TrackEnded { generation: u64 },
}

/// 播放运行时事件订阅器。
pub type PlaybackRuntimeReceiver = broadcast::Receiver<PlaybackRuntimeEvent>;
```

```rust
// src/domain/player/backend.rs
use tokio::sync::broadcast;

use crate::domain::player::runtime::{PlaybackRuntimeEvent, PlaybackRuntimeReceiver};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlaybackCommand {
    Load {
        path: std::path::PathBuf,
        generation: u64,
    },
    Pause,
    Resume,
    Stop,
}

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
}

#[derive(Default)]
pub struct NoopBackend;

impl PlaybackBackend for NoopBackend {
    fn load_and_play(
        &self,
        _path: &std::path::Path,
        _generation: u64,
    ) -> crate::core::error::MeloResult<()> {
        Ok(())
    }

    fn pause(&self) -> crate::core::error::MeloResult<()> { Ok(()) }
    fn resume(&self) -> crate::core::error::MeloResult<()> { Ok(()) }
    fn stop(&self) -> crate::core::error::MeloResult<()> { Ok(()) }

    fn subscribe_runtime_events(&self) -> broadcast::Receiver<PlaybackRuntimeEvent> {
        let (_tx, rx) = broadcast::channel(1);
        rx
    }
}
```

```rust
// src/domain/player/service.rs
#[derive(Debug)]
struct PlayerSession {
    playback_state: PlaybackState,
    queue: PlayerQueue,
    last_error: Option<PlayerErrorInfo>,
    version: u64,
    playback_generation: u64,
}

impl Default for PlayerSession {
    fn default() -> Self {
        Self {
            playback_state: PlaybackState::Idle,
            queue: PlayerQueue::default(),
            last_error: None,
            version: 0,
            playback_generation: 0,
        }
    }
}

impl PlayerService {
    pub fn start_runtime_event_loop(self: &Arc<Self>) {
        let service = Arc::clone(self);
        tokio::spawn(async move {
            let mut receiver = service.backend.subscribe_runtime_events();
            while let Ok(event) = receiver.recv().await {
                service.handle_runtime_event(event).await;
            }
        });
    }

    async fn handle_runtime_event(&self, event: PlaybackRuntimeEvent) {
        match event {
            PlaybackRuntimeEvent::TrackEnded { generation } => {
                let decision = {
                    let mut session = self.session.lock().await;
                    if generation != session.playback_generation {
                        return;
                    }

                    match session.queue.current_index() {
                        Some(index) if index + 1 < session.queue.len() => {
                            let _ = session.queue.play_index(index + 1);
                            Some(true)
                        }
                        Some(_) => {
                            session.playback_state = PlaybackState::Stopped;
                            session.last_error = None;
                            let _ = self.publish_locked(&mut session);
                            Some(false)
                        }
                        None => None,
                    }
                };

                if matches!(decision, Some(true)) {
                    let _ = self.play().await;
                }
            }
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
        if let Err(err) = self
            .backend
            .load_and_play(std::path::Path::new(&current.path), generation)
        {
            return self.fail_locked(&mut session, "backend_unavailable", &err.to_string(), err);
        }

        session.playback_generation = generation;
        session.playback_state = PlaybackState::Playing;
        session.last_error = None;
        self.publish_locked(&mut session)
    }
}
```

```rust
// src/daemon/app.rs
pub fn with_backend(backend: Arc<dyn PlaybackBackend>) -> Self {
    let player = Arc::new(PlayerService::new(backend));
    player.start_runtime_event_loop();
    Self { player }
}
```

- [ ] **Step 4: Run the focused library tests and verify they pass**

Run: `cargo test runtime_track_end_ --lib -- --nocapture`  
Expected: PASS and auto-advance / stale-event guards are green.

- [ ] **Step 5: Run project QA before committing**

Run: `pnpm qa`  
Expected: PASS including `cargo fmt`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test -q`.

- [ ] **Step 6: Commit the runtime-event state machine slice**

```bash
git add src/domain/player/mod.rs src/domain/player/runtime.rs src/domain/player/backend.rs src/domain/player/service.rs src/domain/player/service/tests.rs src/daemon/app.rs
git commit -m "feat: auto advance playback on track end"
```

---

### Task 2: Emit real end-of-track events from `RodioBackend` and verify API/WebSocket observe them

**Files:**
- Modify: `src/domain/player/rodio_backend.rs`
- Modify: `tests/api_server.rs`
- Modify: `tests/player_service.rs`

- [ ] **Step 1: Write the failing integration test for auto-advance broadcast**

```rust
// append to tests/api_server.rs
use std::sync::{Arc, Mutex};

use futures_util::StreamExt;
use tokio::sync::broadcast;
use tokio_tungstenite::connect_async;

use melo::domain::player::backend::{PlaybackBackend, PlaybackCommand};
use melo::domain::player::runtime::PlaybackRuntimeEvent;

#[derive(Clone)]
struct EventedBackend {
    commands: Arc<Mutex<Vec<PlaybackCommand>>>,
    runtime_tx: broadcast::Sender<PlaybackRuntimeEvent>,
}

impl Default for EventedBackend {
    fn default() -> Self {
        let (runtime_tx, _) = broadcast::channel(16);
        Self {
            commands: Arc::new(Mutex::new(Vec::new())),
            runtime_tx,
        }
    }
}

impl EventedBackend {
    fn emit_track_end(&self, generation: u64) {
        let _ = self.runtime_tx.send(PlaybackRuntimeEvent::TrackEnded { generation });
    }
}

impl PlaybackBackend for EventedBackend {
    fn load_and_play(
        &self,
        path: &std::path::Path,
        generation: u64,
    ) -> melo::core::error::MeloResult<()> {
        self.commands.lock().unwrap().push(PlaybackCommand::Load {
            path: path.to_path_buf(),
            generation,
        });
        Ok(())
    }

    fn pause(&self) -> melo::core::error::MeloResult<()> { Ok(()) }
    fn resume(&self) -> melo::core::error::MeloResult<()> { Ok(()) }
    fn stop(&self) -> melo::core::error::MeloResult<()> { Ok(()) }

    fn subscribe_runtime_events(&self) -> broadcast::Receiver<PlaybackRuntimeEvent> {
        self.runtime_tx.subscribe()
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn websocket_receives_auto_advanced_snapshot_after_track_end() {
    let backend = Arc::new(EventedBackend::default());
    let state = melo::daemon::app::AppState::with_backend(backend.clone());
    state
        .player
        .append(melo::core::model::player::QueueItem {
            song_id: 1,
            path: "tests/fixtures/full_test.mp3".into(),
            title: "One".into(),
            duration_seconds: Some(212.0),
        })
        .await
        .unwrap();
    state
        .player
        .append(melo::core::model::player::QueueItem {
            song_id: 2,
            path: "tests/fixtures/full_test.mp3".into(),
            title: "Two".into(),
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

    let (mut stream, _response) = connect_async(format!("ws://{addr}/api/ws/player"))
        .await
        .unwrap();

    let _initial = stream.next().await.unwrap().unwrap();
    backend.emit_track_end(1);

    let advanced = stream.next().await.unwrap().unwrap();
    let text = advanced.into_text().unwrap();
    let snapshot: melo::core::model::player::PlayerSnapshot =
        serde_json::from_str(&text).unwrap();

    assert_eq!(snapshot.queue_index, Some(1));
    assert_eq!(snapshot.current_song.unwrap().title, "Two");
}
```

- [ ] **Step 2: Run the integration test to verify it fails**

Run: `cargo test --test api_server websocket_receives_auto_advanced_snapshot_after_track_end -- --nocapture`  
Expected: FAIL because `RodioBackend` and the production backend contract still do not emit runtime events, and `PlaybackCommand::Load` shape has not been updated in integration helpers.

- [ ] **Step 3: Implement generation-aware `RodioBackend` watchers and update integration helpers**

```rust
// src/domain/player/rodio_backend.rs
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use tokio::sync::broadcast;

use crate::domain::player::runtime::{PlaybackRuntimeEvent, PlaybackRuntimeReceiver};

pub struct RodioBackend {
    sink: rodio::MixerDeviceSink,
    player: Mutex<Option<Arc<rodio::Player>>>,
    runtime_tx: broadcast::Sender<PlaybackRuntimeEvent>,
    active_generation: AtomicU64,
}

impl RodioBackend {
    pub fn new() -> MeloResult<Self> {
        let sink = rodio::DeviceSinkBuilder::open_default_sink()
            .map_err(|err| MeloError::Message(err.to_string()))?;
        let (runtime_tx, _) = broadcast::channel(16);
        Ok(Self {
            sink,
            player: Mutex::new(None),
            runtime_tx,
            active_generation: AtomicU64::new(0),
        })
    }
}

impl PlaybackBackend for RodioBackend {
    fn load_and_play(&self, path: &std::path::Path, generation: u64) -> MeloResult<()> {
        let file = std::fs::File::open(path).map_err(|err| MeloError::Message(err.to_string()))?;
        let decoder =
            rodio::Decoder::try_from(file).map_err(|err| MeloError::Message(err.to_string()))?;
        let player = Arc::new(rodio::Player::connect_new(self.sink.mixer()));
        player.append(decoder);
        player.play();

        self.active_generation.store(generation, Ordering::SeqCst);
        let watcher_player = Arc::clone(&player);
        let runtime_tx = self.runtime_tx.clone();
        let active_generation = &self.active_generation;
        std::thread::spawn({
            let active_generation = active_generation as *const AtomicU64;
            move || {
                watcher_player.sleep_until_end();
                let active_generation = unsafe { &*active_generation };
                if active_generation.load(Ordering::SeqCst) == generation && watcher_player.empty() {
                    let _ = runtime_tx.send(PlaybackRuntimeEvent::TrackEnded { generation });
                }
            }
        });

        let mut current_player = self.player.lock().unwrap();
        if let Some(previous_player) = current_player.take() {
            previous_player.stop();
        }
        *current_player = Some(player);
        Ok(())
    }

    fn subscribe_runtime_events(&self) -> PlaybackRuntimeReceiver {
        self.runtime_tx.subscribe()
    }
}
```

```rust
// tests/player_service.rs
assert_eq!(
    backend.commands.lock().unwrap().last(),
    Some(&PlaybackCommand::Load {
        path: std::path::PathBuf::from("tests/fixtures/full_test.mp3"),
        generation: 1,
    })
);
```

- [ ] **Step 4: Run focused integration tests and verify they pass**

Run: `cargo test --test api_server websocket_receives_auto_advanced_snapshot_after_track_end -- --nocapture`  
Expected: PASS and the WebSocket stream shows the same auto-advance result the service snapshot exposes.

Run: `cargo test --test player_service -- --nocapture`  
Expected: PASS with the updated generation-aware backend command assertions.

- [ ] **Step 5: Run project QA before committing**

Run: `pnpm qa`  
Expected: PASS including formatting, clippy, and the full Rust test suite.

- [ ] **Step 6: Commit the Rodio runtime event slice**

```bash
git add src/domain/player/rodio_backend.rs tests/api_server.rs tests/player_service.rs
git commit -m "feat: emit backend runtime events for auto advance"
```

---

## Self-review notes

### Spec coverage

- backend -> service 运行时事件通道：Task 1
- generation 防旧事件污染：Task 1
- 队尾自然结束收敛到 `stopped`：Task 1
- `RodioBackend` 真实结束事件接入：Task 2
- WebSocket / HTTP 共享自动推进结果：Task 2

### Placeholder scan

- 没有遗留占位式描述
- 每个任务都包含具体文件、测试代码、运行命令和提交信息

### Type consistency

- 运行时事件统一使用 `PlaybackRuntimeEvent`
- generation 字段统一命名为 `generation` / `playback_generation`
- backend 播放入口统一为 `load_and_play(path, generation)`

# Melo Playback Volume And Modes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add unified volume, mute, repeat, and shuffle control so the player can honor user playback preferences across daemon, CLI, API, WebSocket, and TUI.

**Architecture:** Keep mode and volume state inside `PlayerService`, add a pure navigation helper for repeat/shuffle decisions, and limit `PlaybackBackend` to actual volume execution. All external surfaces continue consuming a richer `PlayerSnapshot` rather than inventing local semantics.

**Tech Stack:** Rust 2024, Tokio, Rodio 0.22 `Player::set_volume()`, Axum JSON APIs, Clap subcommands, Ratatui playbar/footer rendering, existing Rust unit/integration test stack

---

## File structure impact

### Existing files to modify

- Modify: `src/core/model/player.rs`
- Modify: `src/domain/player/mod.rs`
- Modify: `src/domain/player/backend.rs`
- Modify: `src/domain/player/rodio_backend.rs`
- Modify: `src/domain/player/service.rs`
- Modify: `src/domain/player/service/tests.rs`
- Modify: `src/api/player.rs`
- Modify: `src/daemon/server.rs`
- Modify: `src/cli/args.rs`
- Modify: `src/cli/client.rs`
- Modify: `src/cli/run.rs`
- Modify: `src/tui/app.rs`
- Modify: `src/tui/ui/playbar.rs`
- Modify: `tests/api_server.rs`
- Modify: `tests/cli_help.rs`
- Modify: `tests/cli_remote.rs`
- Modify: `tests/tui_app.rs`

### New files to create

- Create: `src/domain/player/navigation.rs`
- Create: `src/domain/player/navigation/tests.rs`

### Responsibilities

- `src/core/model/player.rs`
  Shared `RepeatMode` enum and snapshot fields for volume/mute/modes
- `src/domain/player/navigation.rs`
  Pure repeat/shuffle navigation rules with deterministic shuffle order
- `src/domain/player/backend.rs`
  Backend volume execution capability
- `src/domain/player/service.rs`
  State transitions for volume, mute, repeat, shuffle, and navigation
- `src/api/player.rs`
  HTTP handlers for volume and mode updates
- `src/cli/*`
  Structured remote commands for volume and mode control

---

### Task 1: Add pure navigation rules and failing service tests for volume/mode semantics

**Files:**
- Create: `src/domain/player/navigation.rs`
- Create: `src/domain/player/navigation/tests.rs`
- Modify: `src/domain/player/mod.rs`
- Modify: `src/core/model/player.rs`
- Modify: `src/domain/player/service/tests.rs`

- [ ] **Step 1: Write the failing navigation and service tests**

```rust
// src/domain/player/navigation/tests.rs
use crate::core::model::player::RepeatMode;
use crate::domain::player::navigation::PlaybackNavigation;

#[test]
fn repeat_all_wraps_manual_next_from_tail_to_head() {
    let navigation = PlaybackNavigation::linear(3, Some(2));
    assert_eq!(
        navigation.next_index(RepeatMode::All, false),
        Some(0)
    );
}

#[test]
fn repeat_one_replays_current_track_on_track_end() {
    let navigation = PlaybackNavigation::linear(3, Some(1));
    assert_eq!(
        navigation.track_end_index(RepeatMode::One, false),
        Some(1)
    );
}

#[test]
fn shuffle_uses_projected_order_without_mutating_visible_queue() {
    let navigation = PlaybackNavigation::shuffled(4, Some(1), 7);
    let projected = navigation.order().to_vec();

    assert_eq!(projected.len(), 4);
    assert!(projected.contains(&0));
    assert!(projected.contains(&1));
    assert!(projected.contains(&2));
    assert!(projected.contains(&3));
    assert_eq!(navigation.current_visible_index(), Some(1));
}
```

```rust
// append to src/domain/player/service/tests.rs
#[tokio::test]
async fn set_volume_updates_snapshot_and_backend_once() {
    let backend = Arc::new(FakeBackend::default());
    let service = PlayerService::new(backend.clone());

    let snapshot = service.set_volume_percent(70).await.unwrap();
    let second = service.set_volume_percent(70).await.unwrap();

    assert_eq!(snapshot.volume_percent, 70);
    assert!(!snapshot.muted);
    assert_eq!(second.version, snapshot.version);
    assert_eq!(
        backend
            .commands
            .lock()
            .unwrap()
            .iter()
            .filter(|cmd| matches!(cmd, PlaybackCommand::SetVolume { .. }))
            .count(),
        1
    );
}

#[tokio::test]
async fn repeat_all_wraps_queue_tail_on_manual_next() {
    let backend = Arc::new(FakeBackend::default());
    let service = PlayerService::new(backend);
    service.append(item(1, "One")).await.unwrap();
    service.append(item(2, "Two")).await.unwrap();
    service.set_repeat_mode(RepeatMode::All).await.unwrap();
    service.play_index(1).await.unwrap();

    let snapshot = service.next().await.unwrap();

    assert_eq!(snapshot.queue_index, Some(0));
    assert_eq!(snapshot.current_song.unwrap().title, "One");
}

#[tokio::test]
async fn mute_preserves_last_non_zero_volume() {
    let backend = Arc::new(FakeBackend::default());
    let service = PlayerService::new(backend);
    service.set_volume_percent(35).await.unwrap();

    let muted = service.mute().await.unwrap();
    let unmuted = service.unmute().await.unwrap();

    assert!(muted.muted);
    assert_eq!(unmuted.volume_percent, 35);
    assert!(!unmuted.muted);
}
```

- [ ] **Step 2: Run the focused tests to verify they fail**

Run: `cargo test repeat_all_wraps_manual_next_from_tail_to_head --lib -- --nocapture`  
Expected: FAIL because `PlaybackNavigation` and `RepeatMode` do not exist yet.

Run: `cargo test set_volume_updates_snapshot_and_backend_once --lib -- --nocapture`  
Expected: FAIL because `set_volume_percent()`, `mute()`, and volume-related snapshot fields do not exist yet.

- [ ] **Step 3: Implement the shared mode contract and pure navigation helper**

```rust
// src/core/model/player.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub enum RepeatMode {
    Off,
    One,
    All,
}

impl RepeatMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::One => "one",
            Self::All => "all",
        }
    }
}

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
    pub volume_percent: u8,
    pub muted: bool,
    pub repeat_mode: String,
    pub shuffle_enabled: bool,
}
```

```rust
// src/domain/player/navigation.rs
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use crate::core::model::player::RepeatMode;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaybackNavigation {
    visible_len: usize,
    current_visible_index: Option<usize>,
    order: Vec<usize>,
}

impl PlaybackNavigation {
    pub fn linear(visible_len: usize, current_visible_index: Option<usize>) -> Self {
        Self {
            visible_len,
            current_visible_index,
            order: (0..visible_len).collect(),
        }
    }

    pub fn shuffled(visible_len: usize, current_visible_index: Option<usize>, seed: u64) -> Self {
        let mut order = (0..visible_len).collect::<Vec<_>>();
        order.sort_by_key(|index| {
            let mut hasher = DefaultHasher::new();
            seed.hash(&mut hasher);
            index.hash(&mut hasher);
            hasher.finish()
        });
        Self {
            visible_len,
            current_visible_index,
            order,
        }
    }

    pub fn order(&self) -> &[usize] {
        &self.order
    }

    pub fn current_visible_index(&self) -> Option<usize> {
        self.current_visible_index
    }

    pub fn next_index(&self, repeat_mode: RepeatMode, shuffle_enabled: bool) -> Option<usize> {
        self.advance(Direction::Next, repeat_mode, shuffle_enabled)
    }

    pub fn prev_index(&self, repeat_mode: RepeatMode, shuffle_enabled: bool) -> Option<usize> {
        self.advance(Direction::Prev, repeat_mode, shuffle_enabled)
    }

    pub fn track_end_index(&self, repeat_mode: RepeatMode, shuffle_enabled: bool) -> Option<usize> {
        if repeat_mode == RepeatMode::One {
            return self.current_visible_index;
        }
        self.advance(Direction::Next, repeat_mode, shuffle_enabled)
    }

    fn advance(
        &self,
        direction: Direction,
        repeat_mode: RepeatMode,
        shuffle_enabled: bool,
    ) -> Option<usize> {
        let current = self.current_visible_index?;
        let order = if shuffle_enabled {
            &self.order
        } else {
            &(0..self.visible_len).collect::<Vec<_>>()
        };
        let order_pos = order.iter().position(|index| *index == current)?;

        match direction {
            Direction::Next if order_pos + 1 < order.len() => Some(order[order_pos + 1]),
            Direction::Prev if order_pos > 0 => Some(order[order_pos - 1]),
            Direction::Next if repeat_mode == RepeatMode::All => order.first().copied(),
            Direction::Prev if repeat_mode == RepeatMode::All => order.last().copied(),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Direction {
    Next,
    Prev,
}

#[cfg(test)]
mod tests;
```

- [ ] **Step 4: Run the focused library tests and verify they pass**

Run: `cargo test repeat_all_wraps_manual_next_from_tail_to_head --lib -- --nocapture`  
Expected: PASS and pure repeat/shuffle navigation rules are green.

Run: `cargo test set_volume_updates_snapshot_and_backend_once --lib -- --nocapture`  
Expected: PASS once the service can represent the new control state.

- [ ] **Step 5: Run project QA before committing**

Run: `pnpm qa`  
Expected: PASS including formatting, clippy, and the full test suite.

- [ ] **Step 6: Commit the shared mode/navigation contract slice**

```bash
git add src/core/model/player.rs src/domain/player/mod.rs src/domain/player/navigation.rs src/domain/player/navigation/tests.rs src/domain/player/service/tests.rs
git commit -m "feat: add playback navigation and mode contracts"
```

---

### Task 2: Implement service state, backend volume execution, and mode-aware playback control

**Files:**
- Modify: `src/domain/player/backend.rs`
- Modify: `src/domain/player/rodio_backend.rs`
- Modify: `src/domain/player/service.rs`

- [ ] **Step 1: Extend the fake backend command enum in tests and run a focused failure**

```rust
// src/domain/player/backend.rs
#[derive(Debug, Clone, PartialEq)]
pub enum PlaybackCommand {
    Load {
        path: std::path::PathBuf,
        generation: u64,
    },
    Pause,
    Resume,
    Stop,
    SetVolume {
        factor: f32,
    },
}
```

Run: `cargo test mute_preserves_last_non_zero_volume --lib -- --nocapture`  
Expected: FAIL because `PlaybackBackend::set_volume()` and the corresponding service fields are still missing.

- [ ] **Step 2: Implement backend volume control and mode-aware service logic**

```rust
// src/domain/player/backend.rs
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
    fn current_position(&self) -> Option<std::time::Duration>;
    fn set_volume(&self, factor: f32) -> crate::core::error::MeloResult<()>;
}
```

```rust
// src/domain/player/rodio_backend.rs
impl PlaybackBackend for RodioBackend {
    fn set_volume(&self, factor: f32) -> MeloResult<()> {
        if let Some(player) = self.player.lock().unwrap().as_ref() {
            player.set_volume(factor);
        }
        Ok(())
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
    volume_percent: u8,
    muted: bool,
    repeat_mode: RepeatMode,
    shuffle_enabled: bool,
    shuffle_seed: u64,
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
            volume_percent: 100,
            muted: false,
            repeat_mode: RepeatMode::Off,
            shuffle_enabled: false,
            shuffle_seed: 0,
        }
    }
}

impl PlayerService {
    pub async fn set_volume_percent(&self, volume_percent: u8) -> MeloResult<PlayerSnapshot> {
        let mut session = self.session.lock().await;
        if session.volume_percent == volume_percent && !session.muted {
            return Ok(Self::snapshot_from_session(&session));
        }

        session.volume_percent = volume_percent.min(100);
        session.muted = false;
        let factor = session.volume_percent as f32 / 100.0;
        self.backend.set_volume(factor)?;
        self.publish_locked(&mut session)
    }

    pub async fn mute(&self) -> MeloResult<PlayerSnapshot> {
        let mut session = self.session.lock().await;
        if session.muted {
            return Ok(Self::snapshot_from_session(&session));
        }

        session.muted = true;
        self.backend.set_volume(0.0)?;
        self.publish_locked(&mut session)
    }

    pub async fn unmute(&self) -> MeloResult<PlayerSnapshot> {
        let mut session = self.session.lock().await;
        if !session.muted {
            return Ok(Self::snapshot_from_session(&session));
        }

        session.muted = false;
        let factor = session.volume_percent as f32 / 100.0;
        self.backend.set_volume(factor)?;
        self.publish_locked(&mut session)
    }

    pub async fn set_repeat_mode(&self, repeat_mode: RepeatMode) -> MeloResult<PlayerSnapshot> {
        let mut session = self.session.lock().await;
        if session.repeat_mode == repeat_mode {
            return Ok(Self::snapshot_from_session(&session));
        }
        session.repeat_mode = repeat_mode;
        self.publish_locked(&mut session)
    }

    pub async fn set_shuffle_enabled(&self, enabled: bool) -> MeloResult<PlayerSnapshot> {
        let mut session = self.session.lock().await;
        if session.shuffle_enabled == enabled {
            return Ok(Self::snapshot_from_session(&session));
        }
        session.shuffle_enabled = enabled;
        if enabled {
            session.shuffle_seed = session.shuffle_seed.wrapping_add(1);
        }
        self.publish_locked(&mut session)
    }

    fn navigation(session: &PlayerSession) -> PlaybackNavigation {
        if session.shuffle_enabled {
            PlaybackNavigation::shuffled(
                session.queue.len(),
                session.queue.current_index(),
                session.shuffle_seed,
            )
        } else {
            PlaybackNavigation::linear(session.queue.len(), session.queue.current_index())
        }
    }

    pub async fn next(&self) -> MeloResult<PlayerSnapshot> {
        let mut session = self.session.lock().await;
        let Some(next_index) =
            Self::navigation(&session).next_index(session.repeat_mode, session.shuffle_enabled)
        else {
            return self.fail_locked(
                &mut session,
                "queue_no_next",
                "queue has no next item",
                MeloError::Message("queue has no next item".to_string()),
            );
        };
        let _ = session.queue.play_index(next_index)?;
        drop(session);
        self.play().await
    }
}
```

- [ ] **Step 3: Run the focused service tests and verify they pass**

Run: `cargo test repeat_all_wraps_queue_tail_on_manual_next --lib -- --nocapture`  
Expected: PASS and manual next now honors `RepeatMode::All`.

Run: `cargo test mute_preserves_last_non_zero_volume --lib -- --nocapture`  
Expected: PASS and mute/unmute preserve the prior non-zero volume.

- [ ] **Step 4: Run project QA before committing**

Run: `pnpm qa`  
Expected: PASS including formatting, clippy, and the full test suite.

- [ ] **Step 5: Commit the service/backend control slice**

```bash
git add src/domain/player/backend.rs src/domain/player/rodio_backend.rs src/domain/player/service.rs
git commit -m "feat: add playback volume and repeat controls"
```

---

### Task 3: Expose volume and modes over HTTP, CLI, and TUI

**Files:**
- Modify: `src/api/player.rs`
- Modify: `src/daemon/server.rs`
- Modify: `src/cli/args.rs`
- Modify: `src/cli/client.rs`
- Modify: `src/cli/run.rs`
- Modify: `src/tui/app.rs`
- Modify: `src/tui/ui/playbar.rs`
- Modify: `tests/api_server.rs`
- Modify: `tests/cli_help.rs`
- Modify: `tests/cli_remote.rs`
- Modify: `tests/tui_app.rs`

- [ ] **Step 1: Write the failing contract tests for HTTP, CLI, and TUI**

```rust
// append to tests/api_server.rs
#[tokio::test]
async fn player_volume_endpoint_updates_snapshot_contract() {
    let app = melo::daemon::app::test_router().await;
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/player/volume")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"volume_percent":55}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}
```

```rust
// append to tests/cli_help.rs
#[test]
fn cli_help_lists_structured_player_command() {
    let mut cmd = Command::cargo_bin("melo").unwrap();
    cmd.arg("--help");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("player"))
        .stdout(predicate::str::contains("queue"))
        .stdout(predicate::str::contains("playlist"));
}
```

```rust
// append to tests/cli_remote.rs
#[tokio::test(flavor = "multi_thread")]
async fn player_mode_show_prints_repeat_and_shuffle_fields() {
    let app = melo::daemon::app::test_router().await;
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let mut cmd = Command::cargo_bin("melo").unwrap();
    cmd.env("MELO_BASE_URL", format!("http://{addr}"));
    cmd.arg("player").arg("mode").arg("show");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("repeat_mode"))
        .stdout(predicate::str::contains("shuffle_enabled"));
}
```

```rust
// append to tests/tui_app.rs
#[test]
fn footer_status_includes_volume_and_repeat_mode() {
    let mut app = melo::tui::app::App::new_for_test();
    app.apply_snapshot(melo::core::model::player::PlayerSnapshot {
        playback_state: "playing".into(),
        current_song: None,
        queue_len: 2,
        queue_index: Some(0),
        has_next: true,
        has_prev: false,
        last_error: None,
        version: 6,
        position_seconds: Some(10.0),
        position_fraction: Some(0.1),
        volume_percent: 55,
        muted: false,
        repeat_mode: "all".into(),
        shuffle_enabled: true,
    });

    let footer = app.footer_status();
    assert!(footer.contains("vol=55"));
    assert!(footer.contains("repeat=all"));
    assert!(footer.contains("shuffle=true"));
}
```

- [ ] **Step 2: Run the focused tests to verify they fail**

Run: `cargo test --test api_server player_volume_endpoint_updates_snapshot_contract -- --nocapture`  
Expected: FAIL because `/api/player/volume` does not exist yet.

Run: `cargo test --test cli_remote player_mode_show_prints_repeat_and_shuffle_fields -- --nocapture`  
Expected: FAIL because the `player mode show` command path does not exist yet.

Run: `cargo test --test tui_app footer_status_includes_volume_and_repeat_mode -- --nocapture`  
Expected: FAIL because the footer does not yet expose volume/repeat/shuffle state.

- [ ] **Step 3: Implement HTTP handlers, structured CLI commands, and TUI rendering**

```rust
// src/api/player.rs
#[derive(Debug, serde::Deserialize)]
pub struct PlayerVolumeRequest {
    pub volume_percent: u8,
}

#[derive(Debug, serde::Deserialize)]
pub struct PlayerModeRequest {
    pub repeat_mode: Option<String>,
    pub shuffle_enabled: Option<bool>,
}

pub async fn set_volume(
    State(state): State<AppState>,
    Json(request): Json<PlayerVolumeRequest>,
) -> Json<crate::core::model::player::PlayerSnapshot> {
    Json(state.player.set_volume_percent(request.volume_percent).await.unwrap())
}

pub async fn set_mode(
    State(state): State<AppState>,
    Json(request): Json<PlayerModeRequest>,
) -> Json<crate::core::model::player::PlayerSnapshot> {
    let mut snapshot = state.player.snapshot().await;
    if let Some(repeat_mode) = request.repeat_mode {
        let mode = match repeat_mode.as_str() {
            "off" => RepeatMode::Off,
            "one" => RepeatMode::One,
            "all" => RepeatMode::All,
            _ => RepeatMode::Off,
        };
        snapshot = state.player.set_repeat_mode(mode).await.unwrap();
    }
    if let Some(shuffle_enabled) = request.shuffle_enabled {
        snapshot = state.player.set_shuffle_enabled(shuffle_enabled).await.unwrap();
    }
    Json(snapshot)
}
```

```rust
// src/daemon/server.rs
.route("/api/player/volume", axum::routing::post(crate::api::player::set_volume))
.route("/api/player/mode", axum::routing::post(crate::api::player::set_mode))
```

```rust
// src/cli/args.rs
#[derive(Debug, Subcommand)]
pub enum PlayerModeCommand {
    Show,
    Repeat { mode: String },
    Shuffle { enabled: String },
}

#[derive(Debug, Subcommand)]
pub enum PlayerCommand {
    Volume { value: u8 },
    Mute,
    Unmute,
    Mode {
        #[command(subcommand)]
        command: PlayerModeCommand,
    },
}

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
    Player {
        #[command(subcommand)]
        command: PlayerCommand,
    },
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
pub async fn player_volume(&self, value: u8) -> MeloResult<PlayerSnapshot> {
    let url = format!("{}/api/player/volume", self.base_url);
    self.client
        .post(url)
        .json(&serde_json::json!({ "volume_percent": value }))
        .send()
        .await
        .map_err(|err| MeloError::Message(err.to_string()))?
        .error_for_status()
        .map_err(|err| MeloError::Message(err.to_string()))?
        .json()
        .await
        .map_err(|err| MeloError::Message(err.to_string()))
}

pub async fn player_mode_show(&self) -> MeloResult<PlayerSnapshot> {
    self.status().await
}
```

```rust
// src/cli/run.rs
Some(Command::Player {
    command: PlayerCommand::Volume { value },
}) => {
    let snapshot = crate::cli::client::ApiClient::from_env()
        .player_volume(value)
        .await?;
    println!("{}", serde_json::to_string_pretty(&snapshot).unwrap());
}
Some(Command::Player {
    command: PlayerCommand::Mode {
        command: PlayerModeCommand::Show,
    },
}) => {
    let snapshot = crate::cli::client::ApiClient::from_env()
        .player_mode_show()
        .await?;
    println!("{}", serde_json::to_string_pretty(&snapshot).unwrap());
}
```

```rust
// src/tui/app.rs
pub fn footer_status(&self) -> String {
    if let Some(error) = &self.player.last_error {
        return format!("ERR {}: {}", error.code, error.message);
    }

    format!(
        "{} | queue={} | prev={} | next={} | vol={} | repeat={} | shuffle={}",
        self.player.playback_state,
        self.player.queue_len,
        self.player.has_prev,
        self.player.has_next,
        self.player.volume_percent,
        self.player.repeat_mode,
        self.player.shuffle_enabled
    )
}
```

```rust
// src/tui/ui/playbar.rs
pub fn playback_label(snapshot: &PlayerSnapshot) -> String {
    let title = snapshot
        .current_song
        .as_ref()
        .map(|song| song.title.as_str())
        .unwrap_or("Nothing Playing");
    let volume = if snapshot.muted {
        "muted".to_string()
    } else {
        format!("vol {}%", snapshot.volume_percent)
    };

    format!(
        "{} | {} | repeat={} | shuffle={} | {}",
        snapshot.playback_state,
        volume,
        snapshot.repeat_mode,
        snapshot.shuffle_enabled,
        title
    )
}
```

- [ ] **Step 4: Run the focused integration tests and verify they pass**

Run: `cargo test --test api_server player_volume_endpoint_updates_snapshot_contract -- --nocapture`  
Expected: PASS and HTTP returns the richer snapshot after a volume update.

Run: `cargo test --test cli_remote player_mode_show_prints_repeat_and_shuffle_fields -- --nocapture`  
Expected: PASS and the structured CLI prints the same mode state seen by HTTP.

Run: `cargo test --test tui_app footer_status_includes_volume_and_repeat_mode -- --nocapture`  
Expected: PASS and TUI continues consuming snapshot-only state.

Run: `cargo test --test cli_help cli_help_lists_structured_player_command -- --nocapture`  
Expected: PASS and `melo --help` now advertises the structured `player` namespace.

- [ ] **Step 5: Run project QA before committing**

Run: `pnpm qa`  
Expected: PASS including formatting, clippy, and the full test suite.

- [ ] **Step 6: Commit the external control surface slice**

```bash
git add src/api/player.rs src/daemon/server.rs src/cli/args.rs src/cli/client.rs src/cli/run.rs src/tui/app.rs src/tui/ui/playbar.rs tests/api_server.rs tests/cli_help.rs tests/cli_remote.rs tests/tui_app.rs
git commit -m "feat: expose playback volume and mode controls"
```

---

## Self-review notes

### Spec coverage

- volume / mute / repeat / shuffle 的共享契约：Task 1 + Task 2
- backend 只执行音量、不持有模式语义：Task 2
- repeat / shuffle 导航规则：Task 1 + Task 2
- HTTP / CLI / TUI 统一消费：Task 3

### Placeholder scan

- 没有遗留占位式描述
- 每个任务都包含具体测试、命令和提交信息

### Type consistency

- repeat 模式统一使用 `RepeatMode`
- 纯导航 helper 统一使用 `PlaybackNavigation`
- 快照字段统一使用 `volume_percent`、`muted`、`repeat_mode`、`shuffle_enabled`

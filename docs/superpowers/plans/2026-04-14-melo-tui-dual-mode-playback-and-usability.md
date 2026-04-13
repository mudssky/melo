# Melo TUI Dual-Mode Playback And Usability Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a lightweight daemon playback runtime flow plus a cached, optimistic TUI so `libmpv` playback controls feel responsive, lists auto-scroll with visible scrollbars, and lyrics support highlight/follow/manual browse without locking the future web client into daemon playback semantics.

**Architecture:** Split the current heavy `/api/ws/tui` aggregate path into a low-frequency bootstrap/data layer and a high-frequency playback runtime layer. Keep daemon playback as a dedicated remote-playback mode, add reusable `libmpv` session handling for lighter `next/prev`, and move TUI interaction onto a local cache + viewport model with optimistic commands and asynchronous reconciliation.

**Tech Stack:** Rust, Tokio, Axum, Ratatui, libmpv2, SeaORM, reqwest, tokio-tungstenite

---

## File Structure

**Create**

- `src/core/model/playback_mode.rs`
  Responsibility: expose user-facing playback mode enum plus projection to legacy `repeat + shuffle`.
- `src/core/model/playback_runtime.rs`
  Responsibility: lightweight runtime snapshot for daemon playback mode and bootstrap payload.
- `src/core/model/track_content.rs`
  Responsibility: shared low-frequency song content payload, parsed lyric lines, artwork summary metadata.
- `src/domain/library/lyrics.rs`
  Responsibility: parse raw lyrics text into timeline rows usable by TUI and future web clients.
- `src/api/bootstrap.rs`
  Responsibility: low-frequency bootstrap endpoint for remote-playback clients.
- `src/api/source.rs`
  Responsibility: list playable sources and tracks inside a source without carrying playback state.
- `src/api/track.rs`
  Responsibility: fetch and refresh low-frequency track content.
- `src/tui/viewports.rs`
  Responsibility: reusable viewport + scrollbar math for source list, track list, and lyrics panel.
- `src/tui/lyrics.rs`
  Responsibility: lyric follow/manual-browse state machine and current-line resolution.

**Modify**

- `src/core/model/mod.rs`
  Responsibility: register new shared model modules.
- `src/core/model/player.rs`
  Responsibility: keep legacy daemon snapshot working while exposing conversions into the new playback mode/runtime models.
- `src/core/config/settings.rs`
  Responsibility: add `player.default_mode` and `tui.lyrics_resume_delay_ms` settings with defaults.
- `src/api/mod.rs`
  Responsibility: export the new API modules.
- `src/api/ws.rs`
  Responsibility: add lightweight playback runtime websocket stream while keeping legacy TUI stream.
- `src/api/tui.rs`
  Responsibility: keep legacy endpoint as compatibility bootstrap during migration.
- `src/daemon/server.rs`
  Responsibility: register bootstrap/source/track/runtime routes.
- `src/daemon/app.rs`
  Responsibility: build bootstrap payloads, lightweight runtime snapshots, and stop using `tui_snapshot()` for high-frequency updates.
- `src/domain/library/service.rs`
  Responsibility: expose `track_content(song_id)` and `refresh_track_content(song_id)` helpers.
- `src/domain/library/repository.rs`
  Responsibility: fetch richer song details needed for shared content payloads.
- `src/domain/playlist/service.rs`
  Responsibility: expose source track listing with stable ordering.
- `src/domain/player/service.rs`
  Responsibility: project daemon player state into lightweight runtime snapshots and preserve user-facing playback mode.
- `src/domain/player/libmpv_backend.rs`
  Responsibility: reuse one `Mpv` instance across `next/prev` within daemon playback mode.
- `src/cli/client.rs`
  Responsibility: add typed bootstrap/source/track-content requests for the new shared data layer.
- `src/tui/client.rs`
  Responsibility: switch from monolithic TUI snapshot consumption to bootstrap + source/track/content queries + runtime stream.
- `src/tui/app.rs`
  Responsibility: replace ad-hoc fields with explicit caches, viewports, lyric state, and optimistic playback state.
- `src/tui/run.rs`
  Responsibility: make playback commands non-blocking, wire async result channels, and consume runtime deltas.
- `src/tui/ui/layout.rs`
  Responsibility: implement dual-center layout proportions.
- `src/tui/ui/playlist.rs`
  Responsibility: render scrollable source/track panes with unified scrollbar behavior.
- `src/tui/ui/details.rs`
  Responsibility: render lyrics as the primary panel with highlight/follow/manual-browse states.
- `src/tui/ui/playbar.rs`
  Responsibility: shrink bottom status to compact playback/runtime hints.

**Test**

- `src/core/model/playback_mode/tests.rs`
- `src/core/model/track_content/tests.rs`
- `src/domain/library/lyrics/tests.rs`
- `src/domain/player/libmpv_backend/tests.rs`
- `src/tui/run/tests.rs`
- `src/tui/app/tests.rs`
- `tests/api_server.rs`
- `tests/tui_app.rs`

## Task 1: Add Shared Playback Mode, Runtime, And Track Content Models

**Files:**
- Create: `src/core/model/playback_mode.rs`
- Create: `src/core/model/track_content.rs`
- Create: `src/core/model/playback_runtime.rs`
- Modify: `src/core/model/mod.rs`
- Modify: `src/core/model/player.rs`
- Modify: `src/core/config/settings.rs`
- Test: `src/core/model/playback_mode/tests.rs`
- Test: `src/core/model/track_content/tests.rs`

- [ ] **Step 1: Write the failing tests**

```rust
// src/core/model/playback_mode/tests.rs
use crate::core::model::playback_mode::PlaybackMode;
use crate::core::model::player::RepeatMode;

#[test]
fn playback_mode_projects_to_legacy_repeat_and_shuffle_flags() {
    let ordered = PlaybackMode::Ordered.project();
    assert_eq!(ordered.repeat_mode, RepeatMode::Off);
    assert!(!ordered.shuffle_enabled);
    assert!(!ordered.stop_after_current);

    let single = PlaybackMode::Single.project();
    assert_eq!(single.repeat_mode, RepeatMode::Off);
    assert!(!single.shuffle_enabled);
    assert!(single.stop_after_current);
}

#[test]
fn playback_mode_parses_config_strings() {
    assert_eq!(PlaybackMode::from_config("ordered").unwrap(), PlaybackMode::Ordered);
    assert_eq!(PlaybackMode::from_config("repeat_one").unwrap(), PlaybackMode::RepeatOne);
    assert_eq!(PlaybackMode::from_config("shuffle").unwrap(), PlaybackMode::Shuffle);
    assert_eq!(PlaybackMode::from_config("single").unwrap(), PlaybackMode::Single);
}

// src/core/model/track_content/tests.rs
use crate::core::model::track_content::{ArtworkSummary, LyricLine, TrackContentSnapshot};

#[test]
fn track_content_reports_current_line_for_runtime_position() {
    let content = TrackContentSnapshot {
        song_id: 7,
        title: "Blue Bird".into(),
        duration_seconds: Some(212.0),
        artwork: Some(ArtworkSummary {
            source_kind: "sidecar".into(),
            source_path: Some("D:/Music/cover.jpg".into()),
            terminal_summary: "Cover: sidecar".into(),
        }),
        lyrics: vec![
            LyricLine { timestamp_seconds: 1.0, text: "a".into() },
            LyricLine { timestamp_seconds: 5.0, text: "b".into() },
            LyricLine { timestamp_seconds: 9.0, text: "c".into() },
        ],
        refresh_token: "song-7-v1".into(),
    };

    assert_eq!(content.current_lyric_index(0.5), None);
    assert_eq!(content.current_lyric_index(1.1), Some(0));
    assert_eq!(content.current_lyric_index(5.2), Some(1));
    assert_eq!(content.current_lyric_index(12.0), Some(2));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test playback_mode_ --lib -- --nocapture`

Expected: FAIL with unresolved imports for `playback_mode` / `track_content`, plus missing `current_lyric_index()`.

- [ ] **Step 3: Write minimal implementation**

```rust
// src/core/model/playback_mode.rs
use serde::{Deserialize, Serialize};

use crate::core::error::{MeloError, MeloResult};
use crate::core::model::player::RepeatMode;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlaybackMode {
    Ordered,
    RepeatOne,
    Shuffle,
    Single,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlaybackModeProjection {
    pub repeat_mode: RepeatMode,
    pub shuffle_enabled: bool,
    pub stop_after_current: bool,
}

impl PlaybackMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ordered => "ordered",
            Self::RepeatOne => "repeat_one",
            Self::Shuffle => "shuffle",
            Self::Single => "single",
        }
    }

    pub fn from_config(value: &str) -> MeloResult<Self> {
        match value {
            "ordered" => Ok(Self::Ordered),
            "repeat_one" => Ok(Self::RepeatOne),
            "shuffle" => Ok(Self::Shuffle),
            "single" => Ok(Self::Single),
            other => Err(MeloError::Message(format!("invalid_playback_mode:{other}"))),
        }
    }

    pub fn project(self) -> PlaybackModeProjection {
        match self {
            Self::Ordered => PlaybackModeProjection {
                repeat_mode: RepeatMode::Off,
                shuffle_enabled: false,
                stop_after_current: false,
            },
            Self::RepeatOne => PlaybackModeProjection {
                repeat_mode: RepeatMode::One,
                shuffle_enabled: false,
                stop_after_current: false,
            },
            Self::Shuffle => PlaybackModeProjection {
                repeat_mode: RepeatMode::Off,
                shuffle_enabled: true,
                stop_after_current: false,
            },
            Self::Single => PlaybackModeProjection {
                repeat_mode: RepeatMode::Off,
                shuffle_enabled: false,
                stop_after_current: true,
            },
        }
    }
}

// src/core/model/track_content.rs
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtworkSummary {
    pub source_kind: String,
    pub source_path: Option<String>,
    pub terminal_summary: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LyricLine {
    pub timestamp_seconds: f64,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrackContentSnapshot {
    pub song_id: i64,
    pub title: String,
    pub duration_seconds: Option<f64>,
    pub artwork: Option<ArtworkSummary>,
    pub lyrics: Vec<LyricLine>,
    pub refresh_token: String,
}

impl TrackContentSnapshot {
    pub fn current_lyric_index(&self, position_seconds: f64) -> Option<usize> {
        self.lyrics
            .iter()
            .enumerate()
            .rev()
            .find(|(_, line)| line.timestamp_seconds <= position_seconds)
            .map(|(index, _)| index)
    }
}

// src/core/model/playback_runtime.rs
use serde::{Deserialize, Serialize};

use crate::core::model::playback_mode::PlaybackMode;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlaybackRuntimeSnapshot {
    pub generation: u64,
    pub playback_state: String,
    pub current_source_ref: Option<String>,
    pub current_song_id: Option<i64>,
    pub current_index: Option<usize>,
    pub position_seconds: Option<f64>,
    pub duration_seconds: Option<f64>,
    pub playback_mode: PlaybackMode,
    pub volume_percent: u8,
    pub muted: bool,
    pub last_error_code: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClientBootstrapSnapshot {
    pub runtime: PlaybackRuntimeSnapshot,
    pub default_playback_mode: PlaybackMode,
    pub current_source_ref: Option<String>,
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test playback_mode_ --lib -- --nocapture`

Expected: PASS for playback mode projection/parsing and `TrackContentSnapshot::current_lyric_index()`.

- [ ] **Step 5: Commit**

```bash
git add src/core/model/mod.rs src/core/model/player.rs src/core/model/playback_mode.rs src/core/model/playback_runtime.rs src/core/model/track_content.rs src/core/config/settings.rs src/core/model/playback_mode/tests.rs src/core/model/track_content/tests.rs
git commit -m "feat(model): add playback mode and track content snapshots"
```

## Task 2: Add Shared Source/Track Content APIs And Lyrics Parsing

**Files:**
- Create: `src/domain/library/lyrics.rs`
- Create: `src/api/bootstrap.rs`
- Create: `src/api/source.rs`
- Create: `src/api/track.rs`
- Modify: `src/api/mod.rs`
- Modify: `src/daemon/server.rs`
- Modify: `src/domain/library/service.rs`
- Modify: `src/domain/library/repository.rs`
- Modify: `src/domain/playlist/service.rs`
- Test: `src/domain/library/lyrics/tests.rs`
- Test: `tests/api_server.rs`

- [ ] **Step 1: Write the failing tests**

```rust
// src/domain/library/lyrics/tests.rs
use crate::domain::library::lyrics::parse_lyrics_timeline;

#[test]
fn parse_lyrics_timeline_extracts_lrc_tags_in_order() {
    let lines = parse_lyrics_timeline("[00:01.00]hello\n[00:05.50]world");
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0].timestamp_seconds, 1.0);
    assert_eq!(lines[0].text, "hello");
    assert_eq!(lines[1].timestamp_seconds, 5.5);
    assert_eq!(lines[1].text, "world");
}

#[test]
fn parse_lyrics_timeline_falls_back_to_plain_lines() {
    let lines = parse_lyrics_timeline("plain one\nplain two");
    assert_eq!(lines[0].timestamp_seconds, 0.0);
    assert_eq!(lines[1].timestamp_seconds, 1.0);
}

// tests/api_server.rs
#[tokio::test(flavor = "multi_thread")]
async fn bootstrap_endpoint_returns_runtime_and_default_mode() {
    let app = melo::daemon::app::test_router().await;
    let server = spawn_test_server(app).await;

    let response = reqwest::Client::new()
        .get(format!("{}/api/bootstrap", server.base_url))
        .send()
        .await
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap();

    assert_eq!(response["code"], 0);
    assert_eq!(response["data"]["default_playback_mode"], "ordered");
    assert!(response["data"]["runtime"]["playback_state"].is_string());
}

#[tokio::test(flavor = "multi_thread")]
async fn track_content_endpoint_returns_parsed_lyrics_and_artwork_summary() {
    let state = melo::daemon::app::AppState::for_test().await;
    let app = melo::daemon::server::router(state);
    let server = spawn_test_server(app).await;

    let response = reqwest::Client::new()
        .get(format!("{}/api/tracks/content?song_id=1", server.base_url))
        .send()
        .await
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap();

    assert_eq!(response["code"], 0);
    assert!(response["data"]["lyrics"].as_array().unwrap().len() >= 1);
    assert!(response["data"]["refresh_token"].is_string());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test parse_lyrics_timeline_ bootstrap_endpoint_returns_runtime_and_default_mode track_content_endpoint_returns_parsed_lyrics_and_artwork_summary -- --nocapture`

Expected: FAIL because the parser module, bootstrap route, and track content route do not exist yet.

- [ ] **Step 3: Write minimal implementation**

```rust
// src/domain/library/lyrics.rs
use crate::core::model::track_content::LyricLine;

pub fn parse_lyrics_timeline(raw: &str) -> Vec<LyricLine> {
    let mut parsed = Vec::new();
    for (plain_index, raw_line) in raw.lines().enumerate() {
        let mut matched = false;
        if let Some(rest) = raw_line.strip_prefix('[')
            && let Some((tag, text)) = rest.split_once(']')
            && let Some((mm, ss)) = tag.split_once(':')
            && let (Ok(minutes), Ok(seconds)) = (mm.parse::<f64>(), ss.parse::<f64>())
        {
            parsed.push(LyricLine {
                timestamp_seconds: minutes * 60.0 + seconds,
                text: text.to_string(),
            });
            matched = true;
        }
        if !matched {
            parsed.push(LyricLine {
                timestamp_seconds: plain_index as f64,
                text: raw_line.to_string(),
            });
        }
    }
    parsed.sort_by(|left, right| left.timestamp_seconds.partial_cmp(&right.timestamp_seconds).unwrap());
    parsed
}

// src/api/bootstrap.rs
use axum::{Json, extract::State};

use crate::api::error::ApiError;
use crate::api::response::ApiResponse;
use crate::daemon::app::AppState;

pub async fn show(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<crate::core::model::playback_runtime::ClientBootstrapSnapshot>>, ApiError> {
    state.client_bootstrap().await.map(ApiResponse::ok).map(Json).map_err(ApiError::from)
}

// src/api/source.rs
pub async fn list(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Vec<crate::core::model::tui::PlaylistListItem>>>, ApiError> {
    state.source_list().await.map(ApiResponse::ok).map(Json).map_err(ApiError::from)
}

pub async fn tracks(
    State(state): State<AppState>,
    Query(query): Query<SourceTracksQuery>,
) -> Result<Json<ApiResponse<Vec<crate::core::model::player::QueueItem>>>, ApiError> {
    state.source_tracks(&query.name).await.map(ApiResponse::ok).map(Json).map_err(ApiError::from)
}

// src/api/track.rs
pub async fn content(
    State(state): State<AppState>,
    Query(query): Query<TrackContentQuery>,
) -> Result<Json<ApiResponse<crate::core::model::track_content::TrackContentSnapshot>>, ApiError> {
    state.track_content(query.song_id).await.map(ApiResponse::ok).map(Json).map_err(ApiError::from)
}

pub async fn refresh(
    State(state): State<AppState>,
    Query(query): Query<TrackContentQuery>,
) -> Result<Json<ApiResponse<crate::core::model::track_content::TrackContentSnapshot>>, ApiError> {
    state.refresh_track_content(query.song_id).await.map(ApiResponse::ok).map(Json).map_err(ApiError::from)
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test parse_lyrics_timeline_ bootstrap_endpoint_returns_runtime_and_default_mode track_content_endpoint_returns_parsed_lyrics_and_artwork_summary -- --nocapture`

Expected: PASS with parsed lyric lines and both content/bootstrap endpoints returning `code = 0`.

- [ ] **Step 5: Commit**

```bash
git add src/domain/library/lyrics.rs src/api/bootstrap.rs src/api/source.rs src/api/track.rs src/api/mod.rs src/daemon/server.rs src/domain/library/service.rs src/domain/library/repository.rs src/domain/playlist/service.rs src/domain/library/lyrics/tests.rs tests/api_server.rs
git commit -m "feat(api): add client bootstrap and content endpoints"
```

## Task 3: Add Lightweight Daemon Playback Runtime Streaming

**Files:**
- Modify: `src/api/ws.rs`
- Modify: `src/daemon/app.rs`
- Modify: `src/domain/player/service.rs`
- Modify: `src/core/model/playback_runtime.rs`
- Test: `tests/api_server.rs`

- [ ] **Step 1: Write the failing tests**

```rust
// tests/api_server.rs
#[tokio::test(flavor = "multi_thread")]
async fn playback_runtime_ws_streams_lightweight_updates() {
    let state = melo::daemon::app::AppState::for_test().await;
    state.player.append(melo::core::model::player::QueueItem {
        song_id: 1,
        path: "tests/fixtures/full_test.mp3".into(),
        title: "Blue Bird".into(),
        duration_seconds: Some(212.0),
    }).await.unwrap();

    let app = melo::daemon::server::router(state);
    let server = spawn_test_server(app).await;
    let ws_url = server.base_url.replace("http://", "ws://") + "/api/ws/playback/runtime";
    let (_socket, mut stream) = connect_test_ws(&ws_url).await;

    let initial: serde_json::Value = next_ws_json(&mut stream).await;
    assert!(initial["current_song_id"].is_null());
    assert!(initial.get("lyrics").is_none());
    assert!(initial.get("visible_playlists").is_none());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test playback_runtime_ws_streams_lightweight_updates --test api_server -- --nocapture`

Expected: FAIL with 404 or websocket path missing.

- [ ] **Step 3: Write minimal implementation**

```rust
// src/domain/player/service.rs
impl PlayerService {
    pub async fn runtime_snapshot(
        &self,
        current_source_ref: Option<String>,
    ) -> crate::core::model::playback_runtime::PlaybackRuntimeSnapshot {
        let snapshot = self.snapshot().await;
        crate::core::model::playback_runtime::PlaybackRuntimeSnapshot {
            generation: snapshot.version,
            playback_state: snapshot.playback_state,
            current_source_ref,
            current_song_id: snapshot.current_song.as_ref().map(|song| song.song_id),
            current_index: snapshot.queue_index,
            position_seconds: snapshot.position_seconds,
            duration_seconds: snapshot.current_song.as_ref().and_then(|song| song.duration_seconds),
            playback_mode: crate::core::model::playback_mode::PlaybackMode::from_runtime_fields(&snapshot.repeat_mode, snapshot.shuffle_enabled),
            volume_percent: snapshot.volume_percent,
            muted: snapshot.muted,
            last_error_code: snapshot.last_error.as_ref().map(|err| err.code.clone()),
        }
    }
}

// src/api/ws.rs
pub async fn playback_runtime_updates(socket: WebSocketUpgrade, State(state): State<AppState>) -> Response {
    socket.on_upgrade(move |websocket| stream_playback_runtime(websocket, state))
}

async fn stream_playback_runtime(mut socket: WebSocket, state: AppState) {
    let mut receiver = state.player.subscribe();
    let initial = state.playback_runtime_snapshot().await;
    if send_payload(&mut socket, &initial).await.is_err() {
        return;
    }

    while receiver.changed().await.is_ok() {
        let payload = state.playback_runtime_snapshot().await;
        if send_payload(&mut socket, &payload).await.is_err() {
            break;
        }
    }
}

// src/daemon/app.rs
impl AppState {
    pub async fn playback_runtime_snapshot(&self) -> crate::core::model::playback_runtime::PlaybackRuntimeSnapshot {
        self.player.runtime_snapshot(self.current_playlist_context().map(|ctx| ctx.name)).await
    }

    pub async fn client_bootstrap(&self) -> MeloResult<crate::core::model::playback_runtime::ClientBootstrapSnapshot> {
        Ok(crate::core::model::playback_runtime::ClientBootstrapSnapshot {
            runtime: self.playback_runtime_snapshot().await,
            default_playback_mode: crate::core::model::playback_mode::PlaybackMode::Ordered,
            current_source_ref: self.current_playlist_context().map(|ctx| ctx.name),
        })
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test playback_runtime_ws_streams_lightweight_updates --test api_server -- --nocapture`

Expected: PASS with a websocket payload containing playback runtime fields only.

- [ ] **Step 5: Commit**

```bash
git add src/api/ws.rs src/daemon/app.rs src/domain/player/service.rs src/core/model/playback_runtime.rs tests/api_server.rs
git commit -m "feat(player): add lightweight playback runtime stream"
```

## Task 4: Reuse `libmpv` Sessions For Context-Local `next/prev`

**Files:**
- Modify: `src/domain/player/libmpv_backend.rs`
- Test: `src/domain/player/libmpv_backend/tests.rs`

- [ ] **Step 1: Write the failing tests**

```rust
// src/domain/player/libmpv_backend/tests.rs
#[test]
fn libmpv_backend_reuses_driver_for_sequential_loads() {
    let driver = super::tests::FakeLibmpvDriver::default();
    let backend = super::LibmpvBackend::new_for_test_with_driver(driver.clone());

    backend.start_session(play_request("A.flac", 1)).unwrap();
    backend.start_session(play_request("B.flac", 2)).unwrap();

    assert_eq!(driver.created_instances(), 1);
    assert_eq!(driver.loadfile_calls(), vec!["A.flac", "B.flac"]);
}

#[test]
fn libmpv_backend_stop_keeps_driver_alive_for_future_reload() {
    let driver = super::tests::FakeLibmpvDriver::default();
    let backend = super::LibmpvBackend::new_for_test_with_driver(driver.clone());

    let session = backend.start_session(play_request("A.flac", 1)).unwrap();
    session.stop().unwrap();
    backend.start_session(play_request("B.flac", 2)).unwrap();

    assert_eq!(driver.created_instances(), 1);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test libmpv_backend_reuses_driver_for_sequential_loads --lib -- --nocapture`

Expected: FAIL because `LibmpvBackend` has no reusable driver abstraction and recreates `Mpv` every time.

- [ ] **Step 3: Write minimal implementation**

```rust
// src/domain/player/libmpv_backend.rs
trait LibmpvDriver: Send {
    fn loadfile_replace(&mut self, path: &std::path::Path) -> MeloResult<()>;
    fn set_pause(&mut self, paused: bool) -> MeloResult<()>;
    fn set_volume(&mut self, volume_percent: f64) -> MeloResult<()>;
    fn stop(&mut self) -> MeloResult<()>;
    fn current_position(&mut self) -> Option<Duration>;
}

struct RealLibmpvDriver {
    mpv: Mpv,
}

impl LibmpvDriver for RealLibmpvDriver {
    fn loadfile_replace(&mut self, path: &std::path::Path) -> MeloResult<()> {
        self.mpv
            .command("loadfile", &[path.to_string_lossy().as_ref(), "replace"])
            .map_err(|err| MeloError::Message(err.to_string()))
    }
}

pub struct LibmpvBackend {
    driver: Arc<Mutex<Box<dyn LibmpvDriver>>>,
    runtime_tx: broadcast::Sender<PlaybackRuntimeEvent>,
}

impl PlaybackBackend for LibmpvBackend {
    fn start_session(&self, request: PlaybackStartRequest) -> MeloResult<Box<dyn PlaybackSessionHandle>> {
        {
            let mut driver = self.driver.lock().unwrap();
            driver.set_volume(f64::from(request.volume_factor.max(0.0) * 100.0))?;
            driver.set_pause(false)?;
            driver.loadfile_replace(&request.path)?;
        }

        Ok(Box::new(LibmpvPlaybackSession {
            driver: Arc::clone(&self.driver),
            runtime_tx: self.runtime_tx.clone(),
        }))
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test libmpv_backend_reuses_driver_for_sequential_loads libmpv_backend_stop_keeps_driver_alive_for_future_reload --lib -- --nocapture`

Expected: PASS with a single fake driver instance reused across `next/prev`-style reloads.

- [ ] **Step 5: Commit**

```bash
git add src/domain/player/libmpv_backend.rs src/domain/player/libmpv_backend/tests.rs
git commit -m "feat(player): reuse libmpv session for sequential loads"
```

## Task 5: Refactor TUI Client And App State Around Cache + Viewport Models

**Files:**
- Create: `src/tui/viewports.rs`
- Create: `src/tui/lyrics.rs`
- Modify: `src/tui/client.rs`
- Modify: `src/tui/app.rs`
- Test: `src/tui/app/tests.rs`
- Test: `tests/tui_app.rs`

- [ ] **Step 1: Write the failing tests**

```rust
// src/tui/app/tests.rs
#[test]
fn viewport_scrolls_when_selected_item_moves_below_visible_window() {
    let mut viewport = crate::tui::viewports::ViewportState::new(4);
    viewport.follow_selection(6, 12);
    assert_eq!(viewport.scroll_top, 3);
}

#[test]
fn lyric_follow_state_pauses_and_resumes_after_timeout() {
    use std::time::{Duration, Instant};

    let now = Instant::now();
    let mut state = crate::tui::lyrics::LyricFollowState::new(Duration::from_secs(3));
    state.pause_for_manual_scroll(now);
    assert!(state.is_manual_browse());
    assert!(!state.should_resume(now + Duration::from_secs(2)));
    assert!(state.should_resume(now + Duration::from_secs(3)));
}

// tests/tui_app.rs
#[test]
fn app_updates_current_lyric_highlight_from_runtime_position() {
    let mut app = melo::tui::app::App::new_for_test();
    app.current_track_song_id = Some(7);
    app.cache_track_content(melo::core::model::track_content::TrackContentSnapshot {
        song_id: 7,
        title: "Blue Bird".into(),
        duration_seconds: Some(212.0),
        artwork: None,
        lyrics: vec![
            melo::core::model::track_content::LyricLine { timestamp_seconds: 1.0, text: "a".into() },
            melo::core::model::track_content::LyricLine { timestamp_seconds: 3.0, text: "b".into() },
        ],
        refresh_token: "7-v1".into(),
    });

    app.apply_runtime_snapshot(melo::core::model::playback_runtime::PlaybackRuntimeSnapshot {
        generation: 2,
        playback_state: "playing".into(),
        current_source_ref: Some("Favorites".into()),
        current_song_id: Some(7),
        current_index: Some(0),
        position_seconds: Some(3.2),
        duration_seconds: Some(212.0),
        playback_mode: melo::core::model::playback_mode::PlaybackMode::Ordered,
        volume_percent: 100,
        muted: false,
        last_error_code: None,
    });

    assert_eq!(app.current_lyric_index(), Some(1));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test viewport_scrolls_when_selected_item_moves_below_visible_window lyric_follow_state_pauses_and_resumes_after_timeout app_updates_current_lyric_highlight_from_runtime_position -- --nocapture`

Expected: FAIL because viewport helpers, lyric follow state, and cached runtime application APIs do not exist.

- [ ] **Step 3: Write minimal implementation**

```rust
// src/tui/viewports.rs
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViewportState {
    pub visible_height: usize,
    pub scroll_top: usize,
}

impl ViewportState {
    pub fn new(visible_height: usize) -> Self {
        Self { visible_height, scroll_top: 0 }
    }

    pub fn follow_selection(&mut self, selected_index: usize, item_count: usize) {
        if selected_index < self.scroll_top {
            self.scroll_top = selected_index;
        } else if selected_index >= self.scroll_top + self.visible_height {
            self.scroll_top = selected_index + 1 - self.visible_height;
        }
        self.scroll_top = self.scroll_top.min(item_count.saturating_sub(self.visible_height));
    }
}

// src/tui/lyrics.rs
#[derive(Debug, Clone)]
pub struct LyricFollowState {
    resume_delay: std::time::Duration,
    paused_at: Option<std::time::Instant>,
}

impl LyricFollowState {
    pub fn new(resume_delay: std::time::Duration) -> Self {
        Self { resume_delay, paused_at: None }
    }

    pub fn pause_for_manual_scroll(&mut self, now: std::time::Instant) {
        self.paused_at = Some(now);
    }

    pub fn is_manual_browse(&self) -> bool {
        self.paused_at.is_some()
    }

    pub fn should_resume(&self, now: std::time::Instant) -> bool {
        self.paused_at.is_some_and(|paused_at| now.duration_since(paused_at) >= self.resume_delay)
    }
}

// src/tui/app.rs
pub struct App {
    pub runtime: crate::core::model::playback_runtime::PlaybackRuntimeSnapshot,
    pub source_viewport: crate::tui::viewports::ViewportState,
    pub track_viewport: crate::tui::viewports::ViewportState,
    pub lyric_viewport: crate::tui::viewports::ViewportState,
    pub lyric_follow_state: crate::tui::lyrics::LyricFollowState,
    pub track_content_cache: std::collections::BTreeMap<i64, crate::core::model::track_content::TrackContentSnapshot>,
}

impl App {
    pub fn cache_track_content(&mut self, content: crate::core::model::track_content::TrackContentSnapshot) {
        self.track_content_cache.insert(content.song_id, content);
    }

    pub fn apply_runtime_snapshot(&mut self, runtime: crate::core::model::playback_runtime::PlaybackRuntimeSnapshot) {
        self.current_track_song_id = runtime.current_song_id;
        self.runtime = runtime;
    }

    pub fn current_lyric_index(&self) -> Option<usize> {
        let song_id = self.current_track_song_id?;
        let content = self.track_content_cache.get(&song_id)?;
        let position = self.runtime.position_seconds?;
        content.current_lyric_index(position)
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test viewport_scrolls_when_selected_item_moves_below_visible_window lyric_follow_state_pauses_and_resumes_after_timeout app_updates_current_lyric_highlight_from_runtime_position -- --nocapture`

Expected: PASS for viewport following, lyric timeout handling, and runtime-based lyric highlight.

- [ ] **Step 5: Commit**

```bash
git add src/tui/viewports.rs src/tui/lyrics.rs src/tui/client.rs src/tui/app.rs src/tui/app/tests.rs tests/tui_app.rs
git commit -m "feat(tui): add cached runtime state and viewports"
```

## Task 6: Make TUI Commands Non-Blocking And Render The Dual-Center Layout

**Files:**
- Modify: `src/tui/run.rs`
- Modify: `src/tui/ui/layout.rs`
- Modify: `src/tui/ui/playlist.rs`
- Modify: `src/tui/ui/details.rs`
- Modify: `src/tui/ui/playbar.rs`
- Test: `src/tui/run/tests.rs`
- Test: `tests/tui_app.rs`

- [ ] **Step 1: Write the failing tests**

```rust
// src/tui/run/tests.rs
#[tokio::test]
async fn dispatching_next_returns_before_remote_confirmation() {
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let mut app = crate::tui::app::App::new_for_test();

    crate::tui::run::enqueue_runtime_command(
        &mut app,
        crate::tui::event::ActionId::Next,
        &tx,
    );

    assert!(app.pending_runtime_action().is_some());
    assert!(rx.try_recv().is_ok());
}

// tests/tui_app.rs
#[test]
fn lyrics_panel_renders_highlight_and_scrollbar() {
    let mut app = melo::tui::app::App::new_for_test();
    app.load_fake_lyrics_panel_for_test();
    let lines = melo::tui::ui::details::render_detail_lines(&app);

    assert!(lines.iter().any(|line| line.contains("[current]")));
    assert!(lines.iter().any(|line| line.contains("│")));
}

#[test]
fn playlist_rows_truncate_long_titles_but_keep_selected_row_visible() {
    let mut app = melo::tui::app::App::new_for_test();
    app.load_fake_track_list_for_test(20);
    app.select_preview_index(12);
    app.sync_viewports_for_test(6);

    let rows = melo::tui::ui::playlist::render_preview_lines(&app);
    assert!(rows.len() >= 6);
    assert!(rows.iter().any(|line| line.contains("…")));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test dispatching_next_returns_before_remote_confirmation lyrics_panel_renders_highlight_and_scrollbar playlist_rows_truncate_long_titles_but_keep_selected_row_visible -- --nocapture`

Expected: FAIL because the command queue, lyric scrollbar rendering, and auto-follow viewport rendering do not exist.

- [ ] **Step 3: Write minimal implementation**

```rust
// src/tui/run.rs
pub(crate) fn enqueue_runtime_command(
    app: &mut crate::tui::app::App,
    action: crate::tui::event::ActionId,
    tx: &tokio::sync::mpsc::UnboundedSender<crate::tui::runtime::RuntimeCommand>,
) {
    app.mark_pending_runtime_action(action);
    let _ = tx.send(crate::tui::runtime::RuntimeCommand::from_action(action));
}

// inside run_loop()
let (command_tx, mut command_rx) = tokio::sync::mpsc::unbounded_channel();
tokio::spawn({
    let api_client = api_client.clone();
    async move {
        while let Some(command) = command_rx.recv().await {
            let result = command.execute(&api_client).await;
            let _ = result_tx.send(result);
        }
    }
});

// src/tui/ui/layout.rs
pub fn split(area: ratatui::layout::Rect, has_task_bar: bool) -> AppLayout {
    let [header, body, footer] = ratatui::layout::Layout::vertical([
        ratatui::layout::Constraint::Length(if has_task_bar { 2 } else { 0 }),
        ratatui::layout::Constraint::Min(10),
        ratatui::layout::Constraint::Length(3),
    ]).areas(area);

    let [left, right] = ratatui::layout::Layout::horizontal([
        ratatui::layout::Constraint::Percentage(51),
        ratatui::layout::Constraint::Percentage(49),
    ]).areas(body);

    // left = sources + tracks, right = now playing + lyrics + artwork
}

// src/tui/ui/details.rs
pub fn render_detail_lines(app: &crate::tui::app::App) -> Vec<String> {
    let mut lines = Vec::new();
    for (visible_row, lyric) in app.visible_lyrics_with_scrollbar() {
        let prefix = if lyric.is_current { "[current]" } else { "         " };
        lines.push(format!("{prefix} {} │", lyric.text));
    }
    lines
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test dispatching_next_returns_before_remote_confirmation lyrics_panel_renders_highlight_and_scrollbar playlist_rows_truncate_long_titles_but_keep_selected_row_visible -- --nocapture`

Expected: PASS with non-blocking command dispatch and visible scrollbar/highlight markers in rendered panels.

- [ ] **Step 5: Commit**

```bash
git add src/tui/run.rs src/tui/ui/layout.rs src/tui/ui/playlist.rs src/tui/ui/details.rs src/tui/ui/playbar.rs src/tui/run/tests.rs tests/tui_app.rs
git commit -m "feat(tui): add optimistic commands and dual-center layout"
```

## Task 7: Wire End-To-End Runtime Reconciliation And Full Regression Coverage

**Files:**
- Modify: `src/cli/client.rs`
- Modify: `src/tui/client.rs`
- Modify: `src/tui/run.rs`
- Modify: `src/daemon/app.rs`
- Modify: `src/api/tui.rs`
- Test: `tests/api_server.rs`
- Test: `tests/tui_app.rs`
- Test: `src/tui/run/tests.rs`

- [ ] **Step 1: Write the failing tests**

```rust
// tests/api_server.rs
#[tokio::test(flavor = "multi_thread")]
async fn runtime_snapshot_does_not_recompute_lyrics_or_playlist_browser() {
    let state = melo::daemon::app::AppState::for_test().await;
    state.player.append(melo::core::model::player::QueueItem {
        song_id: 1,
        path: "tests/fixtures/full_test.mp3".into(),
        title: "Blue Bird".into(),
        duration_seconds: Some(212.0),
    }).await.unwrap();

    let before = state.client_bootstrap().await.unwrap();
    state.player.play().await.unwrap();
    let after = state.playback_runtime_snapshot().await;

    assert_eq!(before.current_source_ref, after.current_source_ref);
    assert!(after.current_song_id.is_some());
}

// src/tui/run/tests.rs
#[tokio::test]
async fn runtime_delta_clears_pending_action_and_refreshes_local_playback_state() {
    let mut app = crate::tui::app::App::new_for_test();
    app.mark_pending_runtime_action(crate::tui::event::ActionId::Next);

    crate::tui::run::apply_runtime_delta_for_test(
        &mut app,
        crate::core::model::playback_runtime::PlaybackRuntimeSnapshot {
            generation: 9,
            playback_state: "playing".into(),
            current_source_ref: Some("Favorites".into()),
            current_song_id: Some(8),
            current_index: Some(1),
            position_seconds: Some(0.0),
            duration_seconds: Some(180.0),
            playback_mode: crate::core::model::playback_mode::PlaybackMode::Ordered,
            volume_percent: 100,
            muted: false,
            last_error_code: None,
        },
    );

    assert!(app.pending_runtime_action().is_none());
    assert_eq!(app.current_track_song_id, Some(8));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test runtime_snapshot_does_not_recompute_lyrics_or_playlist_browser runtime_delta_clears_pending_action_and_refreshes_local_playback_state -- --nocapture`

Expected: FAIL because runtime reconciliation helpers are missing and the runtime/bootstrap split is not fully wired yet.

- [ ] **Step 3: Write minimal implementation**

```rust
// src/cli/client.rs
impl ApiClient {
    pub async fn bootstrap(
        &self,
    ) -> MeloResult<crate::core::model::playback_runtime::ClientBootstrapSnapshot> {
        let url = format!("{}/api/bootstrap", self.base_url);
        self.send_and_decode(self.client.get(url)).await
    }
}

// src/tui/client.rs
impl TuiClient {
    pub async fn bootstrap(
        &self,
        api_client: &crate::cli::client::ApiClient,
    ) -> MeloResult<crate::core::model::playback_runtime::ClientBootstrapSnapshot> {
        api_client.bootstrap().await
    }

    pub async fn runtime_connect(&self) -> MeloResult<crate::tui::ws_client::WsSnapshotStream> {
        let runtime_ws_url = self.runtime_ws_url.clone();
        crate::tui::ws_client::WsClient::new(runtime_ws_url).connect().await
    }
}

// src/tui/run.rs
fn apply_runtime_delta(
    app: &mut crate::tui::app::App,
    runtime: crate::core::model::playback_runtime::PlaybackRuntimeSnapshot,
) {
    app.clear_pending_runtime_action();
    app.apply_runtime_snapshot(runtime);
    app.recenter_lyrics_if_following();
    app.sync_visible_rows();
}

// src/api/tui.rs
pub async fn home(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<crate::core::model::tui::TuiSnapshot>>, ApiError> {
    state
        .legacy_tui_snapshot()
        .await
        .map(ApiResponse::ok)
        .map(Json)
        .map_err(ApiError::from)
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test runtime_snapshot_does_not_recompute_lyrics_or_playlist_browser runtime_delta_clears_pending_action_and_refreshes_local_playback_state -- --nocapture`

Expected: PASS with pending optimistic commands cleared by runtime confirmation and legacy TUI bootstrap still intact.

- [ ] **Step 5: Commit**

```bash
git add src/cli/client.rs src/tui/client.rs src/tui/run.rs src/daemon/app.rs src/api/tui.rs tests/api_server.rs src/tui/run/tests.rs
git commit -m "test(tui): add runtime reconciliation regressions"
```

## Final Verification

- [ ] Run targeted Rust tests for the new shared models, runtime stream, `libmpv` reuse, and TUI rendering:

```bash
cargo test playback_mode_ parse_lyrics_timeline_ playback_runtime_ws_ libmpv_backend_reuses_driver_for_sequential_loads viewport_scrolls_when_selected_item_moves_below_visible_window dispatching_next_returns_before_remote_confirmation -- --nocapture
```

Expected: All targeted tests PASS.

- [ ] Run the full Rust verification required by the repo:

```bash
pnpm qa
```

Expected: `format`, `clippy`, Rust tests, TypeScript checks, and Vitest all PASS.

- [ ] Manually smoke-test daemon playback mode in a real terminal:

```bash
cargo run -- daemon run
cargo run -- tui
```

Expected:
- `next/prev/space` feel non-blocking
- source and track lists auto-scroll with visible scrollbars
- lyrics highlight the current line, pause auto-follow on manual scroll, and resume after 3 seconds
- `libmpv` mode no longer stutters on ordinary in-context `next/prev`

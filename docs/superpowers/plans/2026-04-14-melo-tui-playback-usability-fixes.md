# Melo TUI Playback Usability Fixes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 让 Melo TUI 真正落到“高频 runtime 真相 + 低频内容查询 + 本地展示态”三层模型上，修复切歌热路径卡顿、时间/歌词不跟随、封面来源表达不稳定、预览增长不刷新的问题，同时保持 `--verbose` 进入 TUI 后不再污染 alternate screen。

**Architecture:** 保留现有 `/api/tui/home` 与 `/api/ws/tui` 兼容链路，但 TUI 主循环不再依赖聚合 WebSocket 驱动热路径，而改成一次性 bootstrap + 轻量 playback runtime WS + 周期性低频 home 刷新。播放命令提交与确认拆开：`Enter` 只提交轻量播放命令，当前播放标题/时间等展示只在 runtime 确认后切换；歌词、封面、预览滚动、平滑时间全部收敛到 TUI 本地状态机。

**Tech Stack:** Rust, Tokio, Axum, Ratatui, Crossterm, reqwest, tokio-tungstenite, SeaORM

---

## File Structure

### Hot path and API contracts

- Modify: `src/api/playlist.rs`
  - Responsibility: 保留旧 `/api/playlists/play` 兼容行为，同时新增轻量播放命令入口供 TUI 热路径使用。
- Modify: `src/cli/client.rs`
  - Responsibility: 为轻量播放命令、低频 home 刷新、track content 请求提供显式客户端方法。
- Modify: `src/daemon/app.rs`
  - Responsibility: 提供轻量播放命令所需的来源/目标歌曲确认数据，避免切歌热路径回落到整页聚合快照。
- Modify: `tests/api_server.rs`
  - Responsibility: 验证轻量播放命令响应不再携带低频内容，runtime WS 负责最终确认。

### Local TUI state and clock

- Create: `src/tui/playback_clock.rs`
  - Responsibility: 管理本地时间锚点、平滑推进、纠偏和冻结规则。
- Create: `src/tui/playback_clock/tests.rs`
  - Responsibility: 单元测试 playing/paused/error 下的本地时间行为和纠偏阈值。
- Modify: `src/tui/lyrics.rs`
  - Responsibility: 将歌词跟随状态扩展成 `FollowCurrent` / `ManualBrowse` / `ResumePending` 显式状态机。
- Modify: `src/tui/app.rs`
  - Responsibility: 管理待确认切歌、本地时间锚点、歌词跟随、低频内容缓存、预览增长检测与回放。
- Modify: `src/tui/app/tests.rs`
  - Responsibility: 覆盖运行时确认、预览数量增长、歌词手动浏览与恢复、封面摘要优先级。

### Runtime wiring and low-frequency refresh

- Modify: `src/tui/client.rs`
  - Responsibility: 暴露轻量播放命令、周期性 home 刷新和按 song_id 拉取 track content 的调用。
- Modify: `src/tui/run.rs`
  - Responsibility: 去掉对聚合 TUI WS 的热路径依赖，改成 runtime WS + home polling + track content 异步刷新；`Enter` 只提交轻量播放命令并等待 runtime 确认。
- Modify: `src/tui/run/tests.rs`
  - Responsibility: 覆盖非阻塞切歌、runtime 确认前不切歌、确认后清理 pending、verbose/TUI 边界回归。
- Modify: `tests/tui_app.rs`
  - Responsibility: 覆盖平滑时间、歌词当前句高亮、封面来源文案、预览滚动和补扫增长。
- Modify: `tests/cli_remote.rs`
  - Responsibility: 保护 `--verbose` 在进入 TUI 前有日志、进入 alternate screen 后不再继续镜像到当前终端。

### Cover and low-frequency content rendering

- Modify: `src/domain/library/service.rs`
  - Responsibility: 将封面摘要标准化为 `sidecar` / `embedded` / `none` 三种稳定用户可见状态。
- Modify: `src/core/model/track_content.rs`
  - Responsibility: 确保 track content 能稳定表达无封面状态和歌词时间轴行为。
- Modify: `src/tui/ui/details.rs`
  - Responsibility: 使用本地显示时间驱动歌词高亮与视口，显示封面来源/路径/无封面 fallback。
- Modify: `src/tui/ui/playlist.rs`
  - Responsibility: 使用统一视口状态渲染可滚动预览，保持长列表可访问。
- Modify: `src/tui/ui/playbar.rs`
  - Responsibility: 改为消费本地平滑时间与 pending 状态，而不是直接依赖旧 `PlayerSnapshot.position_seconds`。

## Task 1: Add Lightweight Playlist Play Command And Runtime Confirmation Contract

**Files:**
- Modify: `src/api/playlist.rs`
- Modify: `src/cli/client.rs`
- Modify: `src/daemon/app.rs`
- Modify: `tests/api_server.rs`

- [ ] **Step 1: Write the failing API and runtime confirmation tests**

Add this to `tests/api_server.rs`:

```rust
#[tokio::test(flavor = "multi_thread")]
async fn playlist_play_command_returns_lightweight_ack_without_tui_snapshot() {
    let harness = melo::test_support::TestHarness::new().await;
    let playlist_service = harness.playlist_service();
    harness.seed_song("One", "Aimer", "Singles", 2015).await;
    harness.seed_song("Two", "Aimer", "Singles", 2015).await;
    let one_path = harness.write_song_file("audio/one.flac").await;
    let two_path = harness.write_song_file("audio/two.flac").await;
    let conn = rusqlite::Connection::open(harness.settings.database.path.as_std_path()).unwrap();
    conn.execute(
        "UPDATE songs SET path = ?1 WHERE id = 1",
        [one_path.to_string_lossy().to_string()],
    )
    .unwrap();
    conn.execute(
        "UPDATE songs SET path = ?1 WHERE id = 2",
        [two_path.to_string_lossy().to_string()],
    )
    .unwrap();
    playlist_service.create_static("Favorites", None).await.unwrap();
    playlist_service.add_songs("Favorites", &[1, 2]).await.unwrap();

    let app = melo::daemon::app::test_router_with_settings(harness.settings.clone()).await;
    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/api/playlists/play-command")
                .header("content-type", "application/json")
                .body(axum::body::Body::from(r#"{"name":"Favorites","start_index":1}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), axum::http::StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(payload["code"], 0);
    assert_eq!(payload["data"]["target_song_id"], 2);
    assert!(payload["data"]["player"].is_null());
    assert!(payload["data"]["playlist_browser"].is_null());
}

#[tokio::test(flavor = "multi_thread")]
async fn runtime_ws_confirms_playlist_play_command_after_submission() {
    let harness = melo::test_support::TestHarness::new().await;
    let playlist_service = harness.playlist_service();
    harness.seed_song("One", "Aimer", "Singles", 2015).await;
    harness.seed_song("Two", "Aimer", "Singles", 2015).await;
    let one_path = harness.write_song_file("audio/one.flac").await;
    let two_path = harness.write_song_file("audio/two.flac").await;
    let conn = rusqlite::Connection::open(harness.settings.database.path.as_std_path()).unwrap();
    conn.execute(
        "UPDATE songs SET path = ?1 WHERE id = 1",
        [one_path.to_string_lossy().to_string()],
    )
    .unwrap();
    conn.execute(
        "UPDATE songs SET path = ?1 WHERE id = 2",
        [two_path.to_string_lossy().to_string()],
    )
    .unwrap();
    playlist_service.create_static("Favorites", None).await.unwrap();
    playlist_service.add_songs("Favorites", &[1, 2]).await.unwrap();

    let state = melo::daemon::app::AppState::for_test_with_settings(harness.settings.clone()).await;
    let app = melo::daemon::server::router(state);
    let server = spawn_test_server(app).await;
    let ws_url = server.base_url.replace("http://", "ws://") + "/api/ws/playback/runtime";
    let (_socket, mut stream) = connect_test_ws(&ws_url).await;
    let _initial: serde_json::Value = next_ws_json(&mut stream).await;

    let response = reqwest::Client::new()
        .post(format!("{}/api/playlists/play-command", server.base_url))
        .json(&serde_json::json!({ "name": "Favorites", "start_index": 1 }))
        .send()
        .await
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap();

    let confirmed = next_ws_json(&mut stream).await;
    assert_eq!(response["data"]["target_song_id"], 2);
    assert_eq!(confirmed["current_song_id"], 2);
    assert_eq!(confirmed["playback_state"], "playing");
}
```

- [ ] **Step 2: Run the tests to confirm they fail**

Run: `cargo test playlist_play_command_returns_lightweight_ack_without_tui_snapshot runtime_ws_confirms_playlist_play_command_after_submission --test api_server -- --nocapture`

Expected: FAIL because `/api/playlists/play-command` does not exist yet.

- [ ] **Step 3: Add the lightweight command endpoint and client method**

Update `src/api/playlist.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PlaylistPlayCommandResponse {
    pub source_name: String,
    pub source_kind: String,
    pub target_song_id: i64,
    pub target_index: usize,
    pub accepted_generation: u64,
}

pub async fn play_command(
    State(state): State<AppState>,
    Json(request): Json<PlaylistPlayRequest>,
) -> Result<Json<ApiResponse<PlaylistPlayCommandResponse>>, ApiError> {
    state.submit_playlist_play_command(&request.name, request.start_index)
        .await
        .map(ApiResponse::ok)
        .map(Json)
        .map_err(ApiError::from)
}
```

Update `src/daemon/app.rs` with a helper that resolves the source kind, target song id, replaces the queue, and returns lightweight confirmation metadata:

```rust
pub async fn submit_playlist_play_command(
    &self,
    name: &str,
    start_index: usize,
) -> MeloResult<crate::api::playlist::PlaylistPlayCommandResponse> {
    let preview = self.playlists.preview(name).await?;
    let target = preview
        .get(start_index)
        .ok_or_else(|| crate::core::error::MeloError::Message("queue index out of range".to_string()))?;
    let items = self.playlists.queue_items(name).await?;
    let source_kind = self
        .playlists
        .list_all()
        .await?
        .into_iter()
        .find(|playlist| playlist.name == name)
        .map(|playlist| playlist.kind)
        .unwrap_or_else(|| "static".to_string());

    let snapshot = self.player.replace_queue(items, start_index).await?;
    self.set_current_playlist_context(name, &source_kind);

    Ok(crate::api::playlist::PlaylistPlayCommandResponse {
        source_name: name.to_string(),
        source_kind,
        target_song_id: target.id,
        target_index: start_index,
        accepted_generation: snapshot.version,
    })
}
```

Add a client helper in `src/cli/client.rs`:

```rust
pub async fn playlist_play_command(
    &self,
    name: &str,
    start_index: usize,
) -> MeloResult<crate::api::playlist::PlaylistPlayCommandResponse> {
    self.send_json(
        self.client
            .post(format!("{}/api/playlists/play-command", self.base_url))
            .json(&crate::api::playlist::PlaylistPlayRequest {
                name: name.to_string(),
                start_index,
            }),
    )
    .await
}
```

- [ ] **Step 4: Re-run the tests to confirm they pass**

Run: `cargo test playlist_play_command_returns_lightweight_ack_without_tui_snapshot runtime_ws_confirms_playlist_play_command_after_submission --test api_server -- --nocapture`

Expected: PASS with lightweight command ack and runtime-based confirmation.

## Task 2: Add Local Playback Clock And Explicit Lyric Follow State Machine

**Files:**
- Create: `src/tui/playback_clock.rs`
- Create: `src/tui/playback_clock/tests.rs`
- Modify: `src/tui/mod.rs`
- Modify: `src/tui/lyrics.rs`
- Modify: `src/tui/app.rs`
- Modify: `src/tui/app/tests.rs`

- [ ] **Step 1: Write the failing local clock and lyric follow tests**

Add `src/tui/playback_clock/tests.rs`:

```rust
use std::time::{Duration, Instant};

use crate::core::model::playback_mode::PlaybackMode;
use crate::core::model::playback_runtime::PlaybackRuntimeSnapshot;

#[test]
fn playback_clock_advances_locally_while_playing() {
    let now = Instant::now();
    let mut clock = crate::tui::playback_clock::PlaybackClock::default();
    clock.apply_runtime(
        &PlaybackRuntimeSnapshot {
            generation: 3,
            playback_state: "playing".into(),
            current_source_ref: Some("Favorites".into()),
            current_song_id: Some(7),
            current_index: Some(0),
            position_seconds: Some(10.0),
            duration_seconds: Some(200.0),
            playback_mode: PlaybackMode::Ordered,
            volume_percent: 100,
            muted: false,
            last_error_code: None,
        },
        now,
    );

    let display = clock.display_position(now + Duration::from_millis(900));
    assert!(display.is_some_and(|value| value >= 10.8));
}

#[test]
fn playback_clock_freezes_when_runtime_is_paused() {
    let now = Instant::now();
    let mut clock = crate::tui::playback_clock::PlaybackClock::default();
    let runtime = PlaybackRuntimeSnapshot {
        generation: 5,
        playback_state: "paused".into(),
        current_source_ref: Some("Favorites".into()),
        current_song_id: Some(7),
        current_index: Some(0),
        position_seconds: Some(42.0),
        duration_seconds: Some(200.0),
        playback_mode: PlaybackMode::Ordered,
        volume_percent: 100,
        muted: false,
        last_error_code: None,
    };
    clock.apply_runtime(&runtime, now);

    assert_eq!(
        clock.display_position(now + Duration::from_secs(2)),
        Some(42.0)
    );
}
```

Update `src/tui/app/tests.rs`:

```rust
#[test]
fn lyric_follow_state_transitions_from_manual_browse_back_to_follow_current() {
    use std::time::{Duration, Instant};

    let now = Instant::now();
    let mut state = crate::tui::lyrics::LyricFollowState::new(Duration::from_secs(3));
    state.on_manual_scroll(now);
    assert!(matches!(state.mode(), crate::tui::lyrics::LyricFollowMode::ManualBrowse));

    state.tick(now + Duration::from_millis(100));
    assert!(matches!(state.mode(), crate::tui::lyrics::LyricFollowMode::ResumePending));

    state.tick(now + Duration::from_secs(3));
    assert!(matches!(state.mode(), crate::tui::lyrics::LyricFollowMode::FollowCurrent));
}

#[test]
fn app_uses_local_clock_for_current_lyric_index() {
    let mut app = crate::tui::app::App::new_for_test();
    app.current_track_song_id = Some(7);
    app.cache_track_content(crate::core::model::track_content::TrackContentSnapshot {
        song_id: 7,
        title: "Blue Bird".into(),
        duration_seconds: Some(212.0),
        artwork: None,
        lyrics: vec![
            crate::core::model::track_content::LyricLine { timestamp_seconds: 0.0, text: "a".into() },
            crate::core::model::track_content::LyricLine { timestamp_seconds: 5.0, text: "b".into() },
        ],
        refresh_token: "7-v1".into(),
    });

    let now = std::time::Instant::now();
    app.apply_runtime_snapshot_at(
        crate::core::model::playback_runtime::PlaybackRuntimeSnapshot {
            generation: 2,
            playback_state: "playing".into(),
            current_source_ref: Some("Favorites".into()),
            current_song_id: Some(7),
            current_index: Some(0),
            position_seconds: Some(4.4),
            duration_seconds: Some(212.0),
            playback_mode: crate::core::model::playback_mode::PlaybackMode::Ordered,
            volume_percent: 100,
            muted: false,
            last_error_code: None,
        },
        now,
    );

    assert_eq!(app.current_lyric_index_at(now + Duration::from_secs(1)), Some(1));
}
```

- [ ] **Step 2: Run the tests to confirm they fail**

Run: `cargo test playback_clock_advances_locally_while_playing playback_clock_freezes_when_runtime_is_paused lyric_follow_state_transitions_from_manual_browse_back_to_follow_current app_uses_local_clock_for_current_lyric_index --lib -- --nocapture`

Expected: FAIL because `PlaybackClock`, explicit lyric modes, and `apply_runtime_snapshot_at()` do not exist yet.

- [ ] **Step 3: Implement the local clock and lyric follow state machine**

Create `src/tui/playback_clock.rs`:

```rust
#[derive(Debug, Clone)]
pub struct PlaybackClock {
    anchor_position_seconds: Option<f64>,
    anchor_received_at: Option<std::time::Instant>,
    playback_state: String,
    duration_seconds: Option<f64>,
    generation: u64,
}

impl Default for PlaybackClock {
    fn default() -> Self {
        Self {
            anchor_position_seconds: None,
            anchor_received_at: None,
            playback_state: "idle".to_string(),
            duration_seconds: None,
            generation: 0,
        }
    }
}

impl PlaybackClock {
    pub fn apply_runtime(
        &mut self,
        runtime: &crate::core::model::playback_runtime::PlaybackRuntimeSnapshot,
        received_at: std::time::Instant,
    ) {
        const DRIFT_THRESHOLD_SECONDS: f64 = 0.75;

        let next_position = runtime.position_seconds;
        let replace_anchor = match (self.display_position(received_at), next_position) {
            (_, None) => true,
            (None, Some(_)) => true,
            (Some(current), Some(next)) => {
                runtime.generation != self.generation || (current - next).abs() >= DRIFT_THRESHOLD_SECONDS
            }
        };

        if replace_anchor {
            self.anchor_position_seconds = next_position;
            self.anchor_received_at = Some(received_at);
        }

        self.playback_state = runtime.playback_state.clone();
        self.duration_seconds = runtime.duration_seconds;
        self.generation = runtime.generation;
    }

    pub fn display_position(&self, now: std::time::Instant) -> Option<f64> {
        let base = self.anchor_position_seconds?;
        let duration = self.duration_seconds.unwrap_or(f64::MAX);
        if self.playback_state != "playing" {
            return Some(base.min(duration));
        }

        let anchor_received_at = self.anchor_received_at?;
        Some((base + now.duration_since(anchor_received_at).as_secs_f64()).min(duration))
    }
}

#[cfg(test)]
mod tests;
```

Update `src/tui/lyrics.rs`:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LyricFollowMode {
    FollowCurrent,
    ManualBrowse,
    ResumePending,
}

#[derive(Debug, Clone)]
pub struct LyricFollowState {
    resume_delay: std::time::Duration,
    mode: LyricFollowMode,
    last_manual_at: Option<std::time::Instant>,
}

impl LyricFollowState {
    pub fn mode(&self) -> LyricFollowMode {
        self.mode.clone()
    }

    pub fn on_manual_scroll(&mut self, now: std::time::Instant) {
        self.mode = LyricFollowMode::ManualBrowse;
        self.last_manual_at = Some(now);
    }

    pub fn tick(&mut self, now: std::time::Instant) {
        match self.mode {
            LyricFollowMode::FollowCurrent => {}
            LyricFollowMode::ManualBrowse => {
                self.mode = LyricFollowMode::ResumePending;
                self.last_manual_at = Some(now);
            }
            LyricFollowMode::ResumePending => {
                if self
                    .last_manual_at
                    .is_some_and(|last| now.duration_since(last) >= self.resume_delay)
                {
                    self.mode = LyricFollowMode::FollowCurrent;
                    self.last_manual_at = None;
                }
            }
        }
    }

    pub fn resume_now(&mut self) {
        self.mode = LyricFollowMode::FollowCurrent;
        self.last_manual_at = None;
    }
}
```

Expose the module from `src/tui/mod.rs`:

```rust
pub mod playback_clock;
```

Update `src/tui/app.rs` so runtime application writes the local clock and lyric highlighting consults local display time:

```rust
pub fn apply_runtime_snapshot_at(
    &mut self,
    runtime: crate::core::model::playback_runtime::PlaybackRuntimeSnapshot,
    now: std::time::Instant,
) {
    self.current_track_song_id = runtime.current_song_id;
    self.playback_clock.apply_runtime(&runtime, now);
    self.runtime = runtime;
}

pub fn current_lyric_index_at(&self, now: std::time::Instant) -> Option<usize> {
    let song_id = self.current_track_song_id?;
    let content = self.track_content_cache.get(&song_id)?;
    let position = self.playback_clock.display_position(now)?;
    content.current_lyric_index(position)
}
```

- [ ] **Step 4: Re-run the tests to confirm they pass**

Run: `cargo test playback_clock_advances_locally_while_playing playback_clock_freezes_when_runtime_is_paused lyric_follow_state_transitions_from_manual_browse_back_to_follow_current app_uses_local_clock_for_current_lyric_index --lib -- --nocapture`

Expected: PASS with smooth local time and explicit lyric follow states.

## Task 3: Rewire The TUI Loop To Runtime WS + Low-Frequency Refresh + Pending Playback Confirmation

**Files:**
- Modify: `src/tui/client.rs`
- Modify: `src/tui/run.rs`
- Modify: `src/tui/app.rs`
- Modify: `src/tui/run/tests.rs`
- Modify: `tests/tui_app.rs`

- [ ] **Step 1: Write the failing runtime reconciliation and preview-growth tests**

Update `src/tui/run/tests.rs`:

```rust
#[tokio::test]
async fn playlist_play_command_sets_pending_target_without_immediately_switching_current_song() {
    let mut app = crate::tui::app::App::new_for_test();
    app.apply_snapshot(crate::core::model::player::PlayerSnapshot {
        current_song: Some(crate::core::model::player::NowPlayingSong {
            song_id: 1,
            title: "Old".into(),
            duration_seconds: Some(100.0),
        }),
        ..crate::core::model::player::PlayerSnapshot::default()
    });

    app.mark_pending_playlist_play("Favorites".to_string(), 9, 3);
    assert_eq!(app.current_track_song_id, Some(1));
    assert!(app.pending_playlist_play().is_some());
}

#[tokio::test]
async fn runtime_confirmation_clears_pending_playlist_play_only_after_target_song_arrives() {
    let mut app = crate::tui::app::App::new_for_test();
    app.mark_pending_playlist_play("Favorites".to_string(), 9, 3);

    crate::tui::run::apply_runtime_delta_for_test(
        &mut app,
        crate::core::model::playback_runtime::PlaybackRuntimeSnapshot {
            generation: 2,
            playback_state: "playing".into(),
            current_source_ref: Some("Favorites".into()),
            current_song_id: Some(8),
            current_index: Some(2),
            position_seconds: Some(0.0),
            duration_seconds: Some(180.0),
            playback_mode: crate::core::model::playback_mode::PlaybackMode::Ordered,
            volume_percent: 100,
            muted: false,
            last_error_code: None,
        },
    );
    assert!(app.pending_playlist_play().is_some());

    crate::tui::run::apply_runtime_delta_for_test(
        &mut app,
        crate::core::model::playback_runtime::PlaybackRuntimeSnapshot {
            generation: 3,
            playback_state: "playing".into(),
            current_source_ref: Some("Favorites".into()),
            current_song_id: Some(9),
            current_index: Some(3),
            position_seconds: Some(0.0),
            duration_seconds: Some(180.0),
            playback_mode: crate::core::model::playback_mode::PlaybackMode::Ordered,
            volume_percent: 100,
            muted: false,
            last_error_code: None,
        },
    );
    assert!(app.pending_playlist_play().is_none());
}
```

Update `tests/tui_app.rs`:

```rust
#[test]
fn applying_home_snapshot_with_grown_count_marks_preview_for_reload() {
    let mut app = melo::tui::app::App::new_for_test();
    app.selected_playlist_name = Some("Favorites".into());
    app.preview_name = Some("Favorites".into());
    app.preview_songs = vec![
        melo::tui::app::PreviewSongRow { song_id: 1, title: "One".into() },
        melo::tui::app::PreviewSongRow { song_id: 2, title: "Two".into() },
    ];

    app.apply_playlist_browser_snapshot(melo::core::model::tui::PlaylistBrowserSnapshot {
        default_view: melo::core::model::tui::TuiViewKind::Playlist,
        default_selected_playlist: Some("Favorites".into()),
        current_playing_playlist: None,
        visible_playlists: vec![melo::core::model::tui::PlaylistListItem {
            name: "Favorites".into(),
            kind: "static".into(),
            count: 3,
            is_current_playing_source: false,
            is_ephemeral: false,
        }],
    });

    assert!(app.preview_reload_needed());
}
```

- [ ] **Step 2: Run the tests to confirm they fail**

Run: `cargo test playlist_play_command_sets_pending_target_without_immediately_switching_current_song runtime_confirmation_clears_pending_playlist_play_only_after_target_song_arrives applying_home_snapshot_with_grown_count_marks_preview_for_reload -- --nocapture`

Expected: FAIL because pending playlist confirmation and preview reload detection do not exist yet.

- [ ] **Step 3: Rewire the TUI runtime loop and low-frequency refresh**

Update `src/tui/client.rs` to add an explicit low-frequency refresh helper:

```rust
pub async fn refresh_home(
    &self,
    api_client: &crate::cli::client::ApiClient,
) -> MeloResult<crate::core::model::tui::TuiSnapshot> {
    api_client.tui_home().await
}
```

Update `src/tui/app.rs` with pending playlist state and preview reload detection:

```rust
pub struct PendingPlaylistPlay {
    pub source_name: String,
    pub target_song_id: i64,
    pub target_index: usize,
}

pub fn mark_pending_playlist_play(
    &mut self,
    source_name: String,
    target_song_id: i64,
    target_index: usize,
) {
    self.pending_playlist_play = Some(PendingPlaylistPlay {
        source_name,
        target_song_id,
        target_index,
    });
}

pub fn pending_playlist_play(&self) -> Option<&PendingPlaylistPlay> {
    self.pending_playlist_play.as_ref()
}

pub fn apply_playlist_browser_snapshot(
    &mut self,
    playlist_browser: crate::core::model::tui::PlaylistBrowserSnapshot,
) {
    self.preview_reload_needed = self
        .preview_name
        .as_ref()
        .and_then(|preview_name| {
            playlist_browser
                .visible_playlists
                .iter()
                .find(|playlist| &playlist.name == preview_name)
                .map(|playlist| playlist.count != self.preview_songs.len())
        })
        .unwrap_or(false);
    self.playlist_browser = playlist_browser;
}
```

Update `src/tui/run.rs`:

```rust
fn apply_runtime_delta(
    app: &mut crate::tui::app::App,
    runtime: crate::core::model::playback_runtime::PlaybackRuntimeSnapshot,
) {
    let now = std::time::Instant::now();
    app.apply_runtime_snapshot_at(runtime.clone(), now);
    if app
        .pending_playlist_play()
        .is_some_and(|pending| {
            runtime.current_song_id == Some(pending.target_song_id)
                && runtime.current_index == Some(pending.target_index)
                && runtime.playback_state == "playing"
        })
    {
        app.clear_pending_playlist_play();
    }
    app.tick_lyrics(now);
}
```

Replace the continuous `/api/ws/tui` consumer with a low-frequency home polling task:

```rust
let (home_tx, mut home_rx) = tokio::sync::mpsc::unbounded_channel();
tokio::spawn({
    let api_client = api_client.clone();
    let client = client.clone();
    async move {
        loop {
            match client.refresh_home(&api_client).await {
                Ok(snapshot) => {
                    if home_tx.send(snapshot).is_err() {
                        break;
                    }
                }
                Err(_) => {}
            }
            tokio::time::sleep(Duration::from_millis(750)).await;
        }
    }
});
```

When `Enter` on preview submits playback, use the new lightweight API instead of waiting on `TuiSnapshot`:

```rust
crate::tui::event::Intent::Action(crate::tui::event::ActionId::PlayPreviewSelection) => {
    if let Some(name) = app.selected_playlist_name().map(ToString::to_string) {
        let ack = api_client
            .playlist_play_command(&name, app.selected_preview_index())
            .await?;
        app.mark_pending_playlist_play(ack.source_name, ack.target_song_id, ack.target_index);
    }
}
```

Do the same for `PlaySelection`, targeting index `0`.

- [ ] **Step 4: Re-run the tests to confirm they pass**

Run: `cargo test playlist_play_command_sets_pending_target_without_immediately_switching_current_song runtime_confirmation_clears_pending_playlist_play_only_after_target_song_arrives applying_home_snapshot_with_grown_count_marks_preview_for_reload -- --nocapture`

Expected: PASS with low-frequency refresh driven preview growth and runtime-only confirmation.

## Task 4: Render Smooth Time, Lyrics Follow, Preview Scroll, And Stable Cover Source Text

**Files:**
- Modify: `src/domain/library/service.rs`
- Modify: `src/core/model/track_content.rs`
- Modify: `src/tui/ui/details.rs`
- Modify: `src/tui/ui/playlist.rs`
- Modify: `src/tui/ui/playbar.rs`
- Modify: `tests/tui_app.rs`
- Modify: `src/tui/run/tests.rs`
- Modify: `tests/cli_remote.rs`

- [ ] **Step 1: Write the failing rendering and cover-source tests**

Update `tests/tui_app.rs`:

```rust
#[test]
fn detail_lines_prefer_track_content_cover_source_summary() {
    let mut app = melo::tui::app::App::new_for_test();
    app.current_track_song_id = Some(7);
    app.current_track_cover_summary = Some("old fallback".into());
    app.cache_track_content(melo::core::model::track_content::TrackContentSnapshot {
        song_id: 7,
        title: "Blue Bird".into(),
        duration_seconds: Some(212.0),
        artwork: Some(melo::core::model::track_content::ArtworkSummary {
            source_kind: "embedded".into(),
            source_path: None,
            terminal_summary: "封面来自音频元数据".into(),
        }),
        lyrics: Vec::new(),
        refresh_token: "7-v1".into(),
    });

    let lines = melo::tui::ui::details::render_detail_lines_at(&app, std::time::Instant::now());
    assert!(lines.iter().any(|line| line.contains("音频元数据")));
    assert!(!lines.iter().any(|line| line.contains("old fallback")));
}

#[test]
fn playbar_uses_smoothed_runtime_position() {
    let mut app = melo::tui::app::App::new_for_test();
    let now = std::time::Instant::now();
    app.apply_snapshot(melo::core::model::player::PlayerSnapshot {
        current_song: Some(melo::core::model::player::NowPlayingSong {
            song_id: 1,
            title: "Blue Bird".into(),
            duration_seconds: Some(212.0),
        }),
        ..melo::core::model::player::PlayerSnapshot::default()
    });
    app.apply_runtime_snapshot_at(
        melo::core::model::playback_runtime::PlaybackRuntimeSnapshot {
            generation: 4,
            playback_state: "playing".into(),
            current_source_ref: Some("Favorites".into()),
            current_song_id: Some(1),
            current_index: Some(0),
            position_seconds: Some(72.0),
            duration_seconds: Some(212.0),
            playback_mode: melo::core::model::playback_mode::PlaybackMode::Ordered,
            volume_percent: 100,
            muted: false,
            last_error_code: None,
        },
        now,
    );

    let label = melo::tui::ui::playbar::playback_label_at(&app, now + std::time::Duration::from_secs(1));
    assert!(label.contains("01:13"));
}
```

Update `src/tui/run/tests.rs`:

```rust
#[test]
fn verbose_boundary_helper_still_exposes_startup_cwd_only() {
    let text = crate::cli::run::launch_cwd_text(std::path::Path::new("D:/Music/Aimer"));
    assert_eq!(text, "D:/Music/Aimer");
}
```

- [ ] **Step 2: Run the tests to confirm they fail**

Run: `cargo test detail_lines_prefer_track_content_cover_source_summary playbar_uses_smoothed_runtime_position verbose_boundary_helper_still_exposes_startup_cwd_only -- --nocapture`

Expected: FAIL because details/playbar still read old fallback fields and do not use local time.

- [ ] **Step 3: Implement stable cover text, lyric rendering, and smoothed playbar**

Update `src/domain/library/service.rs` so track content always emits a stable user-facing cover summary:

```rust
let artwork = match self.repository.artwork_for_song(record.song_id).await? {
    Some(artwork) if artwork.source_kind == "sidecar" => Some(ArtworkSummary {
        source_kind: artwork.source_kind,
        source_path: artwork.source_path.clone(),
        terminal_summary: format!(
            "封面来源：sidecar{}",
            artwork
                .source_path
                .as_ref()
                .map(|path| format!(" · {path}"))
                .unwrap_or_default()
        ),
    }),
    Some(artwork) if artwork.source_kind == "embedded" => Some(ArtworkSummary {
        source_kind: artwork.source_kind,
        source_path: None,
        terminal_summary: "封面来自音频元数据".to_string(),
    }),
    _ => Some(ArtworkSummary {
        source_kind: "none".to_string(),
        source_path: None,
        terminal_summary: "无封面".to_string(),
    }),
};
```

Update `src/tui/ui/details.rs` to render against `Instant::now()` via a helper and to prefer `track_content_cache` artwork summary:

```rust
pub fn render_detail_lines_at(
    app: &crate::tui::app::App,
    now: std::time::Instant,
) -> Vec<String> {
    let mut lines = Vec::new();

    if let Some(song_id) = app.current_track_song_id
        && let Some(content) = app.track_content_cache.get(&song_id)
    {
        lines.push(format!("当前曲目：{}", content.title));
        lines.push(String::new());
        lines.extend(app.render_visible_lyric_lines(now));
        lines.push(String::new());
        lines.push(
            content
                .artwork
                .as_ref()
                .map(|artwork| artwork.terminal_summary.clone())
                .unwrap_or_else(|| "封面信息不可用".to_string()),
        );
        return lines;
    }

    lines.push("当前曲目：无".to_string());
    lines.push(String::new());
    lines.push("歌词加载中...".to_string());
    lines.push(String::new());
    lines.push(
        app.current_track_cover_summary
            .clone()
            .unwrap_or_else(|| "封面信息不可用".to_string()),
    );
    lines
}

pub fn render_detail_lines(app: &crate::tui::app::App) -> Vec<String> {
    render_detail_lines_at(app, std::time::Instant::now())
}
```

Update `src/tui/ui/playbar.rs`:

```rust
pub fn playback_label_at(app: &crate::tui::app::App, now: std::time::Instant) -> String {
    let title = app
        .player
        .current_song
        .as_ref()
        .map(|song| song.title.as_str())
        .unwrap_or("Nothing Playing");
    let position = app.playback_clock().display_position(now);
    let duration = app.runtime.duration_seconds.or_else(|| {
        app.player
            .current_song
            .as_ref()
            .and_then(|song| song.duration_seconds)
    });
    let progress = match (position, duration) {
        (Some(position), Some(duration)) => format!("{} / {}", format_mmss(position), format_mmss(duration)),
        (Some(position), None) => format!("{} / --:--", format_mmss(position)),
        _ => "--:-- / --:--".to_string(),
    };

    let pending = if app.pending_playlist_play().is_some() { " | pending" } else { "" };
    format!("{} | {} | {}{}", app.runtime.playback_state, progress, title, pending)
}

pub fn playback_label(app: &crate::tui::app::App) -> String {
    playback_label_at(app, std::time::Instant::now())
}
```

Update the `run.rs` draw call to use `playback_label(&app)` and `render_detail_lines(&app)`.

- [ ] **Step 4: Re-run the tests to confirm they pass**

Run: `cargo test detail_lines_prefer_track_content_cover_source_summary playbar_uses_smoothed_runtime_position verbose_boundary_helper_still_exposes_startup_cwd_only -- --nocapture`

Expected: PASS with stable cover-source text and smoothed local time.

## Final Verification

- [ ] Run the focused Rust regression set:

```bash
cargo test playlist_play_command_returns_lightweight_ack_without_tui_snapshot runtime_ws_confirms_playlist_play_command_after_submission playback_clock_advances_locally_while_playing lyric_follow_state_transitions_from_manual_browse_back_to_follow_current runtime_confirmation_clears_pending_playlist_play_only_after_target_song_arrives detail_lines_prefer_track_content_cover_source_summary playbar_uses_smoothed_runtime_position -- --nocapture
```

Expected: All targeted tests PASS.

- [ ] Run the repo-required full verification:

```bash
pnpm qa
```

Expected: `format + lint + test` 全部 PASS。

- [ ] Manually smoke test in a real terminal:

```bash
cargo run -- daemon run
cargo run -- --verbose
```

Expected:
- `Enter` 选中预览切歌时界面继续刷新，当前播放只在 runtime 确认后切换
- 底部时间持续平滑推进，暂停/停止后冻结
- 歌词高亮跟随本地显示时间，鼠标滚动歌词后延迟恢复跟随
- 预览能滚动到尾部，并在补扫增长后看到新增歌曲
- 封面区稳定显示 `sidecar` / `embedded` / `none`
- `--verbose` 在进入 alternate screen 后不再继续覆盖 TUI

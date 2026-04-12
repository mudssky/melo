# Melo Playlist View and Current Play Context Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a playlist-centric TUI home that defaults to the current direct-open ephemeral playlist, supports playlist preview and playlist-based playback, and keeps the current playing playlist context visible without exposing `queue` as the UI concept.

**Architecture:** Keep `ratatui + crossterm`, expand the daemon-side TUI snapshot with a playlist browser section, and add a small in-memory store for the current playing playlist source. Materialize playlist playback through a new `replace_queue` player operation, expose playlist preview/play and TUI home APIs, then rebuild the TUI app state so local browsing selection and current playing source remain separate. Because ephemeral playlist names are full filesystem paths, implement playlist preview/play with query/body parameters instead of path-segment routes so path-like names remain lossless.

**Tech Stack:** Rust, Tokio, Axum, Ratatui, Crossterm, SeaORM, reqwest, utoipa, tempfile

---

## File Structure

### Daemon-side current playlist context and TUI snapshot

- Create: `src/daemon/playback_context.rs`
  - Responsibility: hold `PlayingPlaylistContext` and `PlayingPlaylistStore`, the daemon-owned in-memory source of truth for “当前播放来源”.
- Create: `src/daemon/playback_context/tests.rs`
  - Responsibility: unit coverage for setting, replacing, and clearing the current playlist context.
- Modify: `src/daemon/mod.rs`
  - Responsibility: export the playback context module.
- Modify: `src/core/model/tui.rs`
  - Responsibility: define `TuiViewKind`, `PlaylistListItem`, `PlaylistBrowserSnapshot`, and expand `TuiSnapshot`.
- Modify: `src/daemon/app.rs`
  - Responsibility: store `PlaylistService` and `PlayingPlaylistStore`, pass the store into `OpenService`, and build the expanded TUI snapshot.
- Modify: `src/api/ws.rs`
  - Responsibility: stream the expanded `TuiSnapshot` and handle snapshot construction failures gracefully.
- Modify: `tests/api_server.rs`
  - Responsibility: websocket contract coverage for the expanded TUI snapshot.

### Playlist materialization and direct-open source tracking

- Modify: `src/domain/player/service.rs`
  - Responsibility: add `replace_queue(items, start_index)` for one-shot playlist materialization without incremental `append`.
- Modify: `src/domain/player/service/tests.rs`
  - Responsibility: unit tests for `replace_queue` semantics, including preserved `repeat`/`shuffle`.
- Modify: `src/domain/playlist/service.rs`
  - Responsibility: add `queue_items(name)` for playlist-to-player conversion.
- Modify: `src/domain/open/service.rs`
  - Responsibility: use `replace_queue` for direct-open playback and set the daemon playback context store after success.
- Modify: `tests/direct_open_background_scan.rs`
  - Responsibility: prove direct-open still prewarms quickly and now records the current playing playlist source.

### HTTP APIs and client helpers

- Create: `src/api/playlist.rs`
  - Responsibility: implement `GET /api/playlists/preview?name=...` and `POST /api/playlists/play`.
- Create: `src/api/tui.rs`
  - Responsibility: implement `GET /api/tui/home`.
- Modify: `src/api/mod.rs`
  - Responsibility: export the new playlist and tui API modules.
- Modify: `src/api/docs.rs`
  - Responsibility: register the new HTTP APIs and schemas in OpenAPI.
- Modify: `src/api/queue.rs`
  - Responsibility: clear the current playlist context store on manual queue mutations.
- Modify: `src/daemon/server.rs`
  - Responsibility: register `/api/tui/home`, `/api/playlists/preview`, and `/api/playlists/play`.
- Modify: `src/cli/client.rs`
  - Responsibility: add typed HTTP helpers for TUI home, playlist preview, and playlist play.
- Modify: `tests/api_server.rs`
  - Responsibility: integration coverage for the new endpoints and OpenAPI paths.

### TUI state, layout, rendering, and runtime wiring

- Modify: `src/tui/app.rs`
  - Responsibility: store playlist browser snapshot, local browsing selection, preview cache/error/loading state, and focus handling.
- Create: `src/tui/app/tests.rs`
  - Responsibility: unit tests for playlist selection reconciliation, focus switching, `Enter` behavior, and mode toggle actions.
- Modify: `src/tui/event.rs`
  - Responsibility: add high-level actions for preview loading, playlist playback, repeat cycling, and shuffle toggling.
- Modify: `src/tui/run.rs`
  - Responsibility: bootstrap from `/api/tui/home`, load previews on selection changes, invoke playlist play APIs, and keep websocket updates flowing.
- Modify: `src/tui/run/tests.rs`
  - Responsibility: unit coverage for repeat-mode cycling and layout/init helpers.
- Modify: `src/tui/ui/layout.rs`
  - Responsibility: split the right pane into “当前播放来源/播放模式” and “歌单预览”.
- Create: `src/tui/ui/playlist.rs`
  - Responsibility: render left playlist list, right status panel, and preview panel lines.
- Modify: `src/tui/ui/mod.rs`
  - Responsibility: export the new playlist UI helper module.
- Modify: `src/tui/ui/popup.rs`
  - Responsibility: update help text to reflect `Tab`, `Enter`, `r`, and `s`.
- Modify: `tests/tui_app.rs`
  - Responsibility: integration coverage for footer status plus playlist-browser-specific render state.
- Create: `tests/tui_home.rs`
  - Responsibility: end-to-end daemon snapshot regression for “direct-open defaults to the current ephemeral playlist”.

## Task 1: Add Daemon Playback Context and Expand `TuiSnapshot`

**Files:**
- Create: `src/daemon/playback_context.rs`
- Create: `src/daemon/playback_context/tests.rs`
- Modify: `src/daemon/mod.rs`
- Modify: `src/core/model/tui.rs`
- Modify: `src/daemon/app.rs`
- Modify: `src/api/ws.rs`
- Test: `tests/api_server.rs`

- [ ] **Step 1: Write the failing store and websocket snapshot tests**

Add this test to `src/daemon/playback_context/tests.rs`:

```rust
use crate::daemon::playback_context::{PlayingPlaylistContext, PlayingPlaylistStore};

#[test]
fn playback_context_store_sets_and_clears_current_playlist() {
    let store = PlayingPlaylistStore::default();
    assert!(store.current().is_none());

    store.set(PlayingPlaylistContext {
        name: "C:/Music/Aimer".to_string(),
        kind: "ephemeral".to_string(),
    });
    assert_eq!(store.current().unwrap().name, "C:/Music/Aimer");

    store.clear();
    assert!(store.current().is_none());
}
```

Add this test to `tests/api_server.rs` near the existing `/api/ws/tui` coverage:

```rust
#[tokio::test(flavor = "multi_thread")]
async fn api_tui_websocket_initial_snapshot_includes_playlist_browser_defaults() {
    let state = melo::daemon::app::AppState::for_test().await;
    state.set_current_playlist_context("C:/Music/Aimer", "ephemeral");
    let app = melo::daemon::server::router(state);
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let (mut stream, _response) = connect_async(format!("ws://{addr}/api/ws/tui"))
        .await
        .unwrap();
    let message = stream.next().await.unwrap().unwrap();
    let snapshot: melo::core::model::tui::TuiSnapshot =
        serde_json::from_str(&message.into_text().unwrap()).unwrap();

    assert_eq!(
        snapshot.playlist_browser.default_view,
        melo::core::model::tui::TuiViewKind::Playlist
    );
    assert_eq!(
        snapshot.playlist_browser.default_selected_playlist.as_deref(),
        Some("C:/Music/Aimer")
    );
    assert_eq!(
        snapshot
            .playlist_browser
            .current_playing_playlist
            .as_ref()
            .unwrap()
            .kind,
        "ephemeral"
    );
}
```

- [ ] **Step 2: Run the new tests to confirm the gap**

Run: `cargo test playback_context_store_sets_and_clears_current_playlist api_tui_websocket_initial_snapshot_includes_playlist_browser_defaults -- --nocapture`

Expected: FAIL because `PlayingPlaylistStore`, `TuiViewKind`, and `playlist_browser` do not exist yet.

- [ ] **Step 3: Implement the playback context store and expanded TUI model**

Create `src/daemon/playback_context.rs` with:

```rust
use std::sync::RwLock;

/// 当前播放来源的 daemon 内存态。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayingPlaylistContext {
    /// 当前播放来源名称。
    pub name: String,
    /// 当前播放来源类型。
    pub kind: String,
}

/// 当前播放来源存储。
#[derive(Debug, Default)]
pub struct PlayingPlaylistStore {
    current: RwLock<Option<PlayingPlaylistContext>>,
}

impl PlayingPlaylistStore {
    /// 读取当前播放来源。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - `Option<PlayingPlaylistContext>`：当前播放来源
    pub fn current(&self) -> Option<PlayingPlaylistContext> {
        self.current.read().ok().and_then(|guard| guard.clone())
    }

    /// 设置当前播放来源。
    ///
    /// # 参数
    /// - `context`：新的当前播放来源
    ///
    /// # 返回值
    /// - 无
    pub fn set(&self, context: PlayingPlaylistContext) {
        if let Ok(mut guard) = self.current.write() {
            *guard = Some(context);
        }
    }

    /// 清空当前播放来源。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - 无
    pub fn clear(&self) {
        if let Ok(mut guard) = self.current.write() {
            *guard = None;
        }
    }
}

#[cfg(test)]
mod tests;
```

Update `src/core/model/tui.rs` to:

```rust
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::core::model::player::PlayerSnapshot;
use crate::core::model::runtime_task::RuntimeTaskSnapshot;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum TuiViewKind {
    Playlist,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, ToSchema)]
pub struct PlaylistListItem {
    pub name: String,
    pub kind: String,
    pub count: usize,
    pub is_current_playing_source: bool,
    pub is_ephemeral: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, ToSchema)]
pub struct PlaylistBrowserSnapshot {
    pub default_view: TuiViewKind,
    pub default_selected_playlist: Option<String>,
    pub current_playing_playlist: Option<PlaylistListItem>,
    pub visible_playlists: Vec<PlaylistListItem>,
}

impl Default for PlaylistBrowserSnapshot {
    fn default() -> Self {
        Self {
            default_view: TuiViewKind::Playlist,
            default_selected_playlist: None,
            current_playing_playlist: None,
            visible_playlists: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, ToSchema)]
pub struct TuiSnapshot {
    pub player: PlayerSnapshot,
    pub active_task: Option<RuntimeTaskSnapshot>,
    pub playlist_browser: PlaylistBrowserSnapshot,
}
```

- [ ] **Step 4: Wire `AppState` and websocket streaming to the new snapshot**

Update `src/daemon/app.rs` so `AppState` owns `PlaylistService` and `PlayingPlaylistStore`, then build `playlist_browser` inside `tui_snapshot()`:

```rust
#[derive(Clone)]
pub struct AppState {
    pub player: Arc<PlayerService>,
    pub settings: Settings,
    pub playlists: PlaylistService,
    pub open: Arc<crate::domain::open::service::OpenService>,
    runtime_tasks: Arc<crate::daemon::tasks::RuntimeTaskStore>,
    playback_context: Arc<crate::daemon::playback_context::PlayingPlaylistStore>,
    runtime: Arc<DaemonRuntimeMeta>,
    shutdown_notify: Arc<Notify>,
    shutdown_requested: Arc<AtomicBool>,
}

pub fn set_current_playlist_context(&self, name: &str, kind: &str) {
    self.playback_context
        .set(crate::daemon::playback_context::PlayingPlaylistContext {
            name: name.to_string(),
            kind: kind.to_string(),
        });
}

pub fn clear_current_playlist_context(&self) {
    self.playback_context.clear();
}

pub fn current_playlist_context(
    &self,
) -> Option<crate::daemon::playback_context::PlayingPlaylistContext> {
    self.playback_context.current()
}

pub async fn tui_snapshot(&self) -> MeloResult<crate::core::model::tui::TuiSnapshot> {
    let player = self.player.snapshot().await;
    let current = self.playback_context.current();
    let mut visible_playlists = self
        .playlists
        .list_visible()
        .await?
        .into_iter()
        .map(|playlist| crate::core::model::tui::PlaylistListItem {
            is_current_playing_source: current
                .as_ref()
                .is_some_and(|context| context.name == playlist.name),
            is_ephemeral: playlist.kind == "ephemeral",
            name: playlist.name,
            kind: playlist.kind,
            count: playlist.count,
        })
        .collect::<Vec<_>>();

    let current_playing_playlist = current.map(|context| crate::core::model::tui::PlaylistListItem {
        name: context.name.clone(),
        kind: context.kind.clone(),
        count: player.queue_len,
        is_current_playing_source: true,
        is_ephemeral: context.kind == "ephemeral",
    });

    visible_playlists.sort_by(|left, right| left.name.cmp(&right.name));

    Ok(crate::core::model::tui::TuiSnapshot {
        player,
        active_task: self.runtime_tasks.current(),
        playlist_browser: crate::core::model::tui::PlaylistBrowserSnapshot {
            default_view: crate::core::model::tui::TuiViewKind::Playlist,
            default_selected_playlist: current_playing_playlist
                .as_ref()
                .map(|playlist| playlist.name.clone()),
            current_playing_playlist,
            visible_playlists,
        },
    })
}
```

Update `src/api/ws.rs` to handle the new `MeloResult<TuiSnapshot>`:

```rust
async fn stream_tui_snapshots(mut socket: WebSocket, state: AppState) {
    let mut player_rx = state.player.subscribe();
    let mut task_rx = state.runtime_tasks().subscribe();

    let Ok(initial_snapshot) = state.tui_snapshot().await else {
        return;
    };
    if send_payload(&mut socket, &initial_snapshot).await.is_err() {
        return;
    }

    loop {
        tokio::select! {
            changed = player_rx.changed() => {
                if changed.is_err() {
                    break;
                }
            }
            changed = task_rx.changed() => {
                if changed.is_err() {
                    break;
                }
            }
        }

        let Ok(snapshot) = state.tui_snapshot().await else {
            break;
        };
        if send_payload(&mut socket, &snapshot).await.is_err() {
            break;
        }
    }
}
```

- [ ] **Step 5: Run the focused tests again**

Run: `cargo test playback_context_store_sets_and_clears_current_playlist api_tui_websocket_initial_snapshot_includes_playlist_browser_defaults -- --nocapture`

Expected: PASS.

- [ ] **Step 6: Run full verification and commit the snapshot contract slice**

Run: `pnpm qa`

Expected: PASS.

Run:

```bash
git add src/daemon/playback_context.rs src/daemon/playback_context/tests.rs src/daemon/mod.rs src/core/model/tui.rs src/daemon/app.rs src/api/ws.rs tests/api_server.rs
git commit -m "feat: add tui playlist browser snapshot"
```

## Task 2: Add One-Shot Playlist Materialization and Direct-Open Context Tracking

**Files:**
- Modify: `src/domain/player/service.rs`
- Modify: `src/domain/player/service/tests.rs`
- Modify: `src/domain/playlist/service.rs`
- Modify: `src/domain/open/service.rs`
- Test: `tests/direct_open_background_scan.rs`

- [ ] **Step 1: Write the failing player-service and direct-open tests**

Add this test to `src/domain/player/service/tests.rs`:

```rust
#[tokio::test]
async fn replace_queue_sets_selected_index_and_preserves_repeat_and_shuffle() {
    let backend = Arc::new(FakeBackend::default());
    let service = PlayerService::new(backend);

    service.set_repeat_mode(RepeatMode::All).await.unwrap();
    service.set_shuffle_enabled(true).await.unwrap();
    service
        .replace_queue(
            vec![item(1, "One"), item(2, "Two"), item(3, "Three")],
            1,
        )
        .await
        .unwrap();

    let snapshot = service.snapshot().await;
    assert_eq!(snapshot.queue_len, 3);
    assert_eq!(snapshot.queue_index, Some(1));
    assert_eq!(snapshot.current_song.unwrap().title, "Two");
    assert_eq!(snapshot.repeat_mode, "all");
    assert!(snapshot.shuffle_enabled);
}
```

Add this test to `tests/direct_open_background_scan.rs`:

```rust
#[tokio::test(flavor = "multi_thread")]
async fn directory_open_sets_current_playlist_context_after_prewarm() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::write(temp.path().join("01-first.flac"), b"audio").unwrap();

    let mut settings = Settings::for_test(temp.path().join("melo.db"));
    settings.open.max_depth = 1;
    settings.open.prewarm_limit = 1;
    DatabaseBootstrap::new(&settings).init().await.unwrap();

    let player = Arc::new(PlayerService::new(Arc::new(NoopBackend)));
    player.start_runtime_event_loop();
    player.start_progress_loop();
    let library = LibraryService::for_test(settings.clone());
    let playlists = PlaylistService::new(settings.clone());
    let tasks = Arc::new(RuntimeTaskStore::new());
    let playback_context =
        Arc::new(melo::daemon::playback_context::PlayingPlaylistStore::default());
    let open = OpenService::new(
        settings.clone(),
        library,
        playlists,
        Arc::clone(&player),
        Arc::clone(&tasks),
        Arc::clone(&playback_context),
    );

    open.open(OpenRequest {
        target: temp.path().to_string_lossy().to_string(),
        mode: "path_dir".to_string(),
    })
    .await
    .unwrap();

    let current = playback_context.current().unwrap();
    assert_eq!(current.kind, "ephemeral");
    assert_eq!(current.name, temp.path().to_string_lossy());
}
```

- [ ] **Step 2: Run the tests to confirm they fail**

Run: `cargo test replace_queue_sets_selected_index_and_preserves_repeat_and_shuffle directory_open_sets_current_playlist_context_after_prewarm -- --nocapture`

Expected: FAIL because `replace_queue` does not exist and `OpenService::new` does not accept the playback context store.

- [ ] **Step 3: Add `replace_queue` and playlist queue materialization**

Update `src/domain/player/service.rs`:

```rust
pub async fn replace_queue(
    &self,
    items: Vec<QueueItem>,
    start_index: usize,
) -> MeloResult<PlayerSnapshot> {
    if items.is_empty() {
        return Err(MeloError::Message("queue is empty".to_string()));
    }
    if start_index >= items.len() {
        return Err(MeloError::Message("queue index out of range".to_string()));
    }

    let mut session = self.session.lock().await;
    session.queue = PlayerQueue::from_items(items, Some(start_index));
    session.playback_state = PlaybackState::Stopped;
    session.last_error = None;
    session.position_seconds = Some(0.0);
    drop(session);
    self.play().await
}
```

Update `src/domain/playlist/service.rs`:

```rust
use crate::core::model::player::QueueItem;

pub async fn queue_items(&self, name: &str) -> MeloResult<Vec<QueueItem>> {
    let settings = self.current_settings()?;
    let song_ids = if let Some(definition) = settings.playlists.smart.get(name) {
        let query = SmartQuery::parse(&definition.query)?;
        self.library_repository
            .list_by_query(&query)
            .await?
            .into_iter()
            .map(|song| song.id)
            .collect::<Vec<_>>()
    } else {
        self.repository
            .preview_static(name)
            .await?
            .into_iter()
            .map(|song| song.id)
            .collect::<Vec<_>>()
    };

    self.library_repository.queue_items_by_song_ids(&song_ids).await
}
```

- [ ] **Step 4: Switch direct-open to `replace_queue` and update playback context only after success**

Update the `OpenService` constructor and the two direct-open branches in `src/domain/open/service.rs`:

```rust
pub struct OpenService {
    settings: Settings,
    library: LibraryService,
    playlists: PlaylistService,
    player: Arc<PlayerService>,
    tasks: Arc<RuntimeTaskStore>,
    playback_context: Arc<crate::daemon::playback_context::PlayingPlaylistStore>,
}

pub fn new(
    settings: Settings,
    library: LibraryService,
    playlists: PlaylistService,
    player: Arc<PlayerService>,
    tasks: Arc<RuntimeTaskStore>,
    playback_context: Arc<crate::daemon::playback_context::PlayingPlaylistStore>,
) -> Self {
    Self {
        settings,
        library,
        playlists,
        player,
        tasks,
        playback_context,
    }
}

let queue_items = self.library.queue_items_for_song_ids(&song_ids).await?;
let snapshot = self.player.replace_queue(queue_items, 0).await?;
self.playback_context.set(
    crate::daemon::playback_context::PlayingPlaylistContext {
        name: playlist.name.clone(),
        kind: playlist.kind.clone(),
    },
);
```

Apply the same `replace_queue(..., 0)` path in both `open_audio_file_target` and `open_directory_target`.

- [ ] **Step 5: Run the focused tests again**

Run: `cargo test replace_queue_sets_selected_index_and_preserves_repeat_and_shuffle directory_open_sets_current_playlist_context_after_prewarm -- --nocapture`

Expected: PASS.

- [ ] **Step 6: Run full verification and commit the playlist materialization slice**

Run: `pnpm qa`

Expected: PASS.

Run:

```bash
git add src/domain/player/service.rs src/domain/player/service/tests.rs src/domain/playlist/service.rs src/domain/open/service.rs tests/direct_open_background_scan.rs
git commit -m "feat: materialize playlist playback into current play list"
```

## Task 3: Add TUI Home and Playlist Preview/Play HTTP APIs

**Files:**
- Create: `src/api/playlist.rs`
- Create: `src/api/tui.rs`
- Modify: `src/api/mod.rs`
- Modify: `src/api/docs.rs`
- Modify: `src/api/queue.rs`
- Modify: `src/daemon/server.rs`
- Modify: `src/cli/client.rs`
- Modify: `tests/api_server.rs`

- [ ] **Step 1: Write the failing API tests for home, preview, play, and context clearing**

Add these tests to `tests/api_server.rs`:

```rust
#[tokio::test]
async fn tui_home_endpoint_returns_playlist_browser_snapshot() {
    let state = melo::daemon::app::AppState::for_test().await;
    state.set_current_playlist_context("C:/Music/Aimer", "ephemeral");
    let app = melo::daemon::server::router(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/tui/home")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(payload["data"]["playlist_browser"]["default_view"], "playlist");
}

#[tokio::test]
async fn playlist_preview_endpoint_accepts_path_named_ephemeral_playlist() {
    let harness = melo::test_support::TestHarness::new().await;
    let playlist_service = harness.playlist_service();
    let ephemeral_name = "C:/Temp/Aimer".to_string();
    harness.seed_song("Blue Bird", "Ikimono-gakari", "Blue Bird", 2008).await;
    playlist_service
        .upsert_ephemeral(&ephemeral_name, "path_dir", &ephemeral_name, true, None, &[1])
        .await
        .unwrap();
    let app = melo::daemon::app::test_router_with_settings(harness.settings.clone()).await;

    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/playlists/preview?name={ephemeral_name}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn playlist_play_endpoint_starts_from_selected_index_and_updates_current_source() {
    let harness = melo::test_support::TestHarness::new().await;
    let playlist_service = harness.playlist_service();
    harness.seed_song("One", "Aimer", "Singles", 2015).await;
    harness.seed_song("Two", "Aimer", "Singles", 2015).await;
    playlist_service.create_static("Favorites", None).await.unwrap();
    playlist_service.add_songs("Favorites", &[1, 2]).await.unwrap();
    let app = melo::daemon::app::test_router_with_settings(harness.settings.clone()).await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/playlists/play")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"name":"Favorites","start_index":1}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(payload["data"]["player"]["queue_index"], 1);
    assert_eq!(
        payload["data"]["playlist_browser"]["current_playing_playlist"]["name"],
        "Favorites"
    );
}

#[tokio::test]
async fn queue_clear_endpoint_clears_current_playlist_context() {
    let state = melo::daemon::app::AppState::for_test().await;
    state.set_current_playlist_context("Favorites", "static");
    let app = melo::daemon::server::router(state.clone());

    let _ = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/queue/clear")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert!(state.current_playlist_context().is_none());
}
```

Update the existing OpenAPI test in `tests/api_server.rs` to assert:

```rust
assert!(payload["paths"]["/api/tui/home"].is_object());
assert!(payload["paths"]["/api/playlists/preview"].is_object());
assert!(payload["paths"]["/api/playlists/play"].is_object());
assert!(payload["paths"]["/api/ws/tui"].is_object());
```

- [ ] **Step 2: Run the API tests to confirm they fail**

Run: `cargo test tui_home_endpoint_returns_playlist_browser_snapshot playlist_preview_endpoint_accepts_path_named_ephemeral_playlist playlist_play_endpoint_starts_from_selected_index_and_updates_current_source queue_clear_endpoint_clears_current_playlist_context openapi_json_endpoint_is_available -- --nocapture`

Expected: FAIL because the new routes and client/server types do not exist yet.

- [ ] **Step 3: Implement the new API modules and route registration**

Create `src/api/tui.rs`:

```rust
use axum::{Json, extract::State};

use crate::api::error::ApiError;
use crate::api::response::ApiResponse;
use crate::daemon::app::AppState;

#[utoipa::path(
    get,
    path = "/api/tui/home",
    responses(
        (status = 200, description = "TUI 首页聚合快照", body = crate::api::response::ApiResponse<crate::core::model::tui::TuiSnapshot>),
        (status = 500, description = "快照聚合失败", body = crate::api::response::ApiResponse<serde_json::Value>)
    )
)]
pub async fn home(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<crate::core::model::tui::TuiSnapshot>>, ApiError> {
    state
        .tui_snapshot()
        .await
        .map(ApiResponse::ok)
        .map(Json)
        .map_err(ApiError::from)
}
```

Create `src/api/playlist.rs`:

```rust
use axum::{
    Json,
    extract::{Query, State},
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::api::error::ApiError;
use crate::api::response::ApiResponse;
use crate::daemon::app::AppState;

#[derive(Debug, Deserialize, ToSchema)]
pub struct PlaylistPreviewQuery {
    pub name: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct PlaylistPlayRequest {
    pub name: String,
    pub start_index: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PlaylistPreviewSong {
    pub id: i64,
    pub title: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PlaylistPreviewResponse {
    pub name: String,
    pub songs: Vec<PlaylistPreviewSong>,
}

#[utoipa::path(
    get,
    path = "/api/playlists/preview",
    params(("name" = String, Query, description = "歌单名")),
    responses((status = 200, body = crate::api::response::ApiResponse<PlaylistPreviewResponse>))
)]
pub async fn preview(
    State(state): State<AppState>,
    Query(query): Query<PlaylistPreviewQuery>,
) -> Result<Json<ApiResponse<PlaylistPreviewResponse>>, ApiError> {
    let songs = state
        .playlists
        .preview(&query.name)
        .await?
        .into_iter()
        .map(|song| PlaylistPreviewSong {
            id: song.id,
            title: song.title,
        })
        .collect::<Vec<_>>();

    Ok(Json(ApiResponse::ok(PlaylistPreviewResponse {
        name: query.name,
        songs,
    })))
}

#[utoipa::path(
    post,
    path = "/api/playlists/play",
    request_body = PlaylistPlayRequest,
    responses((status = 200, body = crate::api::response::ApiResponse<crate::core::model::tui::TuiSnapshot>))
)]
pub async fn play(
    State(state): State<AppState>,
    Json(request): Json<PlaylistPlayRequest>,
) -> Result<Json<ApiResponse<crate::core::model::tui::TuiSnapshot>>, ApiError> {
    let items = state.playlists.queue_items(&request.name).await?;
    let kind = state
        .playlists
        .list_all()
        .await?
        .into_iter()
        .find(|playlist| playlist.name == request.name)
        .map(|playlist| playlist.kind)
        .unwrap_or_else(|| "static".to_string());

    state.player.replace_queue(items, request.start_index).await?;
    state.set_current_playlist_context(&request.name, &kind);

    state
        .tui_snapshot()
        .await
        .map(ApiResponse::ok)
        .map(Json)
        .map_err(ApiError::from)
}
```

Register the routes in `src/daemon/server.rs`:

```rust
.route("/api/tui/home", axum::routing::get(crate::api::tui::home))
.route("/api/playlists/preview", axum::routing::get(crate::api::playlist::preview))
.route("/api/playlists/play", axum::routing::post(crate::api::playlist::play))
```

- [ ] **Step 4: Clear stale context on manual queue mutations and add typed client helpers**

Update `src/api/queue.rs` after successful queue mutations:

```rust
pub async fn clear(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<PlayerSnapshot>>, ApiError> {
    let snapshot = state.player.clear().await.map_err(ApiError::from)?;
    state.clear_current_playlist_context();
    Ok(Json(ApiResponse::ok(snapshot)))
}
```

Apply the same `state.clear_current_playlist_context();` line in `add`, `insert`, `remove`, and `move_item` after the player mutation succeeds.

Update `src/cli/client.rs` with:

```rust
pub async fn tui_home(&self) -> MeloResult<crate::core::model::tui::TuiSnapshot> {
    let url = format!("{}/api/tui/home", self.base_url);
    self.send_and_decode(self.client.get(url)).await
}

pub async fn playlist_preview(
    &self,
    name: &str,
) -> MeloResult<crate::api::playlist::PlaylistPreviewResponse> {
    let mut url = reqwest::Url::parse(&format!("{}/api/playlists/preview", self.base_url))
        .map_err(|err| MeloError::Message(err.to_string()))?;
    url.query_pairs_mut().append_pair("name", name);
    self.send_and_decode(self.client.get(url)).await
}

pub async fn playlist_play(
    &self,
    name: &str,
    start_index: usize,
) -> MeloResult<crate::core::model::tui::TuiSnapshot> {
    let url = format!("{}/api/playlists/play", self.base_url);
    self.send_and_decode(
        self.client
            .post(url)
            .json(&serde_json::json!({ "name": name, "start_index": start_index })),
    )
    .await
}
```

Also update `src/api/docs.rs` and `src/api/mod.rs` to export the new modules and schemas:

```rust
pub mod playlist;
pub mod tui;
```

And in the OpenAPI derive:

```rust
crate::api::playlist::preview,
crate::api::playlist::play,
crate::api::tui::home,
crate::api::ws::tui_updates,
```

- [ ] **Step 5: Run the focused API tests again**

Run: `cargo test tui_home_endpoint_returns_playlist_browser_snapshot playlist_preview_endpoint_accepts_path_named_ephemeral_playlist playlist_play_endpoint_starts_from_selected_index_and_updates_current_source queue_clear_endpoint_clears_current_playlist_context openapi_json_endpoint_is_available -- --nocapture`

Expected: PASS.

- [ ] **Step 6: Run full verification and commit the API slice**

Run: `pnpm qa`

Expected: PASS.

Run:

```bash
git add src/api/playlist.rs src/api/tui.rs src/api/mod.rs src/api/docs.rs src/api/queue.rs src/daemon/server.rs src/cli/client.rs tests/api_server.rs
git commit -m "feat: add playlist browser http api"
```

## Task 4: Rebuild the TUI App State Around Playlist Browsing

**Files:**
- Modify: `src/tui/app.rs`
- Create: `src/tui/app/tests.rs`
- Modify: `src/tui/event.rs`
- Modify: `src/tui/ui/layout.rs`
- Create: `src/tui/ui/playlist.rs`
- Modify: `src/tui/ui/mod.rs`
- Test: `tests/tui_app.rs`

- [ ] **Step 1: Write the failing app-state and rendering tests**

Create `src/tui/app/tests.rs` with:

```rust
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

fn browser_snapshot() -> crate::core::model::tui::PlaylistBrowserSnapshot {
    crate::core::model::tui::PlaylistBrowserSnapshot {
        default_view: crate::core::model::tui::TuiViewKind::Playlist,
        default_selected_playlist: Some("Favorites".to_string()),
        current_playing_playlist: Some(crate::core::model::tui::PlaylistListItem {
            name: "Favorites".to_string(),
            kind: "static".to_string(),
            count: 2,
            is_current_playing_source: true,
            is_ephemeral: false,
        }),
        visible_playlists: vec![
            crate::core::model::tui::PlaylistListItem {
                name: "Favorites".to_string(),
                kind: "static".to_string(),
                count: 2,
                is_current_playing_source: true,
                is_ephemeral: false,
            },
            crate::core::model::tui::PlaylistListItem {
                name: "Aimer".to_string(),
                kind: "smart".to_string(),
                count: 4,
                is_current_playing_source: false,
                is_ephemeral: false,
            },
        ],
    }
}

#[test]
fn app_uses_default_selected_playlist_only_for_initial_selection() {
    let mut app = crate::tui::app::App::new_for_test();
    app.apply_tui_snapshot(crate::core::model::tui::TuiSnapshot {
        player: crate::core::model::player::PlayerSnapshot::default(),
        active_task: None,
        playlist_browser: browser_snapshot(),
    });
    assert_eq!(app.selected_playlist_name(), Some("Favorites"));

    app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    assert_eq!(app.selected_playlist_name(), Some("Aimer"));

    app.apply_tui_snapshot(crate::core::model::tui::TuiSnapshot {
        player: crate::core::model::player::PlayerSnapshot::default(),
        active_task: None,
        playlist_browser: browser_snapshot(),
    });
    assert_eq!(app.selected_playlist_name(), Some("Aimer"));
}

#[test]
fn tab_switches_focus_between_playlist_list_and_preview() {
    let mut app = crate::tui::app::App::new_for_test();
    assert_eq!(app.focus, crate::tui::app::FocusArea::PlaylistList);

    let action = app.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));

    assert_eq!(action, None);
    assert_eq!(app.focus, crate::tui::app::FocusArea::PlaylistPreview);
}

#[test]
fn enter_on_playlist_list_requests_play_from_start() {
    let mut app = crate::tui::app::App::new_for_test();
    app.apply_tui_snapshot(crate::core::model::tui::TuiSnapshot {
        player: crate::core::model::player::PlayerSnapshot::default(),
        active_task: None,
        playlist_browser: browser_snapshot(),
    });

    let action = app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    assert_eq!(action, Some(crate::tui::event::Action::PlaySelectedPlaylistFromStart));
}
```

Update `tests/tui_app.rs` with a playlist-browser render assertion:

```rust
#[test]
fn footer_status_still_includes_repeat_and_shuffle_after_playlist_snapshot_applies() {
    let mut app = melo::tui::app::App::new_for_test();
    app.apply_tui_snapshot(melo::core::model::tui::TuiSnapshot {
        player: melo::core::model::player::PlayerSnapshot {
            repeat_mode: "all".into(),
            shuffle_enabled: true,
            ..melo::core::model::player::PlayerSnapshot::default()
        },
        active_task: None,
        playlist_browser: melo::core::model::tui::PlaylistBrowserSnapshot::default(),
    });

    let footer = app.footer_status();
    assert!(footer.contains("repeat=all"));
    assert!(footer.contains("shuffle=true"));
}
```

- [ ] **Step 2: Run the TUI tests to confirm they fail**

Run: `cargo test app_uses_default_selected_playlist_only_for_initial_selection tab_switches_focus_between_playlist_list_and_preview enter_on_playlist_list_requests_play_from_start footer_status_still_includes_repeat_and_shuffle_after_playlist_snapshot_applies -- --nocapture`

Expected: FAIL because playlist-browser state and the new actions/focus areas do not exist.

- [ ] **Step 3: Implement playlist-browser app state, actions, and render helpers**

Update `src/tui/event.rs`:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    TogglePlayback,
    Next,
    Prev,
    Quit,
    OpenSearch,
    OpenHelp,
    LoadSelectedPlaylistPreview,
    PlaySelectedPlaylistFromStart,
    PlaySelectedPreviewSong,
    CycleRepeatMode,
    ToggleShuffle,
}
```

Update `src/tui/app.rs`:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActiveView {
    Playlist,
    Songs,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FocusArea {
    PlaylistList,
    PlaylistPreview,
}

pub struct App {
    pub player: PlayerSnapshot,
    pub active_task: Option<crate::core::model::runtime_task::RuntimeTaskSnapshot>,
    pub active_view: ActiveView,
    pub focus: FocusArea,
    pub source_label: Option<String>,
    pub startup_notice: Option<String>,
    pub footer_hints_enabled: bool,
    pub show_help: bool,
    pub playlist_browser: crate::core::model::tui::PlaylistBrowserSnapshot,
    pub selected_playlist_name: Option<String>,
    pub preview_name: Option<String>,
    pub preview_titles: Vec<String>,
    pub selected_preview_index: usize,
    pub preview_loading: bool,
    pub preview_error: Option<String>,
}

pub fn selected_playlist_name(&self) -> Option<&str> {
    self.selected_playlist_name.as_deref()
}

pub fn selected_preview_index(&self) -> usize {
    self.selected_preview_index
}

pub fn set_playlist_preview(
    &mut self,
    preview: &crate::api::playlist::PlaylistPreviewResponse,
) {
    self.preview_name = Some(preview.name.clone());
    self.preview_titles = preview.songs.iter().map(|song| song.title.clone()).collect();
    self.preview_loading = false;
    self.preview_error = None;
    if self.selected_preview_index >= self.preview_titles.len() {
        self.selected_preview_index = self.preview_titles.len().saturating_sub(1);
    }
}

pub fn set_playlist_preview_loading(&mut self) {
    self.preview_loading = true;
    self.preview_error = None;
}

pub fn set_playlist_preview_error(&mut self, message: impl Into<String>) {
    self.preview_loading = false;
    self.preview_error = Some(message.into());
    self.preview_titles.clear();
    self.selected_preview_index = 0;
}
```

In `apply_tui_snapshot`, reconcile selection by name instead of always resetting to the default:

```rust
pub fn apply_tui_snapshot(&mut self, snapshot: crate::core::model::tui::TuiSnapshot) {
    self.apply_snapshot(snapshot.player);
    self.active_task = snapshot.active_task;
    self.playlist_browser = snapshot.playlist_browser;
    self.active_view = ActiveView::Playlist;

    let selected_still_exists = self
        .selected_playlist_name
        .as_ref()
        .is_some_and(|selected| {
            self.playlist_browser
                .visible_playlists
                .iter()
                .any(|playlist| &playlist.name == selected)
        });

    if !selected_still_exists {
        self.selected_playlist_name = self
            .playlist_browser
            .default_selected_playlist
            .clone()
            .or_else(|| {
                self.playlist_browser
                    .visible_playlists
                    .first()
                    .map(|playlist| playlist.name.clone())
            });
    }
}
```

Create `src/tui/ui/playlist.rs`:

```rust
pub fn render_playlist_lines(app: &crate::tui::app::App) -> Vec<String> {
    let mut lines = Vec::new();
    if let Some(current) = &app.playlist_browser.current_playing_playlist {
        lines.push("当前播放来源".to_string());
        lines.push(format!("> {} ({})", current.name, current.kind));
        lines.push(String::new());
    }
    lines.push("播放列表".to_string());

    for playlist in &app.playlist_browser.visible_playlists {
        let marker = if app.selected_playlist_name() == Some(playlist.name.as_str()) {
            ">"
        } else {
            " "
        };
        lines.push(format!("{marker} {} ({})", playlist.name, playlist.count));
    }

    lines
}

pub fn render_status_lines(app: &crate::tui::app::App) -> Vec<String> {
    vec![
        format!(
            "当前播放列表：{}",
            app.playlist_browser
                .current_playing_playlist
                .as_ref()
                .map(|playlist| playlist.name.as_str())
                .unwrap_or("无")
        ),
        format!("repeat={}", app.player.repeat_mode),
        format!("shuffle={}", app.player.shuffle_enabled),
    ]
}

pub fn render_preview_lines(app: &crate::tui::app::App) -> Vec<String> {
    if app.preview_loading {
        return vec!["加载中...".to_string()];
    }
    if let Some(error) = &app.preview_error {
        return vec![format!("ERR {error}")];
    }
    if app.preview_titles.is_empty() {
        return vec!["暂无歌曲".to_string()];
    }

    app.preview_titles
        .iter()
        .enumerate()
        .map(|(index, title)| {
            if index == app.selected_preview_index {
                format!("> {title}")
            } else {
                format!("  {title}")
            }
        })
        .collect()
}
```

Update `src/tui/ui/layout.rs` to split the right pane:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AppLayout {
    pub task_bar: Option<Rect>,
    pub sidebar: Rect,
    pub content_header: Rect,
    pub content_body: Rect,
    pub playbar: Rect,
}

let horizontal = Layout::default()
    .direction(Direction::Horizontal)
    .constraints([Constraint::Length(32), Constraint::Min(0)])
    .split(body);
let right = Layout::default()
    .direction(Direction::Vertical)
    .constraints([Constraint::Length(6), Constraint::Min(0)])
    .split(horizontal[1]);

AppLayout {
    task_bar: show_task_bar.then_some(vertical[0]),
    sidebar: horizontal[0],
    content_header: right[0],
    content_body: right[1],
    playbar: *vertical.last().unwrap(),
}
```

- [ ] **Step 4: Run the focused TUI tests again**

Run: `cargo test app_uses_default_selected_playlist_only_for_initial_selection tab_switches_focus_between_playlist_list_and_preview enter_on_playlist_list_requests_play_from_start footer_status_still_includes_repeat_and_shuffle_after_playlist_snapshot_applies -- --nocapture`

Expected: PASS.

- [ ] **Step 5: Run full verification and commit the TUI state/render slice**

Run: `pnpm qa`

Expected: PASS.

Run:

```bash
git add src/tui/app.rs src/tui/app/tests.rs src/tui/event.rs src/tui/ui/layout.rs src/tui/ui/playlist.rs src/tui/ui/mod.rs tests/tui_app.rs
git commit -m "feat: add playlist-centered tui state"
```

## Task 5: Wire the TUI Runtime to Home/Preview/Play APIs and Add Final Regressions

**Files:**
- Modify: `src/tui/run.rs`
- Modify: `src/tui/run/tests.rs`
- Modify: `src/tui/ui/popup.rs`
- Create: `tests/tui_home.rs`

- [ ] **Step 1: Write the failing runtime helper and end-to-end snapshot tests**

Add this to `src/tui/run/tests.rs`:

```rust
#[test]
fn repeat_mode_cycles_off_all_one_off() {
    assert_eq!(crate::tui::run::next_repeat_mode("off"), "all");
    assert_eq!(crate::tui::run::next_repeat_mode("all"), "one");
    assert_eq!(crate::tui::run::next_repeat_mode("one"), "off");
}
```

Create `tests/tui_home.rs` with:

```rust
use melo::core::config::settings::Settings;
use melo::core::db::bootstrap::DatabaseBootstrap;
use melo::daemon::app::AppState;

#[tokio::test(flavor = "multi_thread")]
async fn direct_open_updates_tui_home_default_selected_playlist() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::write(temp.path().join("01-first.flac"), b"audio").unwrap();

    let mut settings = Settings::for_test(temp.path().join("melo.db"));
    settings.open.max_depth = 1;
    settings.open.prewarm_limit = 1;
    DatabaseBootstrap::new(&settings).init().await.unwrap();

    let state = AppState::for_test_with_settings(settings.clone()).await;
    let response = state
        .open_target(melo::domain::open::service::OpenRequest {
            target: temp.path().to_string_lossy().to_string(),
            mode: "cwd_dir".to_string(),
        })
        .await
        .unwrap();

    let snapshot = state.tui_snapshot().await.unwrap();
    assert_eq!(response.playlist_name, temp.path().to_string_lossy());
    assert_eq!(
        snapshot.playlist_browser.default_selected_playlist.as_deref(),
        Some(temp.path().to_string_lossy().as_ref())
    );
    assert_eq!(
        snapshot.playlist_browser.default_view,
        melo::core::model::tui::TuiViewKind::Playlist
    );
}
```

- [ ] **Step 2: Run the runtime tests to confirm they fail**

Run: `cargo test repeat_mode_cycles_off_all_one_off direct_open_updates_tui_home_default_selected_playlist -- --nocapture`

Expected: FAIL because `next_repeat_mode` is not exposed and `tui_snapshot()` does not yet fully drive the runtime flow.

- [ ] **Step 3: Wire `run.rs` to `/api/tui/home`, preview loads, play actions, and mode toggles**

Update `src/tui/run.rs` with:

```rust
pub(crate) fn next_repeat_mode(current: &str) -> &'static str {
    match current {
        "off" => "all",
        "all" => "one",
        _ => "off",
    }
}

let home = api_client.tui_home().await?;
let mut app = crate::tui::app::App::new_for_test();
app.apply_tui_snapshot(home);
if let Some(selected) = app.selected_playlist_name().map(ToString::to_string) {
    app.set_playlist_preview_loading();
    match api_client.playlist_preview(&selected).await {
        Ok(preview) => app.set_playlist_preview(&preview),
        Err(err) => app.set_playlist_preview_error(err.to_string()),
    }
}
```

Handle the new actions inside the key loop:

```rust
match app.handle_key(key) {
    Some(Action::LoadSelectedPlaylistPreview) => {
        if let Some(name) = app.selected_playlist_name().map(ToString::to_string) {
            app.set_playlist_preview_loading();
            match api_client.playlist_preview(&name).await {
                Ok(preview) => app.set_playlist_preview(&preview),
                Err(err) => app.set_playlist_preview_error(err.to_string()),
            }
        }
    }
    Some(Action::PlaySelectedPlaylistFromStart) => {
        if let Some(name) = app.selected_playlist_name().map(ToString::to_string) {
            let snapshot = api_client.playlist_play(&name, 0).await?;
            app.apply_tui_snapshot(snapshot);
        }
    }
    Some(Action::PlaySelectedPreviewSong) => {
        if let Some(name) = app.selected_playlist_name().map(ToString::to_string) {
            let snapshot = api_client
                .playlist_play(&name, app.selected_preview_index())
                .await?;
            app.apply_tui_snapshot(snapshot);
        }
    }
    Some(Action::CycleRepeatMode) => {
        app.apply_snapshot(
            api_client
                .player_mode_repeat(next_repeat_mode(&app.player.repeat_mode))
                .await?,
        );
    }
    Some(Action::ToggleShuffle) => {
        app.apply_snapshot(
            api_client
                .player_mode_shuffle(!app.player.shuffle_enabled)
                .await?,
        );
    }
    _ => {}
}
```

Render the new panels with `src/tui/ui/playlist.rs`:

```rust
let playlist_lines = crate::tui::ui::playlist::render_playlist_lines(&app).join("\n");
let status_lines = crate::tui::ui::playlist::render_status_lines(&app).join("\n");
let preview_lines = crate::tui::ui::playlist::render_preview_lines(&app).join("\n");

frame.render_widget(
    Paragraph::new(playlist_lines)
        .block(Block::default().borders(Borders::ALL).title("播放列表")),
    layout.sidebar,
);
frame.render_widget(
    Paragraph::new(status_lines)
        .block(Block::default().borders(Borders::ALL).title("当前播放来源")),
    layout.content_header,
);
frame.render_widget(
    Paragraph::new(preview_lines)
        .block(Block::default().borders(Borders::ALL).title("歌单预览")),
    layout.content_body,
);
```

Update `src/tui/ui/popup.rs` help lines to include:

```rust
vec![
    "Tab 切换焦点",
    "Enter 播放当前选择",
    "r 切换循环模式",
    "s 切换随机播放",
    "Space 播放/暂停",
    "? 打开帮助",
    "q 退出",
]
```

- [ ] **Step 4: Run the focused runtime and end-to-end tests again**

Run: `cargo test repeat_mode_cycles_off_all_one_off direct_open_updates_tui_home_default_selected_playlist -- --nocapture`

Expected: PASS.

- [ ] **Step 5: Run full verification and commit the finished feature**

Run: `pnpm qa`

Expected: PASS.

Run:

```bash
git add src/tui/run.rs src/tui/run/tests.rs src/tui/ui/popup.rs tests/tui_home.rs
git commit -m "feat: add playlist-first tui home flow"
```

## Self-Review

### Spec coverage

- TUI 首页默认进入 playlist 视图：Task 1 + Task 5
- direct-open 默认落到当前临时歌单：Task 2 + Task 5
- 当前播放来源与当前浏览选择分离：Task 1 + Task 4
- 左侧 `Enter` 从第 1 首播放整个歌单：Task 4 + Task 5
- 右侧 `Enter` 从所选歌曲开始播放整个歌单：Task 4 + Task 5
- `repeat / shuffle` 作为全局状态并可在 TUI 切换：Task 2 + Task 5
- UI 不暴露 `queue`，但内部仍以物化播放列表实现：Task 2 + Task 4
- 新 API / websocket / OpenAPI 契约：Task 1 + Task 3

### Placeholder scan

- 没有 `TBD`、`TODO`、`implement later`
- 每个代码步骤都给出了具体结构体、函数签名或测试代码
- 每个任务都有明确命令、预期结果和提交点

### Type consistency

- 统一使用 `PlayingPlaylistContext` / `PlayingPlaylistStore`
- 统一使用 `TuiViewKind::Playlist`
- 统一使用 `PlaylistBrowserSnapshot` / `PlaylistListItem`
- 统一使用 `replace_queue(items, start_index)`
- 统一使用 `Action::PlaySelectedPlaylistFromStart` / `Action::PlaySelectedPreviewSong`

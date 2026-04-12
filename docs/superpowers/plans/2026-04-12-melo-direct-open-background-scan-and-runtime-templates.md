# Melo Direct-Open Background Scan and Runtime Templates Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement two-phase directory direct-open so `melo` can enter TUI after a small prewarm set, continue scanning in background, and surface visible CLI/TUI progress with overridable high-information runtime templates.

**Architecture:** Add a small MiniJinja-based runtime template renderer plus an in-memory runtime task store. Refactor directory direct-open to discover files, prewarm only the first `open.prewarm_limit` items, then spawn a background scan coordinator that appends queue items in discovery order while updating a TUI-specific websocket snapshot. Keep i18n compatibility by using built-in default templates now and stable future i18n keys later.

**Tech Stack:** Rust, Tokio, Axum websocket, MiniJinja, Ratatui, SeaORM, assert_cmd, tempfile

---

## File Structure

### Config and template rendering

- Create: `src/core/runtime_templates.rs`
  - Responsibility: central runtime template renderer, built-in default scan templates, MiniJinja filters (`basename`, `truncate`), override fallback logic.
- Create: `src/core/runtime_templates/tests.rs`
  - Responsibility: unit tests for override precedence and invalid-template fallback.
- Modify: `src/core/mod.rs`
  - Responsibility: export the runtime template module.
- Modify: `src/core/config/settings.rs`
  - Responsibility: add `templates.runtime.scan.*` config structs and defaults.
- Modify: `config.example.toml`
  - Responsibility: document the new runtime template override block.
- Modify: `tests/config_loading.rs`
  - Responsibility: integration coverage for config parsing.

### Runtime task state and TUI snapshot model

- Create: `src/core/model/runtime_task.rs`
  - Responsibility: `RuntimeTaskKind`, `RuntimeTaskPhase`, `RuntimeTaskSnapshot`.
- Create: `src/core/model/tui.rs`
  - Responsibility: aggregate `TuiSnapshot { player, active_task }`.
- Modify: `src/core/model/mod.rs`
  - Responsibility: export new model modules.
- Create: `src/daemon/tasks.rs`
  - Responsibility: in-memory runtime task store, task handles, auto-clear after completed / failed terminal states.
- Create: `src/daemon/tasks/tests.rs`
  - Responsibility: task store behavior tests.
- Modify: `src/daemon/mod.rs`
  - Responsibility: export runtime task store module.
- Modify: `src/daemon/app.rs`
  - Responsibility: own a shared `RuntimeTaskStore`, expose TUI snapshot helpers, pass the store into `OpenService`.
- Modify: `src/daemon/app/tests.rs`
  - Responsibility: prove `AppState` exposes aggregated TUI state.

### Direct-open background scan flow

- Create: `src/domain/open/background_scan.rs`
  - Responsibility: background scan coordinator that reads remaining files concurrently, commits them in discovery order, updates playlists, queue, and runtime task state.
- Modify: `src/domain/open/mod.rs`
  - Responsibility: export the background scan module.
- Modify: `src/domain/library/service.rs`
  - Responsibility: add single-path scan helper reused by prewarm and background scan; keep existing scan code DRY.
- Modify: `src/domain/open/service.rs`
  - Responsibility: split directory open into discovery, prewarm, queue setup, and background handoff.
- Modify: `src/domain/open/service/tests.rs`
  - Responsibility: keep format / path classification tests aligned with new helper signatures.
- Create: `tests/direct_open_background_scan.rs`
  - Responsibility: integration tests for prewarm return timing, order-stable background queue growth, and playlist refresh.

### Websocket and TUI delivery

- Modify: `src/api/ws.rs`
  - Responsibility: keep `/api/ws/player`, add `/api/ws/tui`, stream aggregated snapshots.
- Modify: `src/daemon/server.rs`
  - Responsibility: register the new websocket route.
- Modify: `src/tui/client.rs`
  - Responsibility: point TUI traffic at `/api/ws/tui`, expose connect + one-shot snapshot helpers.
- Modify: `src/tui/ws_client.rs`
  - Responsibility: persistent websocket stream wrapper with `next_json<T>()`.
- Modify: `tests/api_server.rs`
  - Responsibility: websocket contract coverage for `/api/ws/tui`.
- Modify: `tests/cli_remote.rs`
  - Responsibility: keep the existing `TuiClient` integration test aligned with `TuiSnapshot`.

### CLI and TUI presentation

- Modify: `src/cli/run.rs`
  - Responsibility: print scan start / handoff messages from templates before entering TUI.
- Create: `src/cli/run/tests.rs`
  - Responsibility: unit tests for CLI scan notice rendering helpers.
- Modify: `src/tui/app.rs`
  - Responsibility: store the latest `active_task`, render a one-line task bar string, stop stuffing scan text into the footer.
- Modify: `src/tui/run.rs`
  - Responsibility: consume continuous `TuiSnapshot` updates and render the top task bar.
- Modify: `src/tui/ui/layout.rs`
  - Responsibility: reserve an optional single-line top bar area.
- Modify: `src/tui/run/tests.rs`
  - Responsibility: top-bar layout behavior tests.
- Modify: `tests/tui_app.rs`
  - Responsibility: task bar rendering and truncation tests.

## Task 1: Add Runtime Template Config and Renderer

**Files:**
- Create: `src/core/runtime_templates.rs`
- Create: `src/core/runtime_templates/tests.rs`
- Modify: `src/core/mod.rs`
- Modify: `src/core/config/settings.rs`
- Modify: `config.example.toml`
- Test: `tests/config_loading.rs`

- [ ] **Step 1: Write the failing config and renderer tests**

Add this test to `tests/config_loading.rs`:

```rust
#[test]
fn settings_load_runtime_scan_template_overrides() {
    let temp = tempdir().unwrap();
    let path = temp.path().join("config.toml");
    fs::write(
        &path,
        r#"
[database]
path = "local/melo.db"

[templates.runtime.scan]
cli_start = "Start {{ source_label }}"
cli_handoff = "Into TUI"
tui_active = "{{ indexed_count }} / {{ discovered_count }} · {{ current_item_name }}"
tui_done = "Done {{ queued_count }}"
tui_failed = "Failed {{ error_message }}"
"#,
    )
    .unwrap();

    let settings = Settings::load_from_path(&path).unwrap();

    assert_eq!(
        settings.templates.runtime.scan.cli_start.as_deref(),
        Some("Start {{ source_label }}")
    );
    assert_eq!(
        settings.templates.runtime.scan.tui_active.as_deref(),
        Some("{{ indexed_count }} / {{ discovered_count }} · {{ current_item_name }}")
    );
}
```

Create `src/core/runtime_templates/tests.rs` with:

```rust
use serde_json::json;

use crate::core::config::settings::Settings;
use crate::core::runtime_templates::{RuntimeTemplateKey, RuntimeTemplateRenderer};

#[test]
fn runtime_template_renderer_prefers_override_for_scan_messages() {
    let mut settings = Settings::default();
    settings.templates.runtime.scan.cli_start =
        Some("Start {{ source_label|basename }}".to_string());

    let rendered = RuntimeTemplateRenderer::default().render(
        &settings,
        RuntimeTemplateKey::CliScanStart,
        json!({ "source_label": "D:/Music/Aimer" }),
    );

    assert_eq!(rendered, "Start Aimer");
}

#[test]
fn runtime_template_renderer_falls_back_to_builtin_when_override_is_invalid() {
    let mut settings = Settings::default();
    settings.templates.runtime.scan.cli_start = Some("{{ source_label ".to_string());

    let rendered = RuntimeTemplateRenderer::default().render(
        &settings,
        RuntimeTemplateKey::CliScanStart,
        json!({ "source_label": "D:/Music/Aimer" }),
    );

    assert_eq!(rendered, "Scanning D:/Music/Aimer...");
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
rtk cargo test -q --test config_loading settings_load_runtime_scan_template_overrides
rtk cargo test -q runtime_template_renderer_falls_back_to_builtin_when_override_is_invalid --lib
```

Expected:

- `config_loading` fails because `Settings` has no `templates` field.
- `--lib` fails because `runtime_templates` module and renderer types do not exist.

- [ ] **Step 3: Implement config structs, defaults, and the runtime template renderer**

Update `src/core/config/settings.rs` with new config structs and defaults:

```rust
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct RuntimeScanTemplateSettings {
    pub cli_start: Option<String>,
    pub cli_handoff: Option<String>,
    pub tui_active: Option<String>,
    pub tui_done: Option<String>,
    pub tui_failed: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct RuntimeTemplateSettings {
    pub scan: RuntimeScanTemplateSettings,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct TemplateSettings {
    pub runtime: RuntimeTemplateSettings,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Settings {
    pub database: DatabaseSettings,
    #[serde(default)]
    pub daemon: DaemonSettings,
    #[serde(default)]
    pub player: PlayerSettings,
    #[serde(default)]
    pub open: OpenSettings,
    #[serde(default)]
    pub library: LibrarySettings,
    #[serde(default)]
    pub playlists: PlaylistSettings,
    #[serde(default)]
    pub tui: TuiSettings,
    #[serde(default)]
    pub templates: TemplateSettings,
}
```

Create `src/core/runtime_templates.rs`:

```rust
use minijinja::Environment;
use serde_json::Value;

use crate::core::config::settings::Settings;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeTemplateKey {
    CliScanStart,
    CliScanHandoff,
    TuiScanActive,
    TuiScanDone,
    TuiScanFailed,
}

pub struct RuntimeTemplateRenderer {
    env: Environment<'static>,
}

impl RuntimeTemplateRenderer {
    pub fn render(&self, settings: &Settings, key: RuntimeTemplateKey, context: Value) -> String {
        let override_text = self.override_template(settings, key);
        let builtin = self.builtin_template(key);

        override_text
            .and_then(|template| self.render_str(template, &context).ok())
            .or_else(|| self.render_str(builtin, &context).ok())
            .unwrap_or_else(|| builtin.to_string())
    }
}
```

Finish the module by wiring a `Default` impl that adds the filters and fallback lookup:

```rust
impl Default for RuntimeTemplateRenderer {
    fn default() -> Self {
        let mut env = Environment::new();
        env.add_filter("basename", |value: String| {
            std::path::Path::new(&value)
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or(value.as_str())
                .to_string()
        });
        env.add_filter("truncate", |value: String, width: usize| {
            let mut out = String::new();
            for ch in value.chars() {
                if out.chars().count() + 1 >= width {
                    out.push('…');
                    break;
                }
                out.push(ch);
            }
            if out.is_empty() { value } else { out }
        });
        Self { env }
    }
}
```

Export the module from `src/core/mod.rs`:

```rust
pub mod config;
pub mod db;
pub mod error;
pub mod model;
pub mod runtime_templates;
```

Document the new config in `config.example.toml`:

```toml
[templates.runtime.scan]
# 目录扫描开始时输出给 CLI 的提示。
cli_start = "Scanning {{ source_label }}..."
# 预热完成、即将进入 TUI 时输出给 CLI 的提示。
cli_handoff = "Launching TUI, background scan continues..."
# TUI 顶部活动任务条。
tui_active = "Scanning {{ source_label }}... {{ indexed_count }} / {{ discovered_count }} · {{ current_item_name }}"
# 扫描完成后短暂显示的任务摘要。
tui_done = "Scan complete: {{ queued_count }} tracks indexed"
# 扫描失败后短暂显示的任务摘要。
tui_failed = "Scan failed: {{ error_message }}"
```

- [ ] **Step 4: Run tests to verify they pass**

Run:

```bash
rtk cargo test -q --test config_loading settings_load_runtime_scan_template_overrides
rtk cargo test -q runtime_template_renderer_prefers_override_for_scan_messages --lib
rtk cargo test -q runtime_template_renderer_falls_back_to_builtin_when_override_is_invalid --lib
```

Expected: all three commands PASS.

- [ ] **Step 5: Commit**

```bash
rtk git add src/core/runtime_templates.rs src/core/runtime_templates/tests.rs src/core/mod.rs src/core/config/settings.rs config.example.toml tests/config_loading.rs
rtk git commit -m "feat(config): add runtime scan templates"
```

## Task 2: Add Runtime Task Snapshots and Daemon Store

**Files:**
- Create: `src/core/model/runtime_task.rs`
- Create: `src/core/model/tui.rs`
- Modify: `src/core/model/mod.rs`
- Create: `src/daemon/tasks.rs`
- Create: `src/daemon/tasks/tests.rs`
- Modify: `src/daemon/mod.rs`
- Modify: `src/daemon/app.rs`
- Modify: `src/daemon/app/tests.rs`

- [ ] **Step 1: Write the failing task-store and TUI snapshot tests**

Create `src/daemon/tasks/tests.rs`:

```rust
use std::time::Duration;

use crate::core::model::runtime_task::RuntimeTaskPhase;
use crate::daemon::tasks::RuntimeTaskStore;

#[tokio::test(start_paused = true)]
async fn runtime_task_store_tracks_active_scan_progress() {
    let store = RuntimeTaskStore::new();
    let mut receiver = store.subscribe();
    let handle = store.start_scan("D:/Music/Aimer".to_string(), 3);

    handle.mark_prewarming(Some("01-Blue Bird.flac".to_string()));
    receiver.changed().await.unwrap();
    let snapshot = receiver.borrow().clone().unwrap();

    assert_eq!(snapshot.phase, RuntimeTaskPhase::Prewarming);
    assert_eq!(snapshot.discovered_count, 3);
    assert_eq!(snapshot.current_item_name.as_deref(), Some("01-Blue Bird.flac"));
}

#[tokio::test(start_paused = true)]
async fn runtime_task_store_clears_completed_snapshot_after_delay() {
    let store = RuntimeTaskStore::new();
    let mut receiver = store.subscribe();
    let handle = store.start_scan("D:/Music/Aimer".to_string(), 2);

    handle.mark_completed(2);
    receiver.changed().await.unwrap();
    assert!(receiver.borrow().is_some());

    tokio::time::advance(Duration::from_secs(3)).await;
    receiver.changed().await.unwrap();
    assert!(receiver.borrow().is_none());
}
```

Add this test to `src/daemon/app/tests.rs`:

```rust
#[tokio::test]
async fn app_state_tui_snapshot_includes_active_runtime_task() {
    let state = crate::daemon::app::AppState::for_test().await;
    let handle = state.runtime_tasks().start_scan("D:/Music/Aimer".to_string(), 4);
    handle.mark_indexing(1, 1, Some("track-01.flac".to_string()));

    let snapshot = state.tui_snapshot().await;

    assert_eq!(snapshot.player.backend_name, "noop");
    assert_eq!(
        snapshot.active_task.unwrap().current_item_name.as_deref(),
        Some("track-01.flac")
    );
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
rtk cargo test -q runtime_task_store_tracks_active_scan_progress --lib
rtk cargo test -q runtime_task_store_clears_completed_snapshot_after_delay --lib
rtk cargo test -q app_state_tui_snapshot_includes_active_runtime_task --lib
```

Expected:

- the first two tests fail because `RuntimeTaskStore` does not exist.
- the app-state test fails because there is no TUI snapshot model or `runtime_tasks()` accessor.

- [ ] **Step 3: Implement runtime task models, store, and AppState wiring**

Create `src/core/model/runtime_task.rs`:

```rust
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeTaskKind {
    LibraryScan,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeTaskPhase {
    Discovering,
    Prewarming,
    Indexing,
    Completed,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, ToSchema)]
pub struct RuntimeTaskSnapshot {
    pub task_id: String,
    pub kind: RuntimeTaskKind,
    pub phase: RuntimeTaskPhase,
    pub source_label: String,
    pub discovered_count: usize,
    pub indexed_count: usize,
    pub queued_count: usize,
    pub current_item_name: Option<String>,
    pub last_error: Option<String>,
}
```

Create `src/core/model/tui.rs`:

```rust
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::core::model::player::PlayerSnapshot;
use crate::core::model::runtime_task::RuntimeTaskSnapshot;

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, ToSchema)]
pub struct TuiSnapshot {
    pub player: PlayerSnapshot,
    pub active_task: Option<RuntimeTaskSnapshot>,
}
```

Export the models in `src/core/model/mod.rs`:

```rust
pub mod player;
pub mod runtime_task;
pub mod tui;
```

Create `src/daemon/tasks.rs`:

```rust
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{watch, Mutex};
use uuid::Uuid;

use crate::core::model::runtime_task::{RuntimeTaskKind, RuntimeTaskPhase, RuntimeTaskSnapshot};

#[derive(Clone)]
pub struct RuntimeTaskStore {
    snapshot_tx: watch::Sender<Option<RuntimeTaskSnapshot>>,
    success_ttl: Duration,
    failure_ttl: Duration,
}

#[derive(Clone)]
pub struct RuntimeTaskHandle {
    store: RuntimeTaskStore,
    snapshot: Arc<Mutex<RuntimeTaskSnapshot>>,
}
```

Implement the store and handle methods:

```rust
impl RuntimeTaskStore {
    pub fn new() -> Self {
        let (snapshot_tx, _snapshot_rx) = watch::channel(None);
        Self {
            snapshot_tx,
            success_ttl: Duration::from_secs(3),
            failure_ttl: Duration::from_secs(5),
        }
    }

    pub fn subscribe(&self) -> watch::Receiver<Option<RuntimeTaskSnapshot>> {
        self.snapshot_tx.subscribe()
    }

    pub fn current(&self) -> Option<RuntimeTaskSnapshot> {
        self.snapshot_tx.borrow().clone()
    }

    pub fn start_scan(&self, source_label: String, discovered_count: usize) -> RuntimeTaskHandle {
        let snapshot = RuntimeTaskSnapshot {
            task_id: Uuid::new_v4().to_string(),
            kind: RuntimeTaskKind::LibraryScan,
            phase: RuntimeTaskPhase::Discovering,
            source_label,
            discovered_count,
            indexed_count: 0,
            queued_count: 0,
            current_item_name: None,
            last_error: None,
        };
        self.snapshot_tx.send_replace(Some(snapshot.clone()));
        RuntimeTaskHandle {
            store: self.clone(),
            snapshot: Arc::new(Mutex::new(snapshot)),
        }
    }
}

impl RuntimeTaskHandle {
    pub fn mark_prewarming(&self, current_item_name: Option<String>) {
        self.update(RuntimeTaskPhase::Prewarming, None, current_item_name, None);
    }

    pub fn mark_indexing(&self, indexed_count: usize, queued_count: usize, current_item_name: Option<String>) {
        self.update(
            RuntimeTaskPhase::Indexing,
            Some((indexed_count, queued_count)),
            current_item_name,
            None,
        );
    }

    pub fn mark_completed(&self, queued_count: usize) {
        self.update(RuntimeTaskPhase::Completed, Some((queued_count, queued_count)), None, None);
        self.schedule_clear(self.store.success_ttl);
    }

    pub fn mark_failed(&self, error_message: String) {
        self.update(RuntimeTaskPhase::Failed, None, None, Some(error_message));
        self.schedule_clear(self.store.failure_ttl);
    }
}
```

Wire the store into `AppState` in `src/daemon/app.rs`:

```rust
#[derive(Clone)]
pub struct AppState {
    pub player: Arc<PlayerService>,
    pub settings: Settings,
    pub open: Arc<crate::domain::open::service::OpenService>,
    runtime_tasks: Arc<crate::daemon::tasks::RuntimeTaskStore>,
    runtime: Arc<DaemonRuntimeMeta>,
    shutdown_notify: Arc<Notify>,
    shutdown_requested: Arc<AtomicBool>,
}

pub fn runtime_tasks(&self) -> Arc<crate::daemon::tasks::RuntimeTaskStore> {
    Arc::clone(&self.runtime_tasks)
}

pub async fn tui_snapshot(&self) -> crate::core::model::tui::TuiSnapshot {
    crate::core::model::tui::TuiSnapshot {
        player: self.player.snapshot().await,
        active_task: self.runtime_tasks.current(),
    }
}
```

Pass the store into `OpenService::new(...)` from every `AppState` constructor path.

- [ ] **Step 4: Run tests to verify they pass**

Run:

```bash
rtk cargo test -q runtime_task_store_tracks_active_scan_progress --lib
rtk cargo test -q runtime_task_store_clears_completed_snapshot_after_delay --lib
rtk cargo test -q app_state_tui_snapshot_includes_active_runtime_task --lib
```

Expected: all three commands PASS.

- [ ] **Step 5: Commit**

```bash
rtk git add src/core/model/runtime_task.rs src/core/model/tui.rs src/core/model/mod.rs src/daemon/tasks.rs src/daemon/tasks/tests.rs src/daemon/mod.rs src/daemon/app.rs src/daemon/app/tests.rs
rtk git commit -m "feat(daemon): add runtime task snapshots"
```

## Task 3: Refactor Directory Open into Prewarm + Background Scan

**Files:**
- Create: `src/domain/open/background_scan.rs`
- Modify: `src/domain/open/mod.rs`
- Modify: `src/domain/library/service.rs`
- Modify: `src/domain/open/service.rs`
- Modify: `src/domain/open/service/tests.rs`
- Create: `tests/direct_open_background_scan.rs`

- [ ] **Step 1: Write the failing background scan regression tests**

Create `tests/direct_open_background_scan.rs`:

```rust
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use melo::core::config::settings::Settings;
use melo::core::error::MeloResult;
use melo::daemon::tasks::RuntimeTaskStore;
use melo::domain::library::metadata::{LyricsSourceKind, MetadataReader, SongMetadata};
use melo::domain::library::service::LibraryService;
use melo::domain::open::service::{OpenRequest, OpenService};
use melo::domain::player::backend::NoopBackend;
use melo::domain::player::service::PlayerService;
use melo::domain::playlist::service::PlaylistService;

struct SlowReader {
    delays: HashMap<String, Duration>,
}

impl MetadataReader for SlowReader {
    fn read(&self, path: &Path) -> MeloResult<SongMetadata> {
        if let Some(delay) = self
            .delays
            .get(path.file_name().and_then(|name| name.to_str()).unwrap_or_default())
        {
            std::thread::sleep(*delay);
        }

        Ok(SongMetadata {
            title: path.file_stem().unwrap().to_string_lossy().to_string(),
            artist: Some("Aimer".to_string()),
            album: Some("Singles".to_string()),
            track_no: None,
            disc_no: None,
            duration_seconds: Some(180.0),
            genre: None,
            lyrics: None,
            lyrics_source_kind: LyricsSourceKind::None,
            lyrics_format: None,
            embedded_artwork: None,
            format: Some("flac".to_string()),
            bitrate: None,
            sample_rate: None,
            bit_depth: None,
            channels: None,
        })
    }
}
```

Add the first regression test:

```rust
#[tokio::test(flavor = "multi_thread")]
async fn directory_open_returns_after_prewarm_and_background_scan_finishes_later() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::write(temp.path().join("01-first.flac"), b"audio").unwrap();
    std::fs::write(temp.path().join("02-second.flac"), b"audio").unwrap();
    std::fs::write(temp.path().join("03-third.flac"), b"audio").unwrap();

    let mut settings = Settings::for_test(temp.path().join("melo.db"));
    settings.open.max_depth = 1;
    settings.open.prewarm_limit = 1;
    settings.open.background_jobs = 2;

    let player = Arc::new(PlayerService::new(Arc::new(NoopBackend)));
    player.start_runtime_event_loop();
    player.start_progress_loop();
    let library = LibraryService::new(
        settings.clone(),
        Arc::new(SlowReader {
            delays: HashMap::from([
                ("02-second.flac".to_string(), Duration::from_millis(250)),
                ("03-third.flac".to_string(), Duration::from_millis(250)),
            ]),
        }),
    );
    let playlists = PlaylistService::new(settings.clone());
    let tasks = Arc::new(RuntimeTaskStore::new());
    let open = OpenService::new(
        settings.clone(),
        library,
        playlists,
        Arc::clone(&player),
        Arc::clone(&tasks),
    );

    let started = Instant::now();
    let response = open
        .open(OpenRequest {
            target: temp.path().to_string_lossy().to_string(),
            mode: "path_dir".to_string(),
        })
        .await
        .unwrap();

    assert!(started.elapsed() < Duration::from_millis(200));
    assert_eq!(response.snapshot.queue_len, 1);

    let mut receiver = tasks.subscribe();
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if receiver
                .borrow()
                .clone()
                .is_some_and(|task| task.queued_count == 3)
            {
                break;
            }
            receiver.changed().await.unwrap();
        }
    })
    .await
    .unwrap();

    assert_eq!(player.snapshot().await.queue_len, 3);
}
```

Add the second regression test:

```rust
#[tokio::test(flavor = "multi_thread")]
async fn background_scan_appends_remaining_tracks_in_discovery_order() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::write(temp.path().join("01-first.flac"), b"audio").unwrap();
    std::fs::write(temp.path().join("02-second.flac"), b"audio").unwrap();
    std::fs::write(temp.path().join("03-third.flac"), b"audio").unwrap();

    let mut settings = Settings::for_test(temp.path().join("melo.db"));
    settings.open.max_depth = 1;
    settings.open.prewarm_limit = 1;
    settings.open.background_jobs = 2;

    let player = Arc::new(PlayerService::new(Arc::new(NoopBackend)));
    player.start_runtime_event_loop();
    player.start_progress_loop();
    let library = LibraryService::new(
        settings.clone(),
        Arc::new(SlowReader {
            delays: HashMap::from([
                ("02-second.flac".to_string(), Duration::from_millis(250)),
                ("03-third.flac".to_string(), Duration::from_millis(50)),
            ]),
        }),
    );
    let playlists = PlaylistService::new(settings.clone());
    let tasks = Arc::new(RuntimeTaskStore::new());
    let open = OpenService::new(
        settings.clone(),
        library,
        playlists,
        Arc::clone(&player),
        Arc::clone(&tasks),
    );

    open.open(OpenRequest {
        target: temp.path().to_string_lossy().to_string(),
        mode: "path_dir".to_string(),
    })
    .await
    .unwrap();

    let mut receiver = tasks.subscribe();
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if receiver
                .borrow()
                .clone()
                .is_some_and(|task| task.queued_count == 3)
            {
                break;
            }
            receiver.changed().await.unwrap();
        }
    })
    .await
    .unwrap();

    assert_eq!(
        player.snapshot().await.queue_preview,
        vec![
            "01-first".to_string(),
            "02-second".to_string(),
            "03-third".to_string()
        ]
    );
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
rtk cargo test -q --test direct_open_background_scan directory_open_returns_after_prewarm_and_background_scan_finishes_later
rtk cargo test -q --test direct_open_background_scan background_scan_appends_remaining_tracks_in_discovery_order
```

Expected:

- compilation fails because `RuntimeTaskStore` is not yet accepted by `OpenService::new`.
- even after compilation fixes, `open()` still waits for all three files before returning.

- [ ] **Step 3: Implement single-path scanning, background coordinator, and two-phase directory open**

Create `src/domain/open/background_scan.rs`:

```rust
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use futures_util::stream::{self, StreamExt};

use crate::core::config::settings::Settings;
use crate::daemon::tasks::RuntimeTaskHandle;
use crate::domain::library::service::LibraryService;
use crate::domain::player::service::PlayerService;
use crate::domain::playlist::service::PlaylistService;

#[derive(Clone)]
pub struct BackgroundScanCoordinator {
    settings: Settings,
    library: LibraryService,
    playlists: PlaylistService,
    player: Arc<PlayerService>,
}
```

Implement the worker with order-preserving commit:

```rust
impl BackgroundScanCoordinator {
    pub fn spawn(
        &self,
        task: RuntimeTaskHandle,
        source_name: String,
        source_kind: String,
        source_key: String,
        visible: bool,
        expires_at: Option<String>,
        initial_song_ids: Vec<i64>,
        remaining_paths: Vec<PathBuf>,
    ) {
        let this = self.clone();
        tokio::spawn(async move {
            let mut all_song_ids = initial_song_ids;
            let mut next_index = 0usize;
            let mut ready = BTreeMap::new();

            let mut stream = stream::iter(remaining_paths.into_iter().enumerate())
                .map(|(index, path)| {
                    let library = this.library.clone();
                    async move {
                        let song_id = library.ensure_song_id_for_path(&path).await?;
                        Ok::<_, crate::core::error::MeloError>((index, path, song_id))
                    }
                })
                .buffer_unordered(this.settings.open.background_jobs.max(1));

            while let Some(result) = stream.next().await {
                match result {
                    Ok((index, path, song_id)) => {
                        ready.insert(index, (path, song_id));
                        while let Some((path, song_id)) = ready.remove(&next_index) {
                            all_song_ids.push(song_id);
                            let current_item_name = path
                                .file_name()
                                .and_then(|name| name.to_str())
                                .map(ToString::to_string);
                            task.mark_indexing(all_song_ids.len(), all_song_ids.len(), current_item_name);

                            this.playlists
                                .upsert_ephemeral(
                                    &source_name,
                                    &source_kind,
                                    &source_key,
                                    visible,
                                    expires_at.as_deref(),
                                    &all_song_ids,
                                )
                                .await?;
                            let queue_item = this
                                .library
                                .queue_items_for_song_ids(&[song_id])
                                .await?
                                .into_iter()
                                .next()
                                .unwrap();
                            this.player.append(queue_item).await?;
                            next_index += 1;
                        }
                    }
                    Err(err) => {
                        task.mark_failed(err.to_string());
                        return;
                    }
                }
            }

            task.mark_completed(all_song_ids.len());
        });
    }
}
```

Refactor `src/domain/library/service.rs` to make scanning reusable one path at a time:

```rust
#[derive(Clone)]
pub struct LibraryService {
    settings: Settings,
    reader: Arc<dyn MetadataReader>,
    repository: LibraryRepository,
}

pub async fn ensure_song_id_for_path(&self, path: &std::path::Path) -> MeloResult<i64> {
    let mut metadata = self.reader.read(path)?;
    let mut lyrics_source_path = None;
    if let Some(resolved_lyrics) = crate::domain::library::assets::resolve_lyrics(path, &metadata) {
        metadata.lyrics = Some(resolved_lyrics.text);
        metadata.lyrics_source_kind = resolved_lyrics.source_kind;
        metadata.lyrics_format = Some(resolved_lyrics.format);
        lyrics_source_path = resolved_lyrics.source_path;
    } else {
        metadata.lyrics = None;
        metadata.lyrics_source_kind = LyricsSourceKind::None;
        metadata.lyrics_format = None;
    }
    let cover_path = crate::domain::library::assets::find_cover(path);
    self.repository
        .upsert_song(path, &metadata, lyrics_source_path.as_deref(), cover_path.as_deref())
        .await
}
```

Update `src/domain/open/service.rs`:

```rust
pub struct OpenService {
    settings: Settings,
    library: LibraryService,
    playlists: PlaylistService,
    player: Arc<PlayerService>,
    tasks: Arc<crate::daemon::tasks::RuntimeTaskStore>,
}

pub fn new(
    settings: Settings,
    library: LibraryService,
    playlists: PlaylistService,
    player: Arc<PlayerService>,
    tasks: Arc<crate::daemon::tasks::RuntimeTaskStore>,
) -> Self {
    Self { settings, library, playlists, player, tasks }
}
```

Split directory open into a helper:

```rust
async fn open_directory_target(&self, request: &OpenRequest, path: PathBuf) -> MeloResult<OpenResponse> {
    let audio_paths = discover_audio_paths(&path, self.settings.open.max_depth)?;
    if audio_paths.is_empty() {
        return Err(MeloError::Message("open_target_empty".to_string()));
    }

    let task = self
        .tasks
        .start_scan(request.target.clone(), audio_paths.len());

    let split_at = self.settings.open.prewarm_limit.min(audio_paths.len()).max(1);
    let mut song_ids = Vec::with_capacity(split_at);
    for audio_path in &audio_paths[..split_at] {
        let current_item_name = audio_path.file_name().and_then(|name| name.to_str()).map(ToString::to_string);
        task.mark_prewarming(current_item_name);
        song_ids.push(self.library.ensure_song_id_for_path(audio_path).await?);
    }

    let expires_at = self.expires_at();
    let playlist = self
        .playlists
        .upsert_ephemeral(
            &request.target,
            &request.mode,
            &request.target,
            self.playlist_visibility(&request.mode),
            expires_at.as_deref(),
            &song_ids,
        )
        .await?;

    self.player.clear().await?;
    for item in self.library.queue_items_for_song_ids(&song_ids).await? {
        self.player.append(item).await?;
    }
    let snapshot = self.player.play().await?;

    let remaining_paths = audio_paths[split_at..].to_vec();
    if !remaining_paths.is_empty() {
        BackgroundScanCoordinator {
            settings: self.settings.clone(),
            library: self.library.clone(),
            playlists: self.playlists.clone(),
            player: Arc::clone(&self.player),
        }
        .spawn(
            task,
            request.target.clone(),
            request.mode.clone(),
            request.target.clone(),
            self.playlist_visibility(&request.mode),
            expires_at,
            song_ids.clone(),
            remaining_paths,
        );
    } else {
        task.mark_completed(song_ids.len());
    }

    Ok(OpenResponse {
        snapshot,
        playlist_name: playlist.name,
        source_label: request.target.clone(),
    })
}
```

Make `open()` dispatch explicitly so the file path remains synchronous:

```rust
pub async fn open(&self, request: OpenRequest) -> MeloResult<OpenResponse> {
    match classify_target(Path::new(&request.target))? {
        OpenTarget::AudioFile(path) => {
            let song_ids = vec![self.library.ensure_song_id_for_path(&path).await?];
            let expires_at = self.expires_at();
            let playlist = self
                .playlists
                .upsert_ephemeral(
                    &request.target,
                    &request.mode,
                    &request.target,
                    self.playlist_visibility(&request.mode),
                    expires_at.as_deref(),
                    &song_ids,
                )
                .await?;
            self.player.clear().await?;
            for item in self.library.queue_items_for_song_ids(&song_ids).await? {
                self.player.append(item).await?;
            }
            let snapshot = self.player.play().await?;
            Ok(OpenResponse {
                snapshot,
                playlist_name: playlist.name,
                source_label: request.target,
            })
        }
        OpenTarget::Directory(path) => self.open_directory_target(&request, path).await,
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run:

```bash
rtk cargo test -q --test direct_open_background_scan directory_open_returns_after_prewarm_and_background_scan_finishes_later
rtk cargo test -q --test direct_open_background_scan background_scan_appends_remaining_tracks_in_discovery_order
rtk cargo test -q discover_audio_paths_respects_max_depth --lib
```

Expected: all three commands PASS.

- [ ] **Step 5: Commit**

```bash
rtk git add src/domain/open/background_scan.rs src/domain/open/mod.rs src/domain/library/service.rs src/domain/open/service.rs src/domain/open/service/tests.rs tests/direct_open_background_scan.rs
rtk git commit -m "feat(open): scan directories in background after prewarm"
```

## Task 4: Add `/api/ws/tui` and Persistent TUI Snapshot Streaming

**Files:**
- Modify: `src/api/ws.rs`
- Modify: `src/daemon/server.rs`
- Modify: `src/tui/client.rs`
- Modify: `src/tui/ws_client.rs`
- Modify: `tests/api_server.rs`
- Modify: `tests/cli_remote.rs`

- [ ] **Step 1: Write the failing websocket aggregation tests**

Add this websocket test to `tests/api_server.rs`:

```rust
#[tokio::test(flavor = "multi_thread")]
async fn api_tui_websocket_initial_snapshot_includes_active_task() {
    let state = melo::daemon::app::AppState::for_test().await;
    let handle = state.runtime_tasks().start_scan("D:/Music/Aimer".to_string(), 4);
    handle.mark_indexing(2, 2, Some("track-02.flac".to_string()));
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

    assert_eq!(snapshot.player.backend_name, "noop");
    assert_eq!(snapshot.active_task.unwrap().indexed_count, 2);
}
```

Update the existing `tests/cli_remote.rs` TUI client test to assert the new aggregate shape:

```rust
#[tokio::test(flavor = "multi_thread")]
async fn tui_client_receives_initial_tui_snapshot() {
    let state = melo::daemon::app::AppState::for_test().await;
    state
        .player
        .append(melo::core::model::player::QueueItem {
            song_id: 7,
            path: "tests/fixtures/full_test.mp3".into(),
            title: "Blue Bird".into(),
            duration_seconds: Some(212.0),
        })
        .await
        .unwrap();
    state.player.play().await.unwrap();
    let handle = state.runtime_tasks().start_scan("D:/Music/Aimer".to_string(), 3);
    handle.mark_indexing(1, 1, Some("Blue Bird.flac".to_string()));
    let app = melo::daemon::server::router(state);
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let client = melo::tui::client::TuiClient::new(format!("http://{addr}"));
    let snapshot = client.next_snapshot().await.unwrap();

    assert_eq!(snapshot.player.playback_state, "playing");
    assert_eq!(snapshot.player.current_song.unwrap().title, "Blue Bird");
    assert_eq!(snapshot.active_task.unwrap().indexed_count, 1);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
rtk cargo test -q --test api_server api_tui_websocket_initial_snapshot_includes_active_task
rtk cargo test -q --test cli_remote tui_client_receives_initial_tui_snapshot
```

Expected:

- `/api/ws/tui` route is missing.
- `TuiClient::next_snapshot()` still returns `PlayerSnapshot`, not `TuiSnapshot`.

- [ ] **Step 3: Implement the aggregate websocket route and persistent TUI websocket client**

Extend `src/api/ws.rs`:

```rust
#[utoipa::path(
    get,
    path = "/api/ws/tui",
    responses((status = 101, description = "升级为 TUI 聚合状态 WebSocket 流"))
)]
pub async fn tui_updates(socket: WebSocketUpgrade, State(state): State<AppState>) -> Response {
    socket.on_upgrade(move |websocket| stream_tui_snapshots(websocket, state))
}

async fn stream_tui_snapshots(mut socket: WebSocket, state: AppState) {
    let mut player_rx = state.player.subscribe();
    let mut task_rx = state.runtime_tasks().subscribe();

    if send_tui_snapshot(&mut socket, state.tui_snapshot().await).await.is_err() {
        return;
    }

    loop {
        tokio::select! {
            changed = player_rx.changed() => if changed.is_err() { break; },
            changed = task_rx.changed() => if changed.is_err() { break; },
        }

        if send_tui_snapshot(&mut socket, state.tui_snapshot().await).await.is_err() {
            break;
        }
    }
}
```

Register the route in `src/daemon/server.rs`:

```rust
.route(
    "/api/ws/tui",
    axum::routing::get(crate::api::ws::tui_updates),
)
```

Refactor `src/tui/ws_client.rs` into a persistent stream wrapper:

```rust
use serde::de::DeserializeOwned;
use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};

pub struct WsSnapshotStream {
    stream: WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>,
}

impl WsClient {
    pub async fn connect(&self) -> MeloResult<WsSnapshotStream> {
        let (stream, _response) = connect_async(&self.url)
            .await
            .map_err(|err| MeloError::Message(err.to_string()))?;
        Ok(WsSnapshotStream { stream })
    }
}

impl WsSnapshotStream {
    pub async fn next_json<T>(&mut self) -> MeloResult<T>
    where
        T: DeserializeOwned,
    {
        while let Some(message) = self.stream.next().await {
            match message.map_err(|err| MeloError::Message(err.to_string()))? {
                Message::Text(text) => {
                    return serde_json::from_str::<T>(&text)
                        .map_err(|err| MeloError::Message(err.to_string()));
                }
                Message::Close(_) => break,
                _ => {}
            }
        }

        Err(MeloError::Message("WebSocket 未收到快照".to_string()))
    }
}
```

Update `src/tui/client.rs`:

```rust
use crate::core::model::tui::TuiSnapshot;

pub async fn connect(&self) -> MeloResult<crate::tui::ws_client::WsSnapshotStream> {
    self.ws_client.connect().await
}

pub async fn next_snapshot(&self) -> MeloResult<TuiSnapshot> {
    let mut stream = self.connect().await?;
    stream.next_json::<TuiSnapshot>().await
}
```

Point the TUI websocket URL at `/api/ws/tui` instead of `/api/ws/player`.

- [ ] **Step 4: Run tests to verify they pass**

Run:

```bash
rtk cargo test -q --test api_server api_tui_websocket_initial_snapshot_includes_active_task
rtk cargo test -q --test cli_remote tui_client_receives_initial_tui_snapshot
rtk cargo test -q --test api_server api_websocket_route_accepts_connections
```

Expected: all three commands PASS.

- [ ] **Step 5: Commit**

```bash
rtk git add src/api/ws.rs src/daemon/server.rs src/tui/client.rs src/tui/ws_client.rs tests/api_server.rs tests/cli_remote.rs
rtk git commit -m "feat(api): stream aggregated tui snapshots"
```

## Task 5: Render CLI Notices and the TUI Top Task Bar

**Files:**
- Modify: `src/cli/run.rs`
- Create: `src/cli/run/tests.rs`
- Modify: `src/tui/app.rs`
- Modify: `src/tui/run.rs`
- Modify: `src/tui/ui/layout.rs`
- Modify: `src/tui/run/tests.rs`
- Modify: `tests/tui_app.rs`

- [ ] **Step 1: Write the failing CLI and TUI rendering tests**

Create `src/cli/run/tests.rs`:

```rust
use crate::core::config::settings::Settings;
use crate::core::runtime_templates::RuntimeTemplateRenderer;

#[test]
fn render_scan_cli_lines_uses_runtime_template_overrides() {
    let mut settings = Settings::default();
    settings.templates.runtime.scan.cli_start = Some("Start {{ source_label|basename }}".to_string());
    settings.templates.runtime.scan.cli_handoff = Some("Into TUI".to_string());

    let renderer = RuntimeTemplateRenderer::default();
    let lines = super::render_scan_cli_lines(&renderer, &settings, "D:/Music/Aimer");

    assert_eq!(lines, vec!["Start Aimer".to_string(), "Into TUI".to_string()]);
}
```

Add these tests to `tests/tui_app.rs`:

```rust
#[test]
fn active_task_bar_renders_current_item_name() {
    let mut app = melo::tui::app::App::new_for_test();
    app.apply_tui_snapshot(melo::core::model::tui::TuiSnapshot {
        player: melo::core::model::player::PlayerSnapshot::default(),
        active_task: Some(melo::core::model::runtime_task::RuntimeTaskSnapshot {
            task_id: "scan-1".into(),
            kind: melo::core::model::runtime_task::RuntimeTaskKind::LibraryScan,
            phase: melo::core::model::runtime_task::RuntimeTaskPhase::Indexing,
            source_label: "D:/Music/Aimer".into(),
            discovered_count: 240,
            indexed_count: 12,
            queued_count: 12,
            current_item_name: Some("Ref:rain.flac".into()),
            last_error: None,
        }),
    });

    let text = app.task_bar_text(
        &melo::core::runtime_templates::RuntimeTemplateRenderer::default(),
        &melo::core::config::settings::Settings::default(),
        120,
    );

    assert!(text.is_some());
    assert!(text.unwrap().contains("Ref:rain.flac"));
}

#[test]
fn active_task_bar_truncates_long_text_to_available_width() {
    let mut app = melo::tui::app::App::new_for_test();
    app.apply_tui_snapshot(melo::core::model::tui::TuiSnapshot {
        player: melo::core::model::player::PlayerSnapshot::default(),
        active_task: Some(melo::core::model::runtime_task::RuntimeTaskSnapshot {
            task_id: "scan-1".into(),
            kind: melo::core::model::runtime_task::RuntimeTaskKind::LibraryScan,
            phase: melo::core::model::runtime_task::RuntimeTaskPhase::Indexing,
            source_label: "D:/Very/Long/Source/Path/That/Should/Be/Trimmed".into(),
            discovered_count: 240,
            indexed_count: 12,
            queued_count: 12,
            current_item_name: Some("A very very long filename that should not overflow.flac".into()),
            last_error: None,
        }),
    });

    let text = app
        .task_bar_text(
            &melo::core::runtime_templates::RuntimeTemplateRenderer::default(),
            &melo::core::config::settings::Settings::default(),
            40,
        )
        .unwrap();

    assert!(unicode_width::UnicodeWidthStr::width(text.as_str()) <= 40);
}
```

Update `src/tui/run/tests.rs`:

```rust
#[test]
fn top_task_bar_layout_only_reserves_space_when_needed() {
    let full = crate::tui::ui::layout::split(ratatui::layout::Rect::new(0, 0, 100, 30), true);
    let compact = crate::tui::ui::layout::split(ratatui::layout::Rect::new(0, 0, 100, 30), false);

    assert!(full.task_bar.is_some());
    assert!(compact.task_bar.is_none());
    assert!(full.content.y > compact.content.y);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
rtk cargo test -q render_scan_cli_lines_uses_runtime_template_overrides --lib
rtk cargo test -q --test tui_app active_task_bar_renders_current_item_name
rtk cargo test -q top_task_bar_layout_only_reserves_space_when_needed --lib
```

Expected:

- the CLI helper does not exist.
- `App` has no `apply_tui_snapshot` or `task_bar_text`.
- layout splitting does not accept a `show_task_bar` flag.

- [ ] **Step 3: Implement CLI notice helpers, TUI top-bar state, and continuous snapshot consumption**

Add CLI helper functions in `src/cli/run.rs`:

```rust
fn render_scan_cli_lines(
    renderer: &crate::core::runtime_templates::RuntimeTemplateRenderer,
    settings: &crate::core::config::settings::Settings,
    source_label: &str,
) -> Vec<String> {
    vec![
        renderer.render(
            settings,
            crate::core::runtime_templates::RuntimeTemplateKey::CliScanStart,
            serde_json::json!({ "source_label": source_label }),
        ),
        renderer.render(
            settings,
            crate::core::runtime_templates::RuntimeTemplateKey::CliScanHandoff,
            serde_json::json!({ "source_label": source_label }),
        ),
    ]
}
```

Print the first line before directory open, and the second line immediately before `tui::run::start(...)` in both:

- default `cwd_dir` path
- explicit `path_dir` direct-open path

Update `src/tui/app.rs`:

```rust
pub struct App {
    pub player: PlayerSnapshot,
    pub active_task: Option<crate::core::model::runtime_task::RuntimeTaskSnapshot>,
    pub active_view: ActiveView,
    pub focus: FocusArea,
    pub source_label: Option<String>,
    pub startup_notice: Option<String>,
    pub footer_hints_enabled: bool,
    pub show_help: bool,
    pub queue_titles: Vec<String>,
}

pub fn apply_tui_snapshot(&mut self, snapshot: crate::core::model::tui::TuiSnapshot) {
    self.apply_snapshot(snapshot.player);
    self.active_task = snapshot.active_task;
}

pub fn task_bar_text(
    &self,
    renderer: &crate::core::runtime_templates::RuntimeTemplateRenderer,
    settings: &crate::core::config::settings::Settings,
    width: usize,
) -> Option<String> {
    let task = self.active_task.as_ref()?;
    let key = match task.phase {
        crate::core::model::runtime_task::RuntimeTaskPhase::Completed => {
            crate::core::runtime_templates::RuntimeTemplateKey::TuiScanDone
        }
        crate::core::model::runtime_task::RuntimeTaskPhase::Failed => {
            crate::core::runtime_templates::RuntimeTemplateKey::TuiScanFailed
        }
        _ => crate::core::runtime_templates::RuntimeTemplateKey::TuiScanActive,
    };

    let rendered = renderer.render(
        settings,
        key,
        serde_json::json!({
            "source_label": task.source_label,
            "discovered_count": task.discovered_count,
            "indexed_count": task.indexed_count,
            "queued_count": task.queued_count,
            "current_item_name": task.current_item_name,
            "error_message": task.last_error,
        }),
    );

    Some(crate::tui::ui::content::render_song_title(&rendered, width))
}
```

Update `src/tui/ui/layout.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AppLayout {
    pub task_bar: Option<Rect>,
    pub sidebar: Rect,
    pub content: Rect,
    pub playbar: Rect,
}

pub fn split(area: Rect, show_task_bar: bool) -> AppLayout {
    let constraints = if show_task_bar {
        vec![Constraint::Length(1), Constraint::Min(0), Constraint::Length(3)]
    } else {
        vec![Constraint::Min(0), Constraint::Length(3)]
    };
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(area);

    let body = if show_task_bar { vertical[1] } else { vertical[0] };
    let horizontal = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(24), Constraint::Min(0)])
        .split(body);

    AppLayout {
        task_bar: show_task_bar.then_some(vertical[0]),
        sidebar: horizontal[0],
        content: horizontal[1],
        playbar: *vertical.last().unwrap(),
    }
}
```

Update `src/tui/run.rs` to consume a persistent aggregate websocket stream:

```rust
let settings = crate::core::config::settings::Settings::load().unwrap_or_default();
let renderer = crate::core::runtime_templates::RuntimeTemplateRenderer::default();
let api_client = crate::cli::client::ApiClient::new(base_url.clone());
let client = crate::tui::client::TuiClient::new(base_url);
let mut stream = client.connect().await?;
let mut app = crate::tui::app::App::new_for_test();
app.apply_tui_snapshot(stream.next_json::<crate::core::model::tui::TuiSnapshot>().await?);
app.footer_hints_enabled = context.footer_hints_enabled;
app.startup_notice = context.startup_notice;
if let Some(source_label) = context.source_label {
    app.set_source_label(source_label);
}

let (snapshot_tx, mut snapshot_rx) = tokio::sync::mpsc::unbounded_channel();
tokio::spawn(async move {
    while let Ok(snapshot) = stream.next_json::<crate::core::model::tui::TuiSnapshot>().await {
        if snapshot_tx.send(snapshot).is_err() {
            break;
        }
    }
});

loop {
    while let Ok(snapshot) = snapshot_rx.try_recv() {
        app.apply_tui_snapshot(snapshot);
    }

    terminal.draw(|frame| {
        let layout = app.layout(frame.area());
        if let Some(task_area) = layout.task_bar {
            if let Some(text) = app.task_bar_text(&renderer, &settings, task_area.width as usize) {
                frame.render_widget(Paragraph::new(text), task_area);
            }
        }
        let queue_lines = app.render_queue_lines().join("\n");
        frame.render_widget(
            Paragraph::new("Songs")
                .block(Block::default().borders(Borders::ALL).title("Views")),
            layout.sidebar,
        );
        frame.render_widget(
            Paragraph::new(queue_lines)
                .block(Block::default().borders(Borders::ALL).title("Queue")),
            layout.content,
        );
        frame.render_widget(
            Paragraph::new(format!(
                "{} | {}",
                crate::tui::ui::playbar::playback_label(&app.player),
                app.footer_status()
            ))
            .block(Block::default().borders(Borders::ALL).title("Status")),
            layout.playbar,
        );
    })?;

    if crossterm::event::poll(Duration::from_millis(50))? {
        if let Event::Key(key) = crossterm::event::read()? {
            if key.kind != KeyEventKind::Press {
                continue;
            }

            match app.handle_key(key) {
                Some(Action::TogglePlayback) => {
                    // action handlers keep using the latest player-only endpoints
                    let snapshot = api_client.post_json("/api/player/toggle").await?;
                    app.apply_snapshot(snapshot);
                }
                Some(Action::Next) => {
                    let snapshot = api_client.post_json("/api/player/next").await?;
                    app.apply_snapshot(snapshot);
                }
                Some(Action::Prev) => {
                    let snapshot = api_client.post_json("/api/player/prev").await?;
                    app.apply_snapshot(snapshot);
                }
                Some(Action::OpenHelp) => {}
                Some(Action::Quit) => break,
                _ => {}
            }
        }
    }
}
```

Update `App::layout` to call `split(area, self.active_task.is_some())`.

- [ ] **Step 4: Run tests to verify they pass**

Run:

```bash
rtk cargo test -q render_scan_cli_lines_uses_runtime_template_overrides --lib
rtk cargo test -q --test tui_app active_task_bar_renders_current_item_name
rtk cargo test -q --test tui_app active_task_bar_truncates_long_text_to_available_width
rtk cargo test -q top_task_bar_layout_only_reserves_space_when_needed --lib
```

Expected: all four commands PASS.

- [ ] **Step 5: Commit**

```bash
rtk git add src/cli/run.rs src/cli/run/tests.rs src/tui/app.rs src/tui/run.rs src/tui/ui/layout.rs src/tui/run/tests.rs tests/tui_app.rs
rtk git commit -m "feat(tui): show background scan task bar"
```

## Verification Checklist

- [ ] Run the focused regression suite:

```bash
rtk cargo test -q --test config_loading
rtk cargo test -q --test direct_open_background_scan
rtk cargo test -q --test api_server
rtk cargo test -q --test cli_remote
rtk cargo test -q --test tui_app
rtk cargo test -q --lib
```

Expected: all targeted suites PASS.

- [ ] Run the full repository verification required by the project:

```bash
rtk pnpm qa
```

Expected: exit code `0` with format, lint, and tests all green.

- [ ] Inspect the final diff for scope:

```bash
rtk git diff --stat
```

Expected: only the files listed in this plan changed; no unrelated reversions.

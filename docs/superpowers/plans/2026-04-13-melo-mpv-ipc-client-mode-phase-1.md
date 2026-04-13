# Melo `mpv-ipc` Client Mode Phase 1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 把当前 `mpv-ipc` 后端修成真正的 headless client mode，并让 TUI 在一期内闭环展示当前播放曲目、歌词和封面上下文，同时保证退出 TUI 默认停播但 daemon 保留。

**Architecture:** 本计划只覆盖已批准 spec 的一期范围，不接入 `libmpv`。实现分成五个切片：后端命名与配置预留、播放结束原因与 `mpv-ipc` 事件细分、服务层与 TUI 退出停播语义、TUI 聚合快照中的当前曲目详情、以及右侧详情区的播放高亮/歌词/封面渲染。每个切片都以 TDD 驱动，并留下独立可工作的提交。

**Tech Stack:** Rust 2024, Tokio, Ratatui, Crossterm, Reqwest, SeaORM, Lofty, mpv JSON IPC, viuer, pnpm, Vitest

---

## Scope Check

已批准 spec 明确包含两期：

- 一期：`mpv-ipc` headless client mode
- 二期：新增 `libmpv backend`

这两期不是同一个实现切片：一期主要修正产品语义和现有后端行为，二期才引入新的原生后端。为了保证每份计划都能独立交付并保持可验证，这里只写 **一期计划**。`libmpv` 二期应在一期合入并稳定后，另写一份独立计划。

## File Structure

### 配置与后端命名

- Modify: `src/core/config/settings.rs`
  - Responsibility: 让配置能表达 `mpv_ipc` / `mpv_lib` 两个后端名，并保留 `mpv` 到 `mpv_ipc` 的兼容别名。
- Modify: `config.example.toml`
  - Responsibility: 文档化新的后端命名和一期默认推荐用法。
- Modify: `tests/config_loading.rs`
  - Responsibility: 验证新后端命名、兼容别名和配置加载结果。
- Modify: `src/domain/player/factory.rs`
  - Responsibility: 解析 `mpv_ipc` / `mpv_lib` / `mpv`，并在一期对 `mpv_lib` 返回明确 unavailable 错误。
- Modify: `src/domain/player/factory/tests.rs`
  - Responsibility: 覆盖后端选择逻辑。

### 运行时 stop reason 与 `mpv-ipc`

- Modify: `src/domain/player/runtime.rs`
  - Responsibility: 定义结构化的 `PlaybackStopReason` 和新的运行时事件。
- Create: `src/domain/player/runtime/tests.rs`
  - Responsibility: 约束 stop reason 的语义工具函数与事件契约。
- Modify: `src/domain/player/mpv_backend.rs`
  - Responsibility: 默认强制 headless 运行参数，并把 `mpv end-file`/异常退出映射到 stop reason。
- Modify: `src/domain/player/mpv_backend/tests.rs`
  - Responsibility: 覆盖 headless 命令行和 `end-file` reason 解析。
- Modify: `src/domain/player/service.rs`
  - Responsibility: 根据 stop reason 决定“切歌 / 停止 / 进入错误态”。
- Modify: `src/domain/player/service/tests.rs`
  - Responsibility: 覆盖自然 EOF、主动 stop、手动关闭后端、后端异常退出的差异。

### 服务层与 TUI 退出语义

- Modify: `src/tui/run.rs`
  - Responsibility: 在 TUI 退出时显式停播，并把右侧详情区的加载逻辑与新的快照字段接起来。
- Modify: `src/tui/run/tests.rs`
  - Responsibility: 覆盖“退出 TUI 是否需要停播”的判定与曲目命中 helper。
- Modify: `tests/cli_remote.rs`
  - Responsibility: 验证新 helper 可见性仍正常，并保留 verbose/TUI 边界回归。

### 当前曲目详情快照

- Modify: `src/core/model/tui.rs`
  - Responsibility: 把当前曲目详情、歌词和封面引用变成正式 TUI 聚合快照字段。
- Modify: `src/domain/playlist/service.rs`
  - Responsibility: 暴露按歌曲 ID 获取歌词/封面所需的查询能力。
- Modify: `src/daemon/app.rs`
  - Responsibility: 在 `tui_snapshot()` 中注入当前播放曲目详情和当前播放歌曲 ID。
- Modify: `src/daemon/app/tests.rs`
  - Responsibility: 覆盖 `tui_snapshot()` 中的当前曲目详情聚合。
- Modify: `tests/tui_home.rs`
  - Responsibility: 覆盖当前播放来源与默认选中在新快照字段下仍然稳定。

### TUI 详情区与封面渲染

- Modify: `Cargo.toml`
  - Responsibility: 添加终端图片协议库 `viuer`，用于在支持终端里显示封面。
- Create: `src/tui/cover.rs`
  - Responsibility: 探测终端图片协议、执行封面绘制、无支持时生成降级文案。
- Create: `src/tui/cover/tests.rs`
  - Responsibility: 覆盖协议探测与降级逻辑。
- Modify: `src/tui/mod.rs`
  - Responsibility: 导出新的 `cover` 模块。
- Modify: `src/tui/app.rs`
  - Responsibility: 将预览歌曲从字符串升级为带 `song_id` 的结构，并保存当前曲目歌词/封面详情。
- Modify: `src/tui/app/tests.rs`
  - Responsibility: 覆盖当前播放歌曲高亮选择与歌词/封面数据落入 App 状态。
- Modify: `src/tui/ui/layout.rs`
  - Responsibility: 将右侧内容区细分为“曲目列表 + 详情区（歌词/封面）”矩形。
- Create: `src/tui/ui/details.rs`
  - Responsibility: 渲染歌词区、封面降级文案、当前曲目详情摘要。
- Modify: `src/tui/ui/playlist.rs`
  - Responsibility: 让预览列表具备 `is_selected` / `is_current_track` 双语义。
- Modify: `src/tui/ui/mod.rs`
  - Responsibility: 导出 `details` 模块。
- Modify: `src/tui/run.rs`
  - Responsibility: 使用新布局和 cover helper 渲染右侧详情区。
- Modify: `tests/tui_app.rs`
  - Responsibility: 覆盖当前播放曲目高亮、歌词占位、封面降级文案。

## Task 1: Reserve Backend Naming and Phase-1 Config Surface

**Files:**
- Modify: `src/core/config/settings.rs`
- Modify: `config.example.toml`
- Modify: `tests/config_loading.rs`
- Modify: `src/domain/player/factory.rs`
- Modify: `src/domain/player/factory/tests.rs`

- [ ] **Step 1: Write the failing config-loading and backend-choice tests**

Add this test to `tests/config_loading.rs`:

```rust
#[test]
fn settings_load_player_backend_variants_for_phase_one() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("config.toml");
    std::fs::write(
        &path,
        r#"
[player]
backend = "mpv_ipc"
"#,
    )
    .unwrap();

    let settings = melo::core::config::settings::Settings::load_from_path(&path).unwrap();
    assert_eq!(settings.player.backend, "mpv_ipc");
}
```

Replace `src/domain/player/factory/tests.rs` with:

```rust
use crate::core::config::settings::{MpvSettings, PlayerSettings};
use crate::domain::player::factory::{BackendChoice, resolve_backend_choice};

#[test]
fn auto_prefers_mpv_ipc_when_probe_succeeds() {
    let settings = PlayerSettings {
        backend: "auto".to_string(),
        mpv: MpvSettings {
            path: "mpv".to_string(),
            ipc_dir: "auto".to_string(),
            extra_args: Vec::new(),
        },
        ..PlayerSettings::default()
    };

    let choice = resolve_backend_choice(&settings, || true).unwrap();
    assert_eq!(choice, BackendChoice::MpvIpc);
}

#[test]
fn explicit_mpv_alias_maps_to_mpv_ipc() {
    let settings = PlayerSettings {
        backend: "mpv".to_string(),
        ..PlayerSettings::default()
    };

    let choice = resolve_backend_choice(&settings, || true).unwrap();
    assert_eq!(choice, BackendChoice::MpvIpc);
}

#[test]
fn explicit_mpv_lib_is_reserved_but_unavailable_in_phase_one() {
    let settings = PlayerSettings {
        backend: "mpv_lib".to_string(),
        ..PlayerSettings::default()
    };

    let err = resolve_backend_choice(&settings, || true).unwrap_err();
    assert!(err.to_string().contains("mpv_lib_backend_unavailable"));
}
```

- [ ] **Step 2: Run the tests to confirm they fail**

Run: `cargo test settings_load_player_backend_variants_for_phase_one explicit_mpv_ -- --nocapture`
Expected: FAIL because `BackendChoice::MpvIpc` and `mpv_lib_backend_unavailable` do not exist yet.

- [ ] **Step 3: Implement the phase-one backend naming surface**

Update `src/domain/player/factory.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendChoice {
    Rodio,
    MpvIpc,
}

pub fn resolve_backend_choice(
    settings: &PlayerSettings,
    mpv_available: impl Fn() -> bool,
) -> MeloResult<BackendChoice> {
    match settings.backend.as_str() {
        "rodio" => Ok(BackendChoice::Rodio),
        "mpv_lib" => Err(MeloError::Message("mpv_lib_backend_unavailable".to_string())),
        "mpv" | "mpv_ipc" => {
            if mpv_available() {
                Ok(BackendChoice::MpvIpc)
            } else {
                Err(MeloError::Message("mpv_backend_unavailable".to_string()))
            }
        }
        _ => {
            if mpv_available() {
                Ok(BackendChoice::MpvIpc)
            } else {
                Ok(BackendChoice::Rodio)
            }
        }
    }
}
```

Update `config.example.toml`:

```toml
[player]
# 可选值：`auto`、`rodio`、`mpv_ipc`。
# `mpv` 仍兼容映射到 `mpv_ipc`。
# `mpv_lib` 为二期保留名，一期不会真正启用。
backend = "auto"
```

- [ ] **Step 4: Run the focused tests to verify they pass**

Run: `cargo test settings_load_player_backend_variants_for_phase_one explicit_mpv_ -- --nocapture`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/core/config/settings.rs config.example.toml tests/config_loading.rs src/domain/player/factory.rs src/domain/player/factory/tests.rs
git commit -m "refactor(player): reserve mpv backend naming for phase one"
```

## Task 2: Add Structured Stop Reasons and Make `mpv-ipc` Truly Headless

**Files:**
- Modify: `src/domain/player/runtime.rs`
- Create: `src/domain/player/runtime/tests.rs`
- Modify: `src/domain/player/mpv_backend.rs`
- Modify: `src/domain/player/mpv_backend/tests.rs`
- Modify: `src/domain/player/service.rs`
- Modify: `src/domain/player/service/tests.rs`

- [ ] **Step 1: Write the failing stop-reason tests**

Replace `src/domain/player/mpv_backend/tests.rs` with:

```rust
use crate::domain::player::mpv_backend::{build_mpv_command, parse_mpv_event};
use crate::domain::player::runtime::{PlaybackRuntimeEvent, PlaybackStopReason};

#[test]
fn build_mpv_command_forces_headless_audio_client_mode() {
    let command = build_mpv_command(
        "C:/Tools/mpv.exe",
        "\\\\.\\pipe\\melo-mpv-test",
        &Vec::<String>::new(),
    );
    let args = command
        .get_args()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect::<Vec<_>>();

    assert!(args.iter().any(|arg| arg == "--idle=yes"));
    assert!(args.iter().any(|arg| arg == "--no-video"));
    assert!(args.iter().any(|arg| arg == "--force-window=no"));
}

#[test]
fn parse_end_file_event_distinguishes_eof_and_user_close() {
    assert_eq!(
        parse_mpv_event(r#"{"event":"end-file","reason":"eof"}"#, 7).unwrap(),
        Some(PlaybackRuntimeEvent::PlaybackStopped {
            generation: 7,
            reason: PlaybackStopReason::NaturalEof,
        })
    );
    assert_eq!(
        parse_mpv_event(r#"{"event":"end-file","reason":"quit"}"#, 7).unwrap(),
        Some(PlaybackRuntimeEvent::PlaybackStopped {
            generation: 7,
            reason: PlaybackStopReason::UserClosedBackend,
        })
    );
}
```

Add to `src/domain/player/service/tests.rs`:

```rust
#[tokio::test]
async fn runtime_user_closed_backend_stops_without_advancing() {
    let backend = Arc::new(FakeBackend::default());
    let runtime = backend.runtime_handle();
    let service = Arc::new(PlayerService::new(backend));
    service.start_runtime_event_loop();

    service.append(item(1, "One")).await.unwrap();
    service.append(item(2, "Two")).await.unwrap();
    service.play().await.unwrap();

    let _ = runtime.tx.send(PlaybackRuntimeEvent::PlaybackStopped {
        generation: 1,
        reason: crate::domain::player::runtime::PlaybackStopReason::UserClosedBackend,
    });
    tokio::task::yield_now().await;

    let snapshot = service.snapshot().await;
    assert_eq!(snapshot.playback_state, PlaybackState::Stopped.as_str());
    assert_eq!(snapshot.queue_index, Some(0));
    assert_eq!(snapshot.current_song.unwrap().title, "One");
}

#[tokio::test]
async fn runtime_backend_aborted_sets_error_without_auto_next() {
    let backend = Arc::new(FakeBackend::default());
    let runtime = backend.runtime_handle();
    let service = Arc::new(PlayerService::new(backend));
    service.start_runtime_event_loop();

    service.append(item(1, "One")).await.unwrap();
    service.append(item(2, "Two")).await.unwrap();
    service.play().await.unwrap();

    let _ = runtime.tx.send(PlaybackRuntimeEvent::PlaybackStopped {
        generation: 1,
        reason: crate::domain::player::runtime::PlaybackStopReason::BackendAborted,
    });
    tokio::task::yield_now().await;

    let snapshot = service.snapshot().await;
    assert_eq!(snapshot.playback_state, PlaybackState::Error.as_str());
    assert_eq!(snapshot.last_error.unwrap().code, "backend_aborted");
}
```

- [ ] **Step 2: Run the focused tests to confirm they fail**

Run: `cargo test build_mpv_command_forces_headless_audio_client_mode parse_end_file_event_distinguishes_eof_and_user_close runtime_user_closed_backend_stops_without_advancing runtime_backend_aborted_sets_error_without_auto_next --lib -- --nocapture`
Expected: FAIL because `PlaybackStopReason` and `PlaybackStopped` do not exist yet.

- [ ] **Step 3: Add the structured stop-reason contract**

Update `src/domain/player/runtime.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackStopReason {
    NaturalEof,
    UserStop,
    UserClosedBackend,
    BackendAborted,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlaybackRuntimeEvent {
    PlaybackStopped {
        generation: u64,
        reason: PlaybackStopReason,
    },
}

#[cfg(test)]
mod tests;
```

Update `src/domain/player/mpv_backend.rs`:

```rust
pub fn build_mpv_command(path: &str, ipc_path: &str, extra_args: &[String]) -> Command {
    let mut command = Command::new(path);
    command.arg("--idle=yes");
    command.arg("--no-terminal");
    command.arg("--force-window=no");
    command.arg("--no-video");
    command.arg(format!("--input-ipc-server={ipc_path}"));
    for arg in extra_args {
        command.arg(arg);
    }
    command
}

pub fn parse_mpv_event(line: &str, generation: u64) -> MeloResult<Option<PlaybackRuntimeEvent>> {
    let value: serde_json::Value =
        serde_json::from_str(line).map_err(|err| MeloError::Message(err.to_string()))?;
    if value.get("event").and_then(|event| event.as_str()) == Some("end-file") {
        let reason = match value.get("reason").and_then(|reason| reason.as_str()) {
            Some("eof") => PlaybackStopReason::NaturalEof,
            Some("stop") => PlaybackStopReason::UserStop,
            Some("quit") => PlaybackStopReason::UserClosedBackend,
            _ => PlaybackStopReason::BackendAborted,
        };
        return Ok(Some(PlaybackRuntimeEvent::PlaybackStopped { generation, reason }));
    }
    Ok(None)
}
```

Update the `handle_runtime_event` match in `src/domain/player/service.rs`:

```rust
async fn handle_runtime_event(&self, event: PlaybackRuntimeEvent) {
    match event {
        PlaybackRuntimeEvent::PlaybackStopped {
            generation,
            reason: PlaybackStopReason::NaturalEof,
        } => {
            // 保留现有“自然播完后切歌 / 停止”的逻辑
        }
        PlaybackRuntimeEvent::PlaybackStopped {
            generation,
            reason: PlaybackStopReason::UserStop | PlaybackStopReason::UserClosedBackend,
        } => {
            let mut session = self.session.lock().await;
            if generation != session.playback_generation {
                return;
            }
            session.playback_state = PlaybackState::Stopped;
            session.last_error = None;
            session.position_seconds = session.queue.current().map(|_| 0.0);
            let _ = self.publish_locked(&mut session);
        }
        PlaybackRuntimeEvent::PlaybackStopped {
            generation,
            reason: PlaybackStopReason::BackendAborted,
        } => {
            let mut session = self.session.lock().await;
            if generation != session.playback_generation {
                return;
            }
            let _ = self.fail_locked(
                &mut session,
                "backend_aborted",
                "backend aborted unexpectedly",
                MeloError::Message("backend aborted unexpectedly".to_string()),
            );
        }
    }
}
```

- [ ] **Step 4: Run the focused tests to verify they pass**

Run: `cargo test build_mpv_command_forces_headless_audio_client_mode parse_end_file_event_distinguishes_eof_and_user_close runtime_user_closed_backend_stops_without_advancing runtime_backend_aborted_sets_error_without_auto_next --lib -- --nocapture`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/domain/player/runtime.rs src/domain/player/runtime/tests.rs src/domain/player/mpv_backend.rs src/domain/player/mpv_backend/tests.rs src/domain/player/service.rs src/domain/player/service/tests.rs
git commit -m "feat(player): classify mpv stop reasons and headless mode"
```

## Task 3: Stop Playback Explicitly When the TUI Exits

**Files:**
- Modify: `src/tui/run.rs`
- Modify: `src/tui/run/tests.rs`
- Modify: `tests/cli_remote.rs`

- [ ] **Step 1: Write the failing quit-stop tests**

Add to `src/tui/run/tests.rs`:

```rust
#[test]
fn should_stop_on_tui_exit_for_active_sessions_only() {
    assert!(crate::tui::run::should_stop_on_tui_exit("playing"));
    assert!(crate::tui::run::should_stop_on_tui_exit("paused"));
    assert!(crate::tui::run::should_stop_on_tui_exit("error"));
    assert!(!crate::tui::run::should_stop_on_tui_exit("stopped"));
    assert!(!crate::tui::run::should_stop_on_tui_exit("idle"));
}
```

Add to `tests/cli_remote.rs`:

```rust
#[test]
fn launch_cwd_text_is_public_for_quit_boundary_regressions() {
    let text = melo::cli::run::launch_cwd_text(std::path::Path::new("D:/Music/Aimer"));
    assert_eq!(text, "D:/Music/Aimer");
}
```

- [ ] **Step 2: Run the focused tests to confirm they fail**

Run: `cargo test should_stop_on_tui_exit_for_active_sessions_only launch_cwd_text_is_public_for_quit_boundary_regressions -- --nocapture`
Expected: FAIL because `should_stop_on_tui_exit` does not exist yet.

- [ ] **Step 3: Implement explicit stop-on-exit**

Update `src/tui/run.rs`:

```rust
pub(crate) fn should_stop_on_tui_exit(playback_state: &str) -> bool {
    matches!(playback_state, "playing" | "paused" | "error")
}

async fn stop_playback_before_exit(
    app: &mut crate::tui::app::App,
    api_client: &crate::cli::client::ApiClient,
) -> MeloResult<()> {
    if should_stop_on_tui_exit(&app.player.playback_state) {
        app.apply_snapshot(api_client.post_json("/api/player/stop").await?);
    }
    Ok(())
}
```

Then update the `ActionId::Quit` branch:

```rust
crate::tui::event::ActionId::Quit => {
    if app.show_help {
        app.show_help = false;
        Ok(false)
    } else {
        stop_playback_before_exit(app, api_client).await?;
        Ok(true)
    }
}
```

- [ ] **Step 4: Run the focused tests to verify they pass**

Run: `cargo test should_stop_on_tui_exit_for_active_sessions_only launch_cwd_text_is_public_for_quit_boundary_regressions -- --nocapture`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/tui/run.rs src/tui/run/tests.rs tests/cli_remote.rs
git commit -m "feat(tui): stop playback when exiting tui"
```

## Task 4: Expose Current-Track Lyrics and Artwork in the TUI Snapshot

**Files:**
- Modify: `src/core/model/tui.rs`
- Modify: `src/domain/playlist/service.rs`
- Modify: `src/daemon/app.rs`
- Modify: `src/daemon/app/tests.rs`
- Modify: `tests/tui_home.rs`

- [ ] **Step 1: Write the failing snapshot-detail tests**

Add to `src/daemon/app/tests.rs`:

```rust
#[tokio::test]
async fn tui_snapshot_includes_current_track_detail_when_queue_has_song() {
    let state = crate::daemon::app::AppState::for_test().await;
    state
        .player
        .append(crate::core::model::player::QueueItem {
            song_id: 7,
            path: "tests/fixtures/full_test.mp3".into(),
            title: "Blue Bird".into(),
            duration_seconds: Some(212.0),
        })
        .await
        .unwrap();
    state.set_current_playlist_context("Favorites", "static");
    state.player.play().await.unwrap();

    let snapshot = state.tui_snapshot().await.unwrap();
    assert_eq!(snapshot.current_track.song_id, Some(7));
    assert_eq!(snapshot.current_track.title.as_deref(), Some("Blue Bird"));
}
```

Add to `tests/tui_home.rs`:

```rust
#[tokio::test(flavor = "multi_thread")]
async fn tui_home_snapshot_carries_lyrics_and_artwork_context_for_current_song() {
    let temp = tempfile::tempdir().unwrap();
    let audio = temp.path().join("01-first.flac");
    let cover = temp.path().join("cover.jpg");
    std::fs::write(&audio, b"audio").unwrap();
    std::fs::write(&cover, b"jpg").unwrap();
    std::fs::write(temp.path().join("01-first.lrc"), "[00:00.00]hello").unwrap();

    let mut settings = melo::core::config::settings::Settings::for_test(temp.path().join("melo.db"));
    settings.open.max_depth = 1;
    settings.open.prewarm_limit = 1;
    melo::core::db::bootstrap::DatabaseBootstrap::new(&settings).init().await.unwrap();

    let state = melo::daemon::app::AppState::for_test_with_settings(settings.clone()).await;
    state
        .open_target(melo::domain::open::service::OpenRequest {
            target: temp.path().to_string_lossy().to_string(),
            mode: "cwd_dir".to_string(),
        })
        .await
        .unwrap();

    let snapshot = state.tui_snapshot().await.unwrap();
    assert!(snapshot.current_track.lyrics.as_deref().unwrap().contains("hello"));
    assert!(snapshot.current_track.artwork.as_ref().unwrap().source_path.is_some());
}
```

- [ ] **Step 2: Run the focused tests to confirm they fail**

Run: `cargo test tui_snapshot_includes_current_track_detail_when_queue_has_song tui_home_snapshot_carries_lyrics_and_artwork_context_for_current_song -- --nocapture`
Expected: FAIL because `TuiSnapshot` has no `current_track` field yet.

- [ ] **Step 3: Add current-track detail to the TUI aggregation model**

Update `src/core/model/tui.rs`:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, ToSchema, Default)]
pub struct ArtworkRefSnapshot {
    pub source_kind: String,
    pub source_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, ToSchema, Default)]
pub struct CurrentTrackSnapshot {
    pub song_id: Option<i64>,
    pub title: Option<String>,
    pub lyrics: Option<String>,
    pub lyrics_source_kind: Option<String>,
    pub artwork: Option<ArtworkRefSnapshot>,
}

pub struct TuiSnapshot {
    pub player: PlayerSnapshot,
    pub active_task: Option<RuntimeTaskSnapshot>,
    pub playlist_browser: PlaylistBrowserSnapshot,
    pub current_track: CurrentTrackSnapshot,
}
```

Update `src/domain/playlist/service.rs`:

```rust
pub async fn song_record(&self, song_id: i64) -> MeloResult<Option<SongRecord>> {
    Ok(self
        .library_repository
        .list_songs()
        .await?
        .into_iter()
        .find(|song| song.id == song_id))
}

pub async fn artwork_for_song(&self, song_id: i64) -> MeloResult<Option<ArtworkRefRecord>> {
    self.library_repository.artwork_for_song(song_id).await
}
```

Update the body of `AppState::tui_snapshot()` in `src/daemon/app.rs`:

```rust
let current_track = if let Some(song) = player.current_song.as_ref() {
    let song_id = song.song_id;
    let song_record = self.playlists.song_record(song_id).await?;
    let artwork = self.playlists.artwork_for_song(song_id).await?;
    crate::core::model::tui::CurrentTrackSnapshot {
        song_id: Some(song_id),
        title: Some(song.title.clone()),
        lyrics: song_record.as_ref().and_then(|song| song.lyrics.clone()),
        lyrics_source_kind: song_record
            .as_ref()
            .map(|song| song.lyrics_source_kind.clone()),
        artwork: artwork.map(|artwork| crate::core::model::tui::ArtworkRefSnapshot {
            source_kind: artwork.source_kind,
            source_path: artwork.source_path,
        }),
    }
} else {
    crate::core::model::tui::CurrentTrackSnapshot::default()
};
```

and include it in the returned `TuiSnapshot`.

- [ ] **Step 4: Run the focused tests to verify they pass**

Run: `cargo test tui_snapshot_includes_current_track_detail_when_queue_has_song tui_home_snapshot_carries_lyrics_and_artwork_context_for_current_song -- --nocapture`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/core/model/tui.rs src/domain/playlist/service.rs src/daemon/app.rs src/daemon/app/tests.rs tests/tui_home.rs
git commit -m "feat(tui): expose current track lyrics and artwork context"
```

## Task 5: Render Current Song Highlight, Lyrics, and Cover Slot in the TUI

**Files:**
- Modify: `Cargo.toml`
- Create: `src/tui/cover.rs`
- Create: `src/tui/cover/tests.rs`
- Modify: `src/tui/mod.rs`
- Modify: `src/tui/app.rs`
- Modify: `src/tui/app/tests.rs`
- Modify: `src/tui/ui/layout.rs`
- Create: `src/tui/ui/details.rs`
- Modify: `src/tui/ui/playlist.rs`
- Modify: `src/tui/ui/mod.rs`
- Modify: `src/tui/run.rs`
- Modify: `tests/tui_app.rs`

- [ ] **Step 1: Write the failing UI-state and cover-capability tests**

Create `src/tui/cover.rs` with only `#[cfg(test)] mod tests;`, then create `src/tui/cover/tests.rs`:

```rust
#[test]
fn detect_cover_protocol_prefers_kitty_then_iterm_then_sixel() {
    let kitty = super::detect_cover_protocol_from_env(&[
        ("TERM".to_string(), "xterm-kitty".to_string()),
    ]);
    assert_eq!(kitty, super::CoverProtocol::Kitty);

    let iterm = super::detect_cover_protocol_from_env(&[
        ("TERM_PROGRAM".to_string(), "iTerm.app".to_string()),
    ]);
    assert_eq!(iterm, super::CoverProtocol::Iterm2);
}

#[test]
fn unsupported_terminal_returns_text_fallback() {
    let protocol = super::detect_cover_protocol_from_env(&[]);
    assert_eq!(protocol, super::CoverProtocol::Unsupported);
}
```

Add to `src/tui/app/tests.rs`:

```rust
#[test]
fn app_sets_preview_row_current_track_by_song_id() {
    let mut app = crate::tui::app::App::new_for_test();
    app.apply_tui_snapshot(crate::core::model::tui::TuiSnapshot {
        player: crate::core::model::player::PlayerSnapshot {
            current_song: Some(crate::core::model::player::NowPlayingSong {
                song_id: 2,
                title: "Aimer".into(),
                duration_seconds: Some(180.0),
            }),
            queue_index: Some(1),
            ..crate::core::model::player::PlayerSnapshot::default()
        },
        active_task: None,
        playlist_browser: browser_snapshot(),
        current_track: crate::core::model::tui::CurrentTrackSnapshot {
            song_id: Some(2),
            title: Some("Aimer".into()),
            lyrics: Some("[00:00.00]hello".into()),
            lyrics_source_kind: Some("sidecar".into()),
            artwork: None,
        },
    });
    app.set_playlist_preview(&crate::api::playlist::PlaylistPreviewResponse {
        name: "Favorites".into(),
        songs: vec![
            crate::api::playlist::PlaylistPreviewSong { id: 1, title: "One".into() },
            crate::api::playlist::PlaylistPreviewSong { id: 2, title: "Aimer".into() },
        ],
    });

    let rows = crate::tui::ui::playlist::preview_row_models(&app);
    assert!(!rows[0].is_current_track);
    assert!(rows[1].is_current_track);
}
```

Add to `tests/tui_app.rs`:

```rust
#[test]
fn detail_lines_show_lyrics_or_cover_fallback() {
    let mut app = melo::tui::app::App::new_for_test();
    app.current_track_lyrics = Some("[00:00.00]hello".to_string());
    app.current_track_cover_summary = Some("Cover unsupported in this terminal".to_string());

    let lines = melo::tui::ui::details::render_detail_lines(&app);
    assert!(lines.iter().any(|line| line.contains("hello")));
    assert!(lines.iter().any(|line| line.contains("unsupported")));
}
```

- [ ] **Step 2: Run the focused tests to confirm they fail**

Run: `cargo test detect_cover_protocol_prefers_kitty_then_iterm_then_sixel unsupported_terminal_returns_text_fallback app_sets_preview_row_current_track_by_song_id detail_lines_show_lyrics_or_cover_fallback -- --nocapture`
Expected: FAIL because `CoverProtocol`, `preview_row_models`, and `render_detail_lines` do not exist yet.

- [ ] **Step 3: Add cover capability detection and richer preview rows**

Update `Cargo.toml`:

```toml
viuer = "0.10.0"
```

Create `src/tui/cover.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoverProtocol {
    Kitty,
    Iterm2,
    Sixel,
    Unsupported,
}

pub fn detect_cover_protocol_from_env(env: &[(String, String)]) -> CoverProtocol {
    let lookup = |key: &str| env.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str());
    if lookup("TERM").is_some_and(|value| value.contains("kitty")) {
        return CoverProtocol::Kitty;
    }
    if lookup("TERM_PROGRAM") == Some("iTerm.app") {
        return CoverProtocol::Iterm2;
    }
    if lookup("TERM").is_some_and(|value| value.contains("sixel")) {
        return CoverProtocol::Sixel;
    }
    CoverProtocol::Unsupported
}

pub fn cover_fallback_summary(protocol: CoverProtocol, artwork_path: Option<&str>) -> String {
    match (protocol, artwork_path) {
        (CoverProtocol::Unsupported, Some(_)) => "Cover unsupported in this terminal".to_string(),
        (_, Some(path)) => format!("Cover: {path}"),
        (_, None) => "No cover available".to_string(),
    }
}

#[cfg(test)]
mod tests;
```

Update `src/tui/app.rs` state:

```rust
pub struct PreviewSongRow {
    pub song_id: i64,
    pub title: String,
}

pub current_track_song_id: Option<i64>,
pub current_track_lyrics: Option<String>,
pub current_track_cover_summary: Option<String>,
pub preview_songs: Vec<PreviewSongRow>,
```

Update `set_playlist_preview(...)`:

```rust
self.preview_songs = preview
    .songs
    .iter()
    .map(|song| PreviewSongRow {
        song_id: song.id,
        title: song.title.clone(),
    })
    .collect();
self.preview_titles = self.preview_songs.iter().map(|song| song.title.clone()).collect();
```

Add `preview_row_models` to `src/tui/ui/playlist.rs`:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreviewRowModel {
    pub text: String,
    pub is_selected: bool,
    pub is_current_track: bool,
}

pub fn preview_row_models(app: &crate::tui::app::App) -> Vec<PreviewRowModel> {
    app.preview_songs
        .iter()
        .enumerate()
        .map(|(index, song)| PreviewRowModel {
            text: song.title.clone(),
            is_selected: index == app.selected_preview_index(),
            is_current_track: app.current_track_song_id == Some(song.song_id),
        })
        .collect()
}
```

- [ ] **Step 4: Render lyrics and cover summary in a dedicated details pane**

Create `src/tui/ui/details.rs`:

```rust
pub fn render_detail_lines(app: &crate::tui::app::App) -> Vec<String> {
    let mut lines = Vec::new();

    if let Some(lyrics) = &app.current_track_lyrics {
        lines.extend(lyrics.lines().take(6).map(ToString::to_string));
    } else {
        lines.push("No lyrics available".to_string());
    }

    lines.push(String::new());
    lines.push(
        app.current_track_cover_summary
            .clone()
            .unwrap_or_else(|| "No cover available".to_string()),
    );

    lines
}
```

Update `src/tui/ui/layout.rs`:

```rust
pub struct AppLayout {
    pub task_bar: Option<Rect>,
    pub sidebar: Rect,
    pub content_header: Rect,
    pub content_tracks: Rect,
    pub content_detail: Rect,
    pub content_body: Rect,
    pub content: Rect,
    pub playbar: Rect,
}
```

and split the right body:

```rust
let lower = Layout::default()
    .direction(Direction::Horizontal)
    .constraints([Constraint::Percentage(58), Constraint::Percentage(42)])
    .split(right[1]);

AppLayout {
    task_bar: show_task_bar.then_some(vertical[0]),
    sidebar: horizontal[0],
    content_header: right[0],
    content_tracks: lower[0],
    content_detail: lower[1],
    content_body: right[1],
    content: right[1],
    playbar: *vertical.last().unwrap(),
}
```

Update `src/tui/run.rs` drawing code:

```rust
let detail_lines = crate::tui::ui::details::render_detail_lines(&app).join("\n");
let preview_lines = crate::tui::ui::playlist::render_preview_lines(&app).join("\n");

frame.render_widget(
    Paragraph::new(preview_lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("当前歌单")
                .border_style(preview_border_style),
        ),
    layout.content_tracks,
);
frame.render_widget(
    Paragraph::new(detail_lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("歌词 / 封面")
                .border_style(theme.pane_border),
        ),
    layout.content_detail,
);
```

When loading the initial TUI snapshot, write these fields into `App`:

```rust
app.current_track_song_id = home.current_track.song_id;
app.current_track_lyrics = home.current_track.lyrics.clone();
app.current_track_cover_summary = Some(crate::tui::cover::cover_fallback_summary(
    crate::tui::cover::detect_cover_protocol_from_env(
        &std::env::vars().collect::<Vec<_>>(),
    ),
    home.current_track
        .artwork
        .as_ref()
        .and_then(|artwork| artwork.source_path.as_deref()),
));
```

- [ ] **Step 5: Run focused verification and commit**

Run: `cargo test detect_cover_protocol_prefers_kitty_then_iterm_then_sixel unsupported_terminal_returns_text_fallback app_sets_preview_row_current_track_by_song_id detail_lines_show_lyrics_or_cover_fallback -- --nocapture`
Expected: PASS.

Run: `pnpm qa`
Expected: PASS.

Run:

```bash
git add Cargo.toml src/tui/cover.rs src/tui/cover/tests.rs src/tui/mod.rs src/tui/app.rs src/tui/app/tests.rs src/tui/ui/layout.rs src/tui/ui/details.rs src/tui/ui/playlist.rs src/tui/ui/mod.rs src/tui/run.rs tests/tui_app.rs
git commit -m "feat(tui): render current track lyrics and cover details"
```

## Final Verification

- [ ] **Step 1: Run the full verification suite**

Run: `pnpm qa`
Expected: PASS with TS checks, Rust format/lint/test all green.

- [ ] **Step 2: Run a manual smoke test**

Run the following in a terminal with a music directory:

```bash
cargo run -- "D:/Music/Aimer"
```

Expected:

- 不出现独立 `mpv` 窗口
- 右侧当前歌单能高亮当前播放曲目
- `q` 退出 TUI 后声音停止
- 再次启动 TUI 时 daemon 仍可复用

## Self-Review

### Spec coverage

- 默认不弹独立 `mpv` 窗口：Task 2
- 退出 TUI 默认停播但 daemon 保留：Task 3
- 手动关闭 `mpv` 解释为 `stop`：Task 2 + Task 3
- 当前播放曲目高亮：Task 4 + Task 5
- 歌词与封面回到 TUI 内部：Task 4 + Task 5
- 为 `libmpv` 二期预留命名与抽象空间：Task 1

### Placeholder scan

- 没有 `TODO` / `TBD` / “类似上一个任务”
- 每个测试步骤都有具体测试代码与命令
- 每个实现步骤都给出具体类型名、字段名或函数签名

### Type consistency

- 后端命名统一使用 `mpv_ipc` / `mpv_lib`
- stop reason 统一使用 `PlaybackStopReason`
- TUI 当前曲目详情统一使用 `CurrentTrackSnapshot`
- 预览歌曲行统一使用 `PreviewSongRow` / `PreviewRowModel`

### Follow-up note

- `libmpv` 二期不在本计划中实现
- 一期完成并稳定后，再单独写 `libmpv backend` 计划

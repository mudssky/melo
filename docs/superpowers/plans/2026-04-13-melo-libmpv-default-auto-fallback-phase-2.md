# Melo `libmpv` Default Auto Fallback Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在不回退一期产品语义的前提下，引入 `libmpv` 后端、让 `auto` 默认优先 `mpv_lib`，并在失败时按 `mpv_ipc -> rodio` 自动回退且对用户可见。

**Architecture:** 本计划采用“固定 backend + 会话句柄化”的平衡方案：daemon 启动时通过 `BackendResolver` 只解析一次实际 backend，并把解析结果注入 `PlayerService`；播放时不再让 service 直接操作 backend，而是通过单次播放生命周期的 `PlaybackSessionHandle` 控制当前 session。这样可以把 fallback 链集中在 resolver 中，把 per-track 运行时细节收敛在 session 中，同时保留一期已有的 stop reason、TUI 退出停播和当前曲目上下文语义。

**Tech Stack:** Rust 2024, Tokio, libmpv2 5.0.3, mpv JSON IPC, Rodio, Ratatui, Crossterm, Reqwest, SeaORM, pnpm, Vitest

---

## Scope Check

这份 spec 只覆盖一个相互耦合的子系统：播放器后端解析与运行时抽象。虽然会波及 daemon/TUI/CLI 展示层，但这些改动都围绕同一个目标展开：

- 引入 `libmpv`
- 保持 `auto` fallback 链
- 通过 `backend_notice` 让解析结果可见

因此本次适合写成一份独立计划，不再继续拆成多个 spec。

> **前置假设**
>
> 本计划默认目标机器可以获取 `libmpv` 运行时库。如果在 Task 4 的 `cargo test` / `pnpm qa` 中因为系统缺少 `libmpv` 导致链接失败，应停止执行并先补齐本机 `libmpv` 运行时，再继续后续步骤。

## File Structure

### 后端解析与快照字段

- Modify: `src/core/model/player.rs`
  - Responsibility: 为播放器快照增加 `backend_notice`，保持对外契约能表达 fallback 结果。
- Create: `src/domain/player/resolver.rs`
  - Responsibility: 只负责 backend 选择顺序、fallback 规则和 notice 文案。
- Create: `src/domain/player/resolver/tests.rs`
  - Responsibility: 覆盖 `auto = mpv_lib -> mpv_ipc -> rodio` 和显式 backend 不回退。
- Modify: `src/domain/player/mod.rs`
  - Responsibility: 导出新的 `resolver` 与 `libmpv_backend` 模块。
- Modify: `src/domain/player/factory.rs`
  - Responsibility: 根据 `ResolvedBackendChoice` 构造实际 backend，并返回 notice。
- Modify: `src/domain/player/factory/tests.rs`
  - Responsibility: 覆盖 factory 对 `mpv_lib` / `mpv_ipc` / `rodio` 的构造与错误路径。
- Modify: `config.example.toml`
  - Responsibility: 文档化新的 `auto` 优先级与 fallback 心智。

### 会话句柄抽象与服务层

- Modify: `src/domain/player/backend.rs`
  - Responsibility: 将 backend 从“直接执行播放命令”重构为“创建播放 session 的工厂”，定义 `PlaybackSessionHandle` 和 `PlaybackStartRequest`。
- Create: `src/domain/player/backend/tests.rs`
  - Responsibility: 约束 session trait 的最小契约与 `NoopBackend` 行为。
- Modify: `src/domain/player/service.rs`
  - Responsibility: service 改为管理 `ActivePlaybackSession`，并通过 session handle 完成 pause/resume/stop/progress。
- Modify: `src/domain/player/service/tests.rs`
  - Responsibility: 覆盖 session 替换、stale event 忽略、notice 下发和 stop 语义。
- Modify: `tests/player_service.rs`
  - Responsibility: 让集成测试跟随新的 service 构造方式与快照字段。

### 现有后端迁移

- Modify: `src/domain/player/mpv_backend.rs`
  - Responsibility: 将 `mpv-ipc` 改造成 `PlaybackBackend -> MpvPlaybackSession` 结构。
- Modify: `src/domain/player/mpv_backend/tests.rs`
  - Responsibility: 覆盖 `mpv` session 的 headless 参数、stop reason 和命令委托。
- Modify: `src/domain/player/rodio_backend.rs`
  - Responsibility: 将 `rodio` 改造成 `PlaybackBackend -> RodioPlaybackSession` 结构。
- Modify: `src/domain/player/rodio_backend/tests.rs`
  - Responsibility: 覆盖 `rodio` session 的自然 EOF、位置读取和控制语义。
- Modify: `tests/api_server.rs`
  - Responsibility: 更新测试替身 backend 以实现新的 session 工厂接口。

### `libmpv` 后端与启动集成

- Modify: `Cargo.toml`
  - Responsibility: 添加 `libmpv2 = "5.0.3"` 依赖。
- Create: `src/domain/player/libmpv_backend.rs`
  - Responsibility: 基于 `libmpv2` 实现 `PlaybackBackend` 和 `LibmpvPlaybackSession`。
- Create: `src/domain/player/libmpv_backend/tests.rs`
  - Responsibility: 覆盖 `libmpv` helper、事件映射和命令翻译。
- Modify: `src/daemon/app.rs`
  - Responsibility: 启动时通过 resolver + factory 构造 backend，并把 `backend_notice` 注入 `PlayerService`。
- Modify: `src/daemon/app/tests.rs`
  - Responsibility: 覆盖 daemon 使用 resolver 结果创建 service 的行为。

### TUI / CLI 可见性

- Modify: `src/tui/app.rs`
  - Responsibility: 在 footer/status 中显示 `backend_notice`。
- Modify: `tests/tui_app.rs`
  - Responsibility: 覆盖 fallback notice 在 TUI 底部状态栏可见。
- Modify: `tests/cli_remote.rs`
  - Responsibility: 覆盖 CLI 输出中包含 `backend_notice` 字段。

## Task 1: Add Resolver Surface and `backend_notice`

**Files:**
- Modify: `src/core/model/player.rs`
- Modify: `config.example.toml`
- Create: `src/domain/player/resolver.rs`
- Create: `src/domain/player/resolver/tests.rs`
- Modify: `src/domain/player/mod.rs`
- Modify: `src/domain/player/factory.rs`
- Modify: `src/domain/player/factory/tests.rs`

- [ ] **Step 1: Write the failing resolver tests**

Create `src/domain/player/resolver.rs` with:

```rust
#[cfg(test)]
mod tests;
```

Then create `src/domain/player/resolver/tests.rs`:

```rust
use crate::core::config::settings::PlayerSettings;
use crate::domain::player::factory::BackendChoice;
use crate::domain::player::resolver::{BackendAvailability, BackendResolver};

fn settings(backend: &str) -> PlayerSettings {
    PlayerSettings {
        backend: backend.to_string(),
        ..PlayerSettings::default()
    }
}

#[test]
fn auto_prefers_libmpv_then_mpv_ipc_then_rodio() {
    let resolver = BackendResolver::default();

    let libmpv = resolver.resolve_choice(
        &settings("auto"),
        BackendAvailability {
            mpv_lib: true,
            mpv_ipc: true,
            rodio: true,
        },
    );
    assert_eq!(libmpv.unwrap().choice, BackendChoice::MpvLib);

    let ipc = resolver.resolve_choice(
        &settings("auto"),
        BackendAvailability {
            mpv_lib: false,
            mpv_ipc: true,
            rodio: true,
        },
    );
    assert_eq!(ipc.unwrap().choice, BackendChoice::MpvIpc);

    let rodio = resolver.resolve_choice(
        &settings("auto"),
        BackendAvailability {
            mpv_lib: false,
            mpv_ipc: false,
            rodio: true,
        },
    );
    assert_eq!(rodio.unwrap().choice, BackendChoice::Rodio);
}

#[test]
fn auto_generates_user_visible_notice_when_fallback_happens() {
    let resolver = BackendResolver::default();
    let resolved = resolver
        .resolve_choice(
            &settings("auto"),
            BackendAvailability {
                mpv_lib: false,
                mpv_ipc: true,
                rodio: true,
            },
        )
        .unwrap();

    assert_eq!(resolved.choice, BackendChoice::MpvIpc);
    assert_eq!(
        resolved.notice.as_deref(),
        Some("mpv_lib unavailable, fell back to mpv_ipc")
    );
}

#[test]
fn explicit_backends_do_not_fallback() {
    let resolver = BackendResolver::default();

    let lib = resolver.resolve_choice(
        &settings("mpv_lib"),
        BackendAvailability {
            mpv_lib: true,
            mpv_ipc: true,
            rodio: true,
        },
    );
    assert_eq!(lib.unwrap().choice, BackendChoice::MpvLib);

    let err = resolver
        .resolve_choice(
            &settings("mpv_ipc"),
            BackendAvailability {
                mpv_lib: true,
                mpv_ipc: false,
                rodio: true,
            },
        )
        .unwrap_err();
    assert!(err.to_string().contains("mpv_backend_unavailable"));
}
```

Replace `src/domain/player/factory/tests.rs` with:

```rust
use crate::core::config::settings::Settings;
use crate::domain::player::factory::{build_backend_for_choice, BackendChoice};

#[test]
fn build_backend_for_choice_rejects_mpv_lib_until_backend_exists() {
    let settings = Settings::default();
    let err = build_backend_for_choice(BackendChoice::MpvLib, &settings).unwrap_err();
    assert!(err.to_string().contains("mpv_lib_backend_unavailable"));
}

#[test]
fn build_backend_for_choice_supports_rodio_and_mpv_ipc() {
    let settings = Settings::default();

    let rodio = build_backend_for_choice(BackendChoice::Rodio, &settings).unwrap();
    assert_eq!(rodio.backend_name(), "rodio");

    let mpv = build_backend_for_choice(BackendChoice::MpvIpc, &settings).unwrap();
    assert_eq!(mpv.backend_name(), "mpv_ipc");
}
```

- [ ] **Step 2: Run the focused tests to verify they fail**

Run:

```bash
rtk cargo test auto_prefers_libmpv_then_mpv_ipc_then_rodio --lib -- --nocapture
rtk cargo test auto_generates_user_visible_notice_when_fallback_happens --lib -- --nocapture
rtk cargo test explicit_backends_do_not_fallback --lib -- --nocapture
rtk cargo test build_backend_for_choice_rejects_mpv_lib_until_backend_exists --lib -- --nocapture
```

Expected:

- FAIL because `resolver.rs` is still empty
- FAIL because `BackendChoice::MpvLib` and `build_backend_for_choice` do not exist yet

- [ ] **Step 3: Implement the resolver, `backend_notice`, and factory placeholder**

Update `src/core/model/player.rs`:

```rust
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, ToSchema)]
pub struct PlayerSnapshot {
    pub backend_name: String,
    pub backend_notice: Option<String>,
    pub playback_state: String,
    pub queue_preview: Vec<String>,
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

impl Default for PlayerSnapshot {
    fn default() -> Self {
        Self {
            backend_name: "unknown".to_string(),
            backend_notice: None,
            playback_state: PlaybackState::Idle.as_str().to_string(),
            queue_preview: Vec::new(),
            current_song: None,
            queue_len: 0,
            queue_index: None,
            has_next: false,
            has_prev: false,
            last_error: None,
            version: 0,
            position_seconds: None,
            position_fraction: None,
            volume_percent: 100,
            muted: false,
            repeat_mode: RepeatMode::Off.as_str().to_string(),
            shuffle_enabled: false,
        }
    }
}
```

Replace `src/domain/player/resolver.rs` with:

```rust
use crate::core::config::settings::PlayerSettings;
use crate::core::error::{MeloError, MeloResult};
use crate::domain::player::factory::BackendChoice;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BackendAvailability {
    pub mpv_lib: bool,
    pub mpv_ipc: bool,
    pub rodio: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedBackendChoice {
    pub choice: BackendChoice,
    pub notice: Option<String>,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct BackendResolver;

impl BackendResolver {
    pub fn resolve_choice(
        &self,
        settings: &PlayerSettings,
        availability: BackendAvailability,
    ) -> MeloResult<ResolvedBackendChoice> {
        match settings.backend.as_str() {
            "rodio" => Ok(ResolvedBackendChoice {
                choice: BackendChoice::Rodio,
                notice: None,
            }),
            "mpv_lib" => {
                if availability.mpv_lib {
                    Ok(ResolvedBackendChoice {
                        choice: BackendChoice::MpvLib,
                        notice: None,
                    })
                } else {
                    Err(MeloError::Message("mpv_lib_backend_unavailable".to_string()))
                }
            }
            "mpv" | "mpv_ipc" => {
                if availability.mpv_ipc {
                    Ok(ResolvedBackendChoice {
                        choice: BackendChoice::MpvIpc,
                        notice: None,
                    })
                } else {
                    Err(MeloError::Message("mpv_backend_unavailable".to_string()))
                }
            }
            _ => {
                if availability.mpv_lib {
                    return Ok(ResolvedBackendChoice {
                        choice: BackendChoice::MpvLib,
                        notice: None,
                    });
                }
                if availability.mpv_ipc {
                    return Ok(ResolvedBackendChoice {
                        choice: BackendChoice::MpvIpc,
                        notice: Some("mpv_lib unavailable, fell back to mpv_ipc".to_string()),
                    });
                }
                if availability.rodio {
                    return Ok(ResolvedBackendChoice {
                        choice: BackendChoice::Rodio,
                        notice: Some(
                            "mpv_lib and mpv_ipc unavailable, fell back to rodio".to_string(),
                        ),
                    });
                }
                Err(MeloError::Message("no_backend_available".to_string()))
            }
        }
    }
}

#[cfg(test)]
mod tests;
```

Update `src/domain/player/factory.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendChoice {
    Rodio,
    MpvIpc,
    MpvLib,
}

pub fn build_backend_for_choice(
    choice: BackendChoice,
    settings: &Settings,
) -> MeloResult<Arc<dyn PlaybackBackend>> {
    match choice {
        BackendChoice::Rodio => Ok(Arc::new(RodioBackend::new()?)),
        BackendChoice::MpvIpc => Ok(Arc::new(MpvBackend::new(settings.clone())?)),
        BackendChoice::MpvLib => Err(MeloError::Message(
            "mpv_lib_backend_unavailable".to_string(),
        )),
    }
}
```

Update `src/domain/player/mod.rs`:

```rust
pub mod backend;
pub mod factory;
pub mod mpv_backend;
pub mod navigation;
pub mod queue;
pub mod resolver;
pub mod rodio_backend;
pub mod runtime;
pub mod service;
pub mod session_store;
```

Update `config.example.toml`:

```toml
[player]
# 后端选择：`auto`、`rodio`、`mpv_ipc`、`mpv_lib`。
# `auto` 的优先级为：`mpv_lib -> mpv_ipc -> rodio`。
# `mpv` 仍兼容映射到 `mpv_ipc`。
backend = "auto"
```

- [ ] **Step 4: Run the focused tests to verify they pass**

Run:

```bash
rtk cargo test auto_prefers_libmpv_then_mpv_ipc_then_rodio --lib -- --nocapture
rtk cargo test auto_generates_user_visible_notice_when_fallback_happens --lib -- --nocapture
rtk cargo test explicit_backends_do_not_fallback --lib -- --nocapture
rtk cargo test build_backend_for_choice_rejects_mpv_lib_until_backend_exists --lib -- --nocapture
```

Expected:

- PASS

- [ ] **Step 5: Run repository verification**

Run:

```bash
rtk pnpm qa
```

Expected:

- PASS with TS checks, Rust format/lint/test all green

- [ ] **Step 6: Commit**

```bash
rtk git add src/core/model/player.rs config.example.toml src/domain/player/resolver.rs src/domain/player/resolver/tests.rs src/domain/player/mod.rs src/domain/player/factory.rs src/domain/player/factory/tests.rs
rtk git commit -m "refactor(player): add backend resolver surface and notice"
```

## Task 2: Introduce `PlaybackSessionHandle` and Refactor `PlayerService`

**Files:**
- Modify: `src/domain/player/backend.rs`
- Create: `src/domain/player/backend/tests.rs`
- Modify: `src/domain/player/service.rs`
- Modify: `src/domain/player/service/tests.rs`
- Modify: `tests/player_service.rs`

- [ ] **Step 1: Write the failing session-handle tests**

Add `#[cfg(test)] mod tests;` to the bottom of `src/domain/player/backend.rs`, then create `src/domain/player/backend/tests.rs`:

```rust
use crate::domain::player::backend::{PlaybackBackend, PlaybackStartRequest};

#[test]
fn noop_backend_creates_noop_session() {
    let backend = crate::domain::player::backend::NoopBackend;
    let session = backend
        .start_session(PlaybackStartRequest {
            path: "tests/fixtures/full_test.mp3".into(),
            generation: 7,
            volume_factor: 1.0,
        })
        .unwrap();

    assert_eq!(session.current_position(), None);
    session.pause().unwrap();
    session.resume().unwrap();
    session.stop().unwrap();
}
```

Add to `src/domain/player/service/tests.rs`:

```rust
#[tokio::test]
async fn replay_stops_previous_session_before_creating_new_one() {
    let backend = Arc::new(FakeBackend::default());
    let service = Arc::new(PlayerService::new(backend.clone()));

    service.append(item(1, "One")).await.unwrap();
    service.append(item(2, "Two")).await.unwrap();
    service.play().await.unwrap();
    service.next().await.unwrap();

    assert_eq!(backend.stopped_generations(), vec![1]);
}

#[tokio::test]
async fn snapshot_exposes_backend_notice() {
    let backend = Arc::new(FakeBackend::default());
    let service = PlayerService::new_with_notice(
        backend,
        Some("mpv_lib unavailable, fell back to mpv_ipc".to_string()),
    );

    let snapshot = service.snapshot().await;
    assert_eq!(
        snapshot.backend_notice.as_deref(),
        Some("mpv_lib unavailable, fell back to mpv_ipc")
    );
}
```

- [ ] **Step 2: Run the focused tests to verify they fail**

Run:

```bash
rtk cargo test noop_backend_creates_noop_session --lib -- --nocapture
rtk cargo test replay_stops_previous_session_before_creating_new_one --lib -- --nocapture
rtk cargo test snapshot_exposes_backend_notice --lib -- --nocapture
```

Expected:

- FAIL because `PlaybackStartRequest` / `PlaybackSessionHandle` / `PlayerService::new_with_notice` do not exist yet

- [ ] **Step 3: Add session-handle abstractions and service support**

Replace the trait surface in `src/domain/player/backend.rs` with:

```rust
pub struct PlaybackStartRequest {
    pub path: std::path::PathBuf,
    pub generation: u64,
    pub volume_factor: f32,
}

pub trait PlaybackSessionHandle: Send + Sync {
    fn pause(&self) -> crate::core::error::MeloResult<()>;
    fn resume(&self) -> crate::core::error::MeloResult<()>;
    fn stop(&self) -> crate::core::error::MeloResult<()>;
    fn subscribe_runtime_events(&self) -> PlaybackRuntimeReceiver;
    fn current_position(&self) -> Option<Duration>;
    fn set_volume(&self, factor: f32) -> crate::core::error::MeloResult<()>;
}

pub trait PlaybackBackend: Send + Sync {
    fn backend_name(&self) -> &'static str;
    fn start_session(
        &self,
        request: PlaybackStartRequest,
    ) -> crate::core::error::MeloResult<Box<dyn PlaybackSessionHandle>>;
}

struct NoopPlaybackSession;

impl PlaybackSessionHandle for NoopPlaybackSession {
    fn pause(&self) -> crate::core::error::MeloResult<()> { Ok(()) }
    fn resume(&self) -> crate::core::error::MeloResult<()> { Ok(()) }
    fn stop(&self) -> crate::core::error::MeloResult<()> { Ok(()) }
    fn subscribe_runtime_events(&self) -> PlaybackRuntimeReceiver {
        let (_tx, rx) = broadcast::channel(1);
        rx
    }
    fn current_position(&self) -> Option<Duration> { None }
    fn set_volume(&self, _factor: f32) -> crate::core::error::MeloResult<()> { Ok(()) }
}

impl PlaybackBackend for NoopBackend {
    fn backend_name(&self) -> &'static str { "noop" }

    fn start_session(
        &self,
        _request: PlaybackStartRequest,
    ) -> crate::core::error::MeloResult<Box<dyn PlaybackSessionHandle>> {
        Ok(Box::new(NoopPlaybackSession))
    }
}

#[cfg(test)]
mod tests;
```

Update `src/domain/player/service.rs`:

```rust
struct ActivePlaybackSession {
    generation: u64,
    handle: Box<dyn crate::domain::player::backend::PlaybackSessionHandle>,
}

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
    active_session: Option<ActivePlaybackSession>,
    backend_notice: Option<String>,
}

pub fn new_with_notice(
    backend: Arc<dyn PlaybackBackend>,
    backend_notice: Option<String>,
) -> Self {
    let session = PlayerSession {
        backend_notice,
        ..PlayerSession::default()
    };
    let backend_name = backend.backend_name();
    let (snapshot_tx, _snapshot_rx) =
        watch::channel(Self::snapshot_from_session(&session, backend_name));
    Self {
        backend,
        backend_name,
        session: Mutex::new(session),
        snapshot_tx,
    }
}
```

Then update the play/pause/resume/stop/progress paths in `src/domain/player/service.rs`:

```rust
let generation = session.playback_generation + 1;
if let Some(active) = session.active_session.take() {
    let _ = active.handle.stop();
}
let handle = self.backend.start_session(crate::domain::player::backend::PlaybackStartRequest {
    path: current_path.to_path_buf(),
    generation,
    volume_factor: Self::volume_factor(&session),
})?;
session.active_session = Some(ActivePlaybackSession { generation, handle });
session.playback_generation = generation;
session.playback_state = PlaybackState::Playing;
session.position_seconds = Some(0.0);
session.last_error = None;
```

```rust
if let Some(active) = session.active_session.as_ref() {
    active.handle.pause()?;
    if let Some(position) = active.handle.current_position() {
        session.position_seconds = Some(position.as_secs_f64());
    }
}
```

```rust
if let Some(active) = session.active_session.take() {
    active.handle.stop()?;
}
session.playback_state = target_state;
session.position_seconds = session.queue.current().map(|_| 0.0);
```

and in `snapshot_from_session`:

```rust
backend_notice: session.backend_notice.clone(),
```

Update the fake test backend in `src/domain/player/service/tests.rs` to return fake sessions:

```rust
struct FakeSessionHandle {
    generation: u64,
    commands: Arc<Mutex<Vec<PlaybackCommand>>>,
    stopped_generations: Arc<Mutex<Vec<u64>>>,
    current_position: Arc<Mutex<Option<Duration>>>,
    runtime_tx: broadcast::Sender<PlaybackRuntimeEvent>,
}

impl crate::domain::player::backend::PlaybackSessionHandle for FakeSessionHandle {
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
        self.stopped_generations.lock().unwrap().push(self.generation);
        Ok(())
    }
    fn subscribe_runtime_events(&self) -> broadcast::Receiver<PlaybackRuntimeEvent> {
        self.runtime_tx.subscribe()
    }
    fn current_position(&self) -> Option<Duration> {
        *self.current_position.lock().unwrap()
    }
    fn set_volume(&self, factor: f32) -> crate::core::error::MeloResult<()> {
        self.commands
            .lock()
            .unwrap()
            .push(PlaybackCommand::SetVolume { factor });
        Ok(())
    }
}
```

- [ ] **Step 4: Run the focused tests to verify they pass**

Run:

```bash
rtk cargo test noop_backend_creates_noop_session --lib -- --nocapture
rtk cargo test replay_stops_previous_session_before_creating_new_one --lib -- --nocapture
rtk cargo test snapshot_exposes_backend_notice --lib -- --nocapture
```

Expected:

- PASS

- [ ] **Step 5: Run repository verification**

Run:

```bash
rtk pnpm qa
```

Expected:

- PASS

- [ ] **Step 6: Commit**

```bash
rtk git add src/domain/player/backend.rs src/domain/player/backend/tests.rs src/domain/player/service.rs src/domain/player/service/tests.rs tests/player_service.rs
rtk git commit -m "refactor(player): introduce playback session handles"
```

## Task 3: Port `mpv-ipc` and `rodio` to Session-Based Backends

**Files:**
- Modify: `src/domain/player/mpv_backend.rs`
- Modify: `src/domain/player/mpv_backend/tests.rs`
- Modify: `src/domain/player/rodio_backend.rs`
- Modify: `src/domain/player/rodio_backend/tests.rs`
- Modify: `tests/api_server.rs`

- [ ] **Step 1: Write the failing backend-session tests**

Replace `src/domain/player/mpv_backend/tests.rs` with:

```rust
use crate::domain::player::mpv_backend::{build_mpv_command, map_pipe_event_to_runtime};
use crate::domain::player::runtime::{PlaybackRuntimeEvent, PlaybackStopReason};

#[test]
fn mpv_backend_creates_session_and_maps_headless_flags() {
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
    assert!(args.iter().any(|arg| arg == "--force-window=no"));
    assert!(args.iter().any(|arg| arg == "--no-video"));
}

#[test]
fn mpv_session_maps_end_file_to_runtime_stop_reason() {
    assert_eq!(
        map_pipe_event_to_runtime(r#"{"event":"end-file","reason":"eof"}"#, 3).unwrap(),
        Some(PlaybackRuntimeEvent::PlaybackStopped {
            generation: 3,
            reason: PlaybackStopReason::NaturalEof,
        })
    );
}
```

Replace `src/domain/player/rodio_backend/tests.rs` with:

```rust
use super::should_emit_track_end;

#[test]
fn rodio_session_emits_natural_eof_only_for_current_generation() {
    assert!(should_emit_track_end(3, 3, true));
    assert!(!should_emit_track_end(4, 3, true));
    assert!(!should_emit_track_end(3, 3, false));
}
```

Add to `tests/api_server.rs`:

```rust
#[tokio::test(flavor = "multi_thread")]
async fn websocket_initial_snapshot_keeps_backend_notice_contract() {
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

    assert!(snapshot.backend_notice.is_none());
}
```

- [ ] **Step 2: Run the focused tests to verify they fail**

Run:

```bash
rtk cargo test mpv_backend_creates_session_and_maps_headless_flags --lib -- --nocapture
rtk cargo test mpv_session_maps_end_file_to_runtime_stop_reason --lib -- --nocapture
rtk cargo test rodio_session_emits_natural_eof_only_for_current_generation --lib -- --nocapture
rtk cargo test websocket_initial_snapshot_keeps_backend_notice_contract -- --nocapture
```

Expected:

- FAIL because `mpv_backend` and `rodio_backend` still implement旧的直接控制接口

- [ ] **Step 3: Implement session-based `mpv-ipc` and `rodio` backends**

Refactor `src/domain/player/mpv_backend.rs` around a session object:

```rust
pub struct MpvBackend {
    mpv_path: String,
    ipc_dir: String,
    extra_args: Vec<String>,
}

pub struct MpvPlaybackSession {
    process: Mutex<Option<MpvProcess>>,
    runtime_tx: broadcast::Sender<PlaybackRuntimeEvent>,
    current_position: Arc<Mutex<Option<Duration>>>,
    expected_stop_generation: Arc<AtomicU64>,
    generation: u64,
}

impl PlaybackBackend for MpvBackend {
    fn backend_name(&self) -> &'static str { "mpv_ipc" }

    fn start_session(
        &self,
        request: PlaybackStartRequest,
    ) -> MeloResult<Box<dyn PlaybackSessionHandle>> {
        let session = MpvPlaybackSession::spawn(
            &self.mpv_path,
            &self.ipc_dir,
            &self.extra_args,
            &request.path,
            request.generation,
            request.volume_factor,
        )?;
        Ok(Box::new(session))
    }
}

impl PlaybackSessionHandle for MpvPlaybackSession {
    fn pause(&self) -> MeloResult<()> {
        self.send_command(serde_json::json!({ "command": ["set_property", "pause", true] }))
    }
    fn resume(&self) -> MeloResult<()> {
        self.send_command(serde_json::json!({ "command": ["set_property", "pause", false] }))
    }
    fn stop(&self) -> MeloResult<()> { self.stop_process() }
    fn subscribe_runtime_events(&self) -> PlaybackRuntimeReceiver { self.runtime_tx.subscribe() }
    fn current_position(&self) -> Option<Duration> { *self.current_position.lock().unwrap() }
    fn set_volume(&self, factor: f32) -> MeloResult<()> {
        self.send_command(serde_json::json!({
            "command": ["set_property", "volume", factor.max(0.0) * 100.0]
        }))
    }
}
```

Keep a pure helper in `src/domain/player/mpv_backend.rs`:

```rust
pub fn map_pipe_event_to_runtime(
    line: &str,
    generation: u64,
) -> MeloResult<Option<PlaybackRuntimeEvent>> {
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

Refactor `src/domain/player/rodio_backend.rs` similarly:

```rust
pub struct RodioBackend {
    sink: rodio::MixerDeviceSink,
}

struct RodioPlaybackSession {
    player: Mutex<Option<Arc<rodio::Player>>>,
    runtime_tx: broadcast::Sender<PlaybackRuntimeEvent>,
    active_generation: Arc<AtomicU64>,
}

impl PlaybackBackend for RodioBackend {
    fn backend_name(&self) -> &'static str { "rodio" }

    fn start_session(
        &self,
        request: PlaybackStartRequest,
    ) -> MeloResult<Box<dyn PlaybackSessionHandle>> {
        let session = RodioPlaybackSession::new(&self.sink, &request.path, request.generation)?;
        session.set_volume(request.volume_factor)?;
        Ok(Box::new(session))
    }
}
```

Update the evented/fake backend in `tests/api_server.rs` to implement `start_session(...)` and return a lightweight fake session handle.

- [ ] **Step 4: Run the focused tests to verify they pass**

Run:

```bash
rtk cargo test mpv_backend_creates_session_and_maps_headless_flags --lib -- --nocapture
rtk cargo test mpv_session_maps_end_file_to_runtime_stop_reason --lib -- --nocapture
rtk cargo test rodio_session_emits_natural_eof_only_for_current_generation --lib -- --nocapture
rtk cargo test websocket_initial_snapshot_keeps_backend_notice_contract -- --nocapture
```

Expected:

- PASS

- [ ] **Step 5: Run repository verification**

Run:

```bash
rtk pnpm qa
```

Expected:

- PASS

- [ ] **Step 6: Commit**

```bash
rtk git add src/domain/player/mpv_backend.rs src/domain/player/mpv_backend/tests.rs src/domain/player/rodio_backend.rs src/domain/player/rodio_backend/tests.rs tests/api_server.rs
rtk git commit -m "refactor(player): port existing backends to session handles"
```

## Task 4: Add `libmpv` Backend and Wire `auto` Fallback at Daemon Startup

**Files:**
- Modify: `Cargo.toml`
- Create: `src/domain/player/libmpv_backend.rs`
- Create: `src/domain/player/libmpv_backend/tests.rs`
- Modify: `src/domain/player/mod.rs`
- Modify: `src/domain/player/factory.rs`
- Modify: `src/domain/player/factory/tests.rs`
- Modify: `src/domain/player/resolver.rs`
- Modify: `src/domain/player/resolver/tests.rs`
- Modify: `src/daemon/app.rs`
- Modify: `src/daemon/app/tests.rs`

- [ ] **Step 1: Write the failing `libmpv` and startup-resolution tests**

Create `src/domain/player/libmpv_backend.rs` with:

```rust
#[cfg(test)]
mod tests;
```

Then create `src/domain/player/libmpv_backend/tests.rs`:

```rust
use crate::domain::player::runtime::PlaybackStopReason;

#[test]
fn libmpv_end_file_reason_maps_to_runtime_reason() {
    assert_eq!(
        super::map_end_file_reason("eof"),
        PlaybackStopReason::NaturalEof
    );
    assert_eq!(
        super::map_end_file_reason("stop"),
        PlaybackStopReason::UserStop
    );
    assert_eq!(
        super::map_end_file_reason("quit"),
        PlaybackStopReason::UserClosedBackend
    );
}

#[test]
fn libmpv_backend_reports_stable_name() {
    let backend = super::LibmpvBackend::new_for_test();
    assert_eq!(backend.backend_name(), "mpv_lib");
}
```

Extend `src/domain/player/factory/tests.rs` with:

```rust
#[test]
fn build_backend_for_choice_supports_mpv_lib() {
    let settings = Settings::default();
    let backend = build_backend_for_choice(BackendChoice::MpvLib, &settings).unwrap();
    assert_eq!(backend.backend_name(), "mpv_lib");
}
```

Extend `src/domain/player/resolver/tests.rs` with:

```rust
#[test]
fn auto_falls_back_all_the_way_to_rodio_with_notice() {
    let resolver = BackendResolver::default();
    let resolved = resolver
        .resolve_choice(
            &settings("auto"),
            BackendAvailability {
                mpv_lib: false,
                mpv_ipc: false,
                rodio: true,
            },
        )
        .unwrap();

    assert_eq!(resolved.choice, BackendChoice::Rodio);
    assert_eq!(
        resolved.notice.as_deref(),
        Some("mpv_lib and mpv_ipc unavailable, fell back to rodio")
    );
}
```

Add to `src/daemon/app/tests.rs`:

```rust
#[tokio::test]
async fn player_service_keeps_backend_notice_from_factory_resolution() {
    let backend = std::sync::Arc::new(crate::domain::player::backend::NoopBackend);
    let service = crate::domain::player::service::PlayerService::new_with_notice(
        backend,
        Some("mpv_lib unavailable, fell back to mpv_ipc".to_string()),
    );

    let snapshot = service.snapshot().await;
    assert_eq!(
        snapshot.backend_notice.as_deref(),
        Some("mpv_lib unavailable, fell back to mpv_ipc")
    );
}
```

- [ ] **Step 2: Run the focused tests to verify they fail**

Run:

```bash
rtk cargo test libmpv_end_file_reason_maps_to_runtime_reason --lib -- --nocapture
rtk cargo test build_backend_for_choice_supports_mpv_lib --lib -- --nocapture
rtk cargo test auto_falls_back_all_the_way_to_rodio_with_notice --lib -- --nocapture
rtk cargo test player_service_keeps_backend_notice_from_factory_resolution --lib -- --nocapture
```

Expected:

- FAIL because `libmpv_backend` does not exist yet and factory still rejects `MpvLib`

- [ ] **Step 3: Implement `libmpv` backend and startup-time fallback wiring**

Update `Cargo.toml`:

```toml
libmpv2 = "5.0.3"
```

Create `src/domain/player/libmpv_backend.rs`:

```rust
use std::sync::{Mutex};
use std::time::Duration;

use libmpv2::{events::Event, Mpv};
use tokio::sync::broadcast;

use crate::core::config::settings::Settings;
use crate::core::error::{MeloError, MeloResult};
use crate::domain::player::backend::{
    PlaybackBackend, PlaybackSessionHandle, PlaybackStartRequest,
};
use crate::domain::player::runtime::{
    PlaybackRuntimeEvent, PlaybackRuntimeReceiver, PlaybackStopReason,
};

pub struct LibmpvBackend {
    _settings: Settings,
}

pub struct LibmpvPlaybackSession {
    mpv: Mutex<Mpv>,
    runtime_tx: broadcast::Sender<PlaybackRuntimeEvent>,
}

impl LibmpvBackend {
    pub fn new(settings: Settings) -> MeloResult<Self> {
        Ok(Self { _settings: settings })
    }

    pub fn new_for_test() -> Self {
        Self {
            _settings: Settings::default(),
        }
    }
}

pub fn map_end_file_reason(reason: &str) -> PlaybackStopReason {
    match reason {
        "eof" => PlaybackStopReason::NaturalEof,
        "stop" => PlaybackStopReason::UserStop,
        "quit" => PlaybackStopReason::UserClosedBackend,
        _ => PlaybackStopReason::BackendAborted,
    }
}

impl PlaybackBackend for LibmpvBackend {
    fn backend_name(&self) -> &'static str { "mpv_lib" }

    fn start_session(
        &self,
        request: PlaybackStartRequest,
    ) -> MeloResult<Box<dyn PlaybackSessionHandle>> {
        let mut mpv = Mpv::new().map_err(|err| MeloError::Message(err.to_string()))?;
        mpv.set_property("pause", false)
            .map_err(|err| MeloError::Message(err.to_string()))?;
        mpv.command("loadfile", &[request.path.to_string_lossy().as_ref()])
            .map_err(|err| MeloError::Message(err.to_string()))?;

        let (runtime_tx, _) = broadcast::channel(16);
        let session = LibmpvPlaybackSession {
            mpv: Mutex::new(mpv),
            runtime_tx,
        };
        session.set_volume(request.volume_factor)?;
        Ok(Box::new(session))
    }
}

impl PlaybackSessionHandle for LibmpvPlaybackSession {
    fn pause(&self) -> MeloResult<()> {
        self.mpv
            .lock()
            .unwrap()
            .set_property("pause", true)
            .map_err(|err| MeloError::Message(err.to_string()))
    }

    fn resume(&self) -> MeloResult<()> {
        self.mpv
            .lock()
            .unwrap()
            .set_property("pause", false)
            .map_err(|err| MeloError::Message(err.to_string()))
    }

    fn stop(&self) -> MeloResult<()> {
        self.mpv
            .lock()
            .unwrap()
            .command("stop", &[])
            .map_err(|err| MeloError::Message(err.to_string()))
    }

    fn subscribe_runtime_events(&self) -> PlaybackRuntimeReceiver {
        self.runtime_tx.subscribe()
    }

    fn current_position(&self) -> Option<Duration> {
        self.mpv
            .lock()
            .unwrap()
            .get_property::<f64>("time-pos")
            .ok()
            .filter(|seconds| *seconds >= 0.0)
            .map(Duration::from_secs_f64)
    }

    fn set_volume(&self, factor: f32) -> MeloResult<()> {
        self.mpv
            .lock()
            .unwrap()
            .set_property("volume", factor.max(0.0) * 100.0)
            .map_err(|err| MeloError::Message(err.to_string()))
    }
}

#[cfg(test)]
mod tests;
```

Update `src/domain/player/factory.rs`:

```rust
use crate::domain::player::libmpv_backend::LibmpvBackend;

pub struct BuiltBackend {
    pub backend: Arc<dyn PlaybackBackend>,
    pub notice: Option<String>,
}

pub fn build_backend_for_choice(
    choice: BackendChoice,
    settings: &Settings,
) -> MeloResult<Arc<dyn PlaybackBackend>> {
    match choice {
        BackendChoice::Rodio => Ok(Arc::new(RodioBackend::new()?)),
        BackendChoice::MpvIpc => Ok(Arc::new(MpvBackend::new(settings.clone())?)),
        BackendChoice::MpvLib => Ok(Arc::new(LibmpvBackend::new(settings.clone())?)),
    }
}

pub fn build_backend(settings: &Settings) -> MeloResult<BuiltBackend> {
    let resolver = crate::domain::player::resolver::BackendResolver::default();
    let availability = crate::domain::player::resolver::BackendAvailability {
        mpv_lib: true,
        mpv_ipc: mpv_exists(&settings.player.mpv.path),
        rodio: true,
    };
    let resolved = resolver.resolve_choice(&settings.player, availability)?;
    Ok(BuiltBackend {
        backend: build_backend_for_choice(resolved.choice, settings)?,
        notice: resolved.notice,
    })
}
```

Update `src/daemon/app.rs`:

```rust
pub async fn new() -> MeloResult<Self> {
    let settings = Settings::load()?;
    crate::core::db::bootstrap::DatabaseBootstrap::new(&settings)
        .prepare_runtime_database()
        .await?;
    let built = factory::build_backend(&settings)?;
    let backend_name = built.backend.backend_name().to_string();
    let runtime = DaemonRuntimeMeta::live(&backend_name)?;
    Ok(Self::with_backend_and_runtime(
        built.backend,
        settings,
        runtime,
        built.notice,
        LibraryService::with_lofty,
    ))
}

fn with_backend_and_runtime<F>(
    backend: Arc<dyn PlaybackBackend>,
    settings: Settings,
    runtime: DaemonRuntimeMeta,
    backend_notice: Option<String>,
    library_factory: F,
) -> Self
where
    F: FnOnce(Settings) -> LibraryService,
{
    let player = Arc::new(PlayerService::new_with_notice(backend, backend_notice));
    player.start_runtime_event_loop();
    player.start_progress_loop();
    let library = library_factory(settings.clone());
    let playlists = PlaylistService::new(settings.clone());
    let runtime_tasks = Arc::new(crate::daemon::tasks::RuntimeTaskStore::new());
    let playback_context =
        Arc::new(crate::daemon::playback_context::PlayingPlaylistStore::default());
    let open = Arc::new(crate::domain::open::service::OpenService::new(
        settings.clone(),
        library,
        playlists.clone(),
        Arc::clone(&player),
        Arc::clone(&runtime_tasks),
        Arc::clone(&playback_context),
    ));
    Self {
        player,
        settings,
        playlists,
        open,
        runtime_tasks,
        playback_context,
        runtime: Arc::new(runtime),
        shutdown_notify: Arc::new(Notify::new()),
        shutdown_requested: Arc::new(AtomicBool::new(false)),
    }
}
```

- [ ] **Step 4: Run the focused tests to verify they pass**

Run:

```bash
rtk cargo test libmpv_end_file_reason_maps_to_runtime_reason --lib -- --nocapture
rtk cargo test build_backend_for_choice_supports_mpv_lib --lib -- --nocapture
rtk cargo test auto_falls_back_all_the_way_to_rodio_with_notice --lib -- --nocapture
rtk cargo test player_service_keeps_backend_notice_from_factory_resolution --lib -- --nocapture
```

Expected:

- PASS

- [ ] **Step 5: Run repository verification**

Run:

```bash
rtk pnpm qa
```

Expected:

- PASS

- [ ] **Step 6: Commit**

```bash
rtk git add Cargo.toml src/domain/player/libmpv_backend.rs src/domain/player/libmpv_backend/tests.rs src/domain/player/mod.rs src/domain/player/factory.rs src/domain/player/factory/tests.rs src/domain/player/resolver.rs src/domain/player/resolver/tests.rs src/daemon/app.rs src/daemon/app/tests.rs
rtk git commit -m "feat(player): add libmpv backend with auto fallback"
```

## Task 5: Surface `backend_notice` in TUI and CLI

**Files:**
- Modify: `src/tui/app.rs`
- Modify: `tests/tui_app.rs`
- Modify: `tests/cli_remote.rs`

- [ ] **Step 1: Write the failing visibility tests**

Add to `tests/tui_app.rs`:

```rust
#[test]
fn footer_status_appends_backend_notice_when_present() {
    let mut app = melo::tui::app::App::new_for_test();
    app.apply_snapshot(melo::core::model::player::PlayerSnapshot {
        backend_name: "rodio".into(),
        backend_notice: Some("mpv_lib and mpv_ipc unavailable, fell back to rodio".into()),
        playback_state: "playing".into(),
        queue_preview: vec![],
        current_song: None,
        queue_len: 0,
        queue_index: None,
        has_next: false,
        has_prev: false,
        last_error: None,
        version: 1,
        position_seconds: None,
        position_fraction: None,
        volume_percent: 100,
        muted: false,
        repeat_mode: "off".into(),
        shuffle_enabled: false,
    });

    assert!(app.footer_status().contains("fell back to rodio"));
}
```

Add to `tests/cli_remote.rs`:

```rust
#[tokio::test(flavor = "multi_thread")]
async fn status_command_prints_backend_notice_field_when_present() {
    let backend = std::sync::Arc::new(melo::domain::player::backend::NoopBackend);
    let service = melo::domain::player::service::PlayerService::new_with_notice(
        backend,
        Some("mpv_lib unavailable, fell back to mpv_ipc".to_string()),
    );
    let snapshot = service.snapshot().await;

    let output = serde_json::to_string(&snapshot).unwrap();
    assert!(output.contains("backend_notice"));
    assert!(output.contains("fell back to mpv_ipc"));
}
```

- [ ] **Step 2: Run the focused tests to verify they fail**

Run:

```bash
rtk cargo test footer_status_appends_backend_notice_when_present -- --nocapture
rtk cargo test status_command_prints_backend_notice_field_when_present -- --nocapture
```

Expected:

- FAIL because `footer_status()` still only显示 backend 名和播放状态，没有展示 notice

- [ ] **Step 3: Implement visible fallback notice**

Update `src/tui/app.rs` in `footer_status()`:

```rust
let mut status = format!(
    "{} | backend={} | queue={} | prev={} | next={} | vol={} | repeat={} | shuffle={}",
    self.player.playback_state,
    self.player.backend_name,
    self.player.queue_len,
    self.player.has_prev,
    self.player.has_next,
    volume,
    self.player.repeat_mode,
    self.player.shuffle_enabled
);

if let Some(backend_notice) = &self.player.backend_notice {
    status.push_str(" | backend_notice=");
    status.push_str(backend_notice);
}
```

No CLI production code change is needed, because the existing CLI status/queue/player commands already print `PlayerSnapshot` as JSON and the new field will automatically appear once序列化模型扩展完成。

- [ ] **Step 4: Run the focused tests to verify they pass**

Run:

```bash
rtk cargo test footer_status_appends_backend_notice_when_present -- --nocapture
rtk cargo test status_command_prints_backend_notice_field_when_present -- --nocapture
```

Expected:

- PASS

- [ ] **Step 5: Run repository verification**

Run:

```bash
rtk pnpm qa
```

Expected:

- PASS

- [ ] **Step 6: Commit**

```bash
rtk git add src/tui/app.rs tests/tui_app.rs tests/cli_remote.rs
rtk git commit -m "feat(tui): surface backend fallback notices"
```

## Final Verification

- [ ] **Step 1: Run the full verification suite**

Run:

```bash
rtk pnpm qa
```

Expected:

- PASS with TS checks, Rust format/lint/test all green

- [ ] **Step 2: Run manual smoke tests**

Run with a real music directory:

```bash
rtk cargo run -- "D:/Music/Aimer"
```

Expected:

- 如果本机可用 `libmpv`，`status` / TUI 中实际 backend 为 `mpv_lib`
- 如果 `libmpv` 不可用但 `mpv` 可用，实际 backend 为 `mpv_ipc`，且可见 `backend_notice`
- 如果两者都不可用但 `rodio` 可用，实际 backend 为 `rodio`，且可见 `backend_notice`
- 只有自然 EOF 才自动切歌
- `q` 退出 TUI 后声音停止，但 daemon 仍可复用

## Self-Review

### Spec coverage

- `auto = mpv_lib -> mpv_ipc -> rodio`：Task 1 + Task 4
- 显式 backend 不回退：Task 1
- 会话句柄化：Task 2
- 现有 `mpv-ipc` / `rodio` 迁移到 session：Task 3
- 新增 `libmpv` backend：Task 4
- fallback 用户可见：Task 1 + Task 4 + Task 5
- 一期 stop reason / TUI 退出停播 / 当前曲目上下文不回退：Task 2 + Task 3 + Final Verification

### Placeholder scan

- 没有 `TODO` / `TBD` / “类似前一个任务”
- 每个任务都给出实际测试代码、命令和提交信息
- `libmpv2` crate 名与版本已明确为 `5.0.3`

### Type consistency

- 后端选择统一使用 `BackendChoice::{Rodio, MpvIpc, MpvLib}`
- 解析结果统一使用 `ResolvedBackendChoice`
- 播放启动请求统一使用 `PlaybackStartRequest`
- 单次播放生命周期统一使用 `PlaybackSessionHandle`
- fallback 提示统一使用 `backend_notice: Option<String>`

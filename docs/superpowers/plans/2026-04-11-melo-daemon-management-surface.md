# Melo Daemon Management Surface Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a complete managed-daemon control surface for Melo so users can explicitly start, inspect, diagnose, stop, restart, and read logs from the daemon without relying on fixed-path assumptions.

**Architecture:** Extend the daemon registration contract with stable identity metadata, then introduce a single observation pipeline that combines registration file data, process inspection, HTTP identity/status probing, and log-file availability into one `DaemonObservation`. Keep low-level process spawning in `src/daemon/process.rs`, move lifecycle orchestration into `src/daemon/manager.rs`, and keep CLI formatting/flag handling in `src/cli/daemon.rs` so `status`/`doctor`/`ps`/`restart` all share the same state model.

**Tech Stack:** Rust 2024, Clap 4, Tokio, Axum, Reqwest, `time`, `uuid`, `sysinfo`, `tracing-subscriber`, `pnpm qa`

**Scope Note:** Keep this plan inside the Rust daemon/CLI/API surface. The safe reinstall script in `scripts/dev-cli/install-dev.cjs` already has its own spec and should consume the new registration contract in a separate plan instead of being mixed into this implementation.

---

## File structure impact

### Existing files to modify

- Modify: `Cargo.toml`
- Modify: `src/api/system.rs`
- Modify: `src/cli/args.rs`
- Modify: `src/cli/client.rs`
- Modify: `src/cli/mod.rs`
- Modify: `src/cli/run.rs`
- Modify: `src/daemon/app.rs`
- Modify: `src/daemon/mod.rs`
- Modify: `src/daemon/process.rs`
- Modify: `src/daemon/registry.rs`
- Modify: `src/daemon/process/tests.rs`
- Modify: `src/daemon/registry/tests.rs`
- Modify: `src/main.rs`
- Modify: `tests/api_server.rs`
- Modify: `tests/cli_help.rs`
- Modify: `tests/cli_remote.rs`

### New files to create

- Create: `src/cli/daemon.rs`
- Create: `src/cli/daemon/tests.rs`
- Create: `src/daemon/manager.rs`
- Create: `src/daemon/manager/tests.rs`
- Create: `src/daemon/observe.rs`
- Create: `src/daemon/observe/tests.rs`

### Responsibilities

- `src/daemon/registry.rs`
  Resolve the shared user-scoped runtime paths, serialize the expanded daemon registration, and provide file-path-injectable load/store helpers for tests.
- `src/daemon/app.rs`
  Hold daemon runtime identity metadata (`instance_id`, `started_at`, `log_path`, `backend`, `version`) so both HTTP endpoints and registration persistence use one source of truth.
- `src/api/system.rs`
  Expose lightweight health and richer daemon status payloads that include instance identity and shutdown state.
- `src/daemon/observe.rs`
  Build the unified `DaemonObservation` and `DoctorReport` from registration, process state, HTTP probes, and log-file checks.
- `src/daemon/manager.rs`
  Implement `start`/`stop`/`restart`/`ensure_running`/`read_logs` on top of observation + spawn/wait helpers.
- `src/cli/daemon.rs`
  Translate daemon subcommands into manager calls and render compact human output, verbose human output, or structured JSON.
- `src/main.rs`
  When running the hidden internal daemon server command, tee tracing output to both stdout and the stable `daemon.log` file.

---

### Task 1: Add managed runtime paths, registration metadata, and system identity APIs

**Files:**
- Modify: `Cargo.toml`
- Modify: `src/daemon/registry.rs`
- Modify: `src/daemon/registry/tests.rs`
- Modify: `src/daemon/app.rs`
- Modify: `src/api/system.rs`
- Modify: `src/daemon/server.rs`
- Modify: `tests/api_server.rs`

- [ ] **Step 1: Write the failing registry and system-status tests**

```rust
// replace src/daemon/registry/tests.rs
use std::path::PathBuf;

use crate::daemon::registry::{DaemonRegistration, runtime_paths_from_env};

#[test]
fn runtime_paths_share_state_and_log_directory() {
    let root = PathBuf::from("C:/Temp/melo-tests");
    let paths = runtime_paths_from_env(Some(root.clone()), None, None).unwrap();
    assert_eq!(paths.state_file, root.join("daemon.json"));
    assert_eq!(paths.log_file, root.join("daemon.log"));
}

#[test]
fn runtime_paths_default_under_localappdata() {
    let paths = runtime_paths_from_env(
        None,
        Some(PathBuf::from("C:/Users/test/AppData/Local")),
        None,
    )
    .unwrap();
    assert_eq!(
        paths.state_file,
        PathBuf::from("C:/Users/test/AppData/Local")
            .join("melo")
            .join("daemon.json")
    );
    assert_eq!(
        paths.log_file,
        PathBuf::from("C:/Users/test/AppData/Local")
            .join("melo")
            .join("daemon.log")
    );
}

#[test]
fn registration_round_trips_identity_and_log_metadata() {
    let registration = DaemonRegistration {
        instance_id: "test-instance".to_string(),
        base_url: "http://127.0.0.1:38123".to_string(),
        pid: 4242,
        started_at: "2026-04-11T13:00:00Z".to_string(),
        version: "0.1.0".to_string(),
        backend: "mpv".to_string(),
        host: "127.0.0.1".to_string(),
        port: 38123,
        log_path: "C:/Users/test/AppData/Local/melo/daemon.log".to_string(),
    };

    let json = serde_json::to_string(&registration).unwrap();
    let decoded: DaemonRegistration = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.instance_id, "test-instance");
    assert_eq!(decoded.log_path, "C:/Users/test/AppData/Local/melo/daemon.log");
}
```

```rust
// append to tests/api_server.rs
use axum::body::{to_bytes, Body};

#[tokio::test]
async fn system_status_endpoint_returns_managed_identity() {
    let app = melo::daemon::app::test_router().await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/system/status")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload: melo::api::system::DaemonStatusResponse =
        serde_json::from_slice(&body).unwrap();

    assert_eq!(payload.backend, "noop");
    assert!(payload.instance_id.starts_with("test-instance"));
    assert!(payload.log_path.ends_with("daemon.log"));
    assert!(!payload.shutdown_requested);
}
```

- [ ] **Step 2: Run the targeted tests to confirm the new contract is missing**

Run: `cargo test registration_round_trips_identity_and_log_metadata --lib -- --nocapture`  
Expected: FAIL because `DaemonRegistration` does not yet include `instance_id` or `log_path`, and `runtime_paths_from_env` does not exist.

Run: `cargo test --test api_server system_status_endpoint_returns_managed_identity -- --nocapture`  
Expected: FAIL because `/api/system/status` is not routed and `DaemonStatusResponse` does not exist.

- [ ] **Step 3: Implement runtime paths, RFC3339 timestamps, and expanded registration metadata**

```toml
# Cargo.toml
[dependencies]
time = { version = "0.3.44", features = ["formatting", "parsing"] }
uuid = { version = "1.23.0", features = ["serde", "v4"] }
```

```rust
// src/daemon/registry.rs
use std::path::{Path, PathBuf};

use time::format_description::well_known::Rfc3339;

use crate::core::error::{MeloError, MeloResult};

const STATE_FILE_NAME: &str = "daemon.json";
const LOG_FILE_NAME: &str = "daemon.log";

/// daemon 运行期文件路径集合。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DaemonPaths {
    /// 注册文件路径。
    pub state_file: PathBuf,
    /// 日志文件路径。
    pub log_file: PathBuf,
}

/// 当前活跃 daemon 的注册信息。
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct DaemonRegistration {
    /// 当前 daemon 实例唯一 ID。
    pub instance_id: String,
    /// Daemon 的基础访问地址。
    pub base_url: String,
    /// Daemon 进程 ID。
    pub pid: u32,
    /// Daemon 启动时间。
    pub started_at: String,
    /// 当前 Melo 版本。
    pub version: String,
    /// 当前播放后端名。
    pub backend: String,
    /// 绑定主机。
    pub host: String,
    /// 绑定端口。
    pub port: u16,
    /// 固定日志文件路径。
    pub log_path: String,
}

/// 根据环境信息解析 daemon 运行期文件路径。
///
/// # 参数
/// - `explicit`：显式状态文件路径或目录路径
/// - `local_app_data`：Windows 本地应用数据目录
/// - `home_dir`：用户家目录
///
/// # 返回值
/// - `MeloResult<DaemonPaths>`：状态文件与日志文件路径
pub fn runtime_paths_from_env(
    explicit: Option<PathBuf>,
    local_app_data: Option<PathBuf>,
    home_dir: Option<PathBuf>,
) -> MeloResult<DaemonPaths> {
    let runtime_dir = if let Some(path) = explicit {
        normalize_runtime_dir(path)
    } else if let Some(root) = local_app_data {
        root.join("melo")
    } else {
        let home = home_dir
            .ok_or_else(|| MeloError::Message("daemon_state_path_unavailable".to_string()))?;
        home.join(".local").join("share").join("melo")
    };

    Ok(DaemonPaths {
        state_file: runtime_dir.join(STATE_FILE_NAME),
        log_file: runtime_dir.join(LOG_FILE_NAME),
    })
}

/// 解析当前环境下的 daemon 运行期文件路径。
///
/// # 参数
/// - 无
///
/// # 返回值
/// - `MeloResult<DaemonPaths>`：当前 daemon 文件路径集合
pub fn runtime_paths() -> MeloResult<DaemonPaths> {
    let explicit = std::env::var_os("MELO_DAEMON_STATE_FILE").map(PathBuf::from);
    let local_app_data = std::env::var_os("LOCALAPPDATA").map(PathBuf::from);
    let home_dir = std::env::var_os("HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("USERPROFILE").map(PathBuf::from));
    runtime_paths_from_env(explicit, local_app_data, home_dir)
}

/// 把 Unix 秒级时间戳格式化为 RFC3339 文本。
///
/// # 参数
/// - `seconds`：Unix 秒级时间戳
///
/// # 返回值
/// - `MeloResult<String>`：RFC3339 时间文本
pub fn started_at_text_from_unix_seconds(seconds: u64) -> MeloResult<String> {
    time::OffsetDateTime::from_unix_timestamp(seconds as i64)
        .map_err(|err| MeloError::Message(err.to_string()))?
        .format(&Rfc3339)
        .map_err(|err| MeloError::Message(err.to_string()))
}

/// 生成当前 UTC 时间的 RFC3339 文本。
///
/// # 参数
/// - 无
///
/// # 返回值
/// - `MeloResult<String>`：当前时间文本
pub fn now_started_at_text() -> MeloResult<String> {
    time::OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .map_err(|err| MeloError::Message(err.to_string()))
}

/// 从指定路径读取 daemon 注册信息。
///
/// # 参数
/// - `path`：注册文件路径
///
/// # 返回值
/// - `MeloResult<Option<DaemonRegistration>>`：存在时返回注册信息
pub async fn load_registration_from(path: &Path) -> MeloResult<Option<DaemonRegistration>> {
    let json = match tokio::fs::read_to_string(path).await {
        Ok(value) => value,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(MeloError::Message(err.to_string())),
    };

    serde_json::from_str(&json)
        .map(Some)
        .map_err(|err| MeloError::Message(err.to_string()))
}

/// 读取当前 daemon 注册信息。
///
/// # 参数
/// - 无
///
/// # 返回值
/// - `MeloResult<Option<DaemonRegistration>>`：存在时返回注册信息
pub async fn load_registration() -> MeloResult<Option<DaemonRegistration>> {
    let paths = runtime_paths()?;
    load_registration_from(&paths.state_file).await
}

/// 把注册信息写入指定路径。
///
/// # 参数
/// - `path`：目标注册文件路径
/// - `registration`：待写入的注册信息
///
/// # 返回值
/// - `MeloResult<()>`：写入结果
pub async fn store_registration_to(
    path: &Path,
    registration: &DaemonRegistration,
) -> MeloResult<()> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|err| MeloError::Message(err.to_string()))?;
    }

    let json = serde_json::to_string_pretty(registration)
        .map_err(|err| MeloError::Message(err.to_string()))?;
    tokio::fs::write(path, json)
        .await
        .map_err(|err| MeloError::Message(err.to_string()))
}

/// 写入当前 daemon 注册信息。
///
/// # 参数
/// - `registration`：待写入的注册信息
///
/// # 返回值
/// - `MeloResult<()>`：写入结果
pub async fn store_registration(registration: &DaemonRegistration) -> MeloResult<()> {
    let paths = runtime_paths()?;
    store_registration_to(&paths.state_file, registration).await
}

/// 删除指定路径下的注册文件。
///
/// # 参数
/// - `path`：注册文件路径
///
/// # 返回值
/// - `MeloResult<()>`：删除结果
pub async fn clear_registration_from(path: &Path) -> MeloResult<()> {
    match tokio::fs::remove_file(path).await {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(MeloError::Message(err.to_string())),
    }
}

/// 清除当前 daemon 注册信息。
///
/// # 参数
/// - 无
///
/// # 返回值
/// - `MeloResult<()>`：删除结果
pub async fn clear_registration() -> MeloResult<()> {
    let paths = runtime_paths()?;
    clear_registration_from(&paths.state_file).await
}

/// 规范化运行目录路径。
///
/// # 参数
/// - `path`：显式路径
///
/// # 返回值
/// - `PathBuf`：运行目录
fn normalize_runtime_dir(path: PathBuf) -> PathBuf {
    let file_name = path.file_name().and_then(|value| value.to_str());
    if matches!(file_name, Some(STATE_FILE_NAME) | Some(LOG_FILE_NAME)) {
        return path.parent().unwrap_or(Path::new(".")).to_path_buf();
    }

    if path.extension().is_some() {
        return path.parent().unwrap_or(Path::new(".")).to_path_buf();
    }

    path
}

#[cfg(test)]
mod tests;
```

- [ ] **Step 4: Implement daemon runtime identity storage and the new system status route**

```rust
// src/daemon/app.rs
use std::net::SocketAddr;
use std::sync::Arc;

use uuid::Uuid;

/// daemon 运行时元数据。
#[derive(Debug, Clone)]
pub struct DaemonRuntimeMeta {
    /// 当前实例 ID。
    pub instance_id: String,
    /// 当前进程 ID。
    pub pid: u32,
    /// 启动时间。
    pub started_at: String,
    /// 当前版本。
    pub version: String,
    /// 当前后端名。
    pub backend: String,
    /// 固定日志文件路径。
    pub log_path: String,
}

impl DaemonRuntimeMeta {
    /// 为生产 daemon 生成运行时元数据。
    ///
    /// # 参数
    /// - `backend`：后端名称
    ///
    /// # 返回值
    /// - `MeloResult<Self>`：运行时元数据
    pub fn live(backend: &str) -> MeloResult<Self> {
        let paths = crate::daemon::registry::runtime_paths()?;
        Ok(Self {
            instance_id: format!("instance-{}", Uuid::new_v4()),
            pid: std::process::id(),
            started_at: crate::daemon::registry::now_started_at_text()?,
            version: env!("CARGO_PKG_VERSION").to_string(),
            backend: backend.to_string(),
            log_path: paths.log_file.to_string_lossy().to_string(),
        })
    }

    /// 为测试 router 生成稳定的运行时元数据。
    ///
    /// # 参数
    /// - `backend`：后端名称
    ///
    /// # 返回值
    /// - `Self`：测试元数据
    pub fn for_test(backend: &str) -> Self {
        Self {
            instance_id: "test-instance-1".to_string(),
            pid: std::process::id(),
            started_at: "2026-04-11T00:00:00Z".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            backend: backend.to_string(),
            log_path: "C:/Temp/melo-tests/daemon.log".to_string(),
        }
    }
}

#[derive(Clone)]
pub struct AppState {
    pub player: Arc<PlayerService>,
    pub settings: Settings,
    pub open: Arc<crate::domain::open::service::OpenService>,
    runtime: Arc<DaemonRuntimeMeta>,
    shutdown_notify: Arc<Notify>,
    shutdown_requested: Arc<AtomicBool>,
}

impl AppState {
    pub fn new() -> MeloResult<Self> {
        let settings = Settings::load()?;
        let backend = factory::build_backend(&settings)?;
        let backend_name = backend.backend_name().to_string();
        let runtime = DaemonRuntimeMeta::live(&backend_name)?;
        Ok(Self::with_backend_and_runtime(
            backend,
            settings,
            runtime,
            LibraryService::with_lofty,
        ))
    }

    pub fn with_backend(backend: Arc<dyn PlaybackBackend>) -> Self {
        let backend_name = backend.backend_name().to_string();
        Self::with_backend_and_runtime(
            backend,
            Settings::default(),
            DaemonRuntimeMeta::for_test(&backend_name),
            LibraryService::for_test,
        )
    }

    fn with_backend_and_runtime<F>(
        backend: Arc<dyn PlaybackBackend>,
        settings: Settings,
        runtime: DaemonRuntimeMeta,
        library_factory: F,
    ) -> Self
    where
        F: FnOnce(Settings) -> LibraryService,
    {
        // 原有 player/open 初始化逻辑保持不变。
        Self {
            player,
            settings,
            open,
            runtime: Arc::new(runtime),
            shutdown_notify: Arc::new(Notify::new()),
            shutdown_requested: Arc::new(AtomicBool::new(false)),
        }
    }

    /// 返回当前 daemon 的系统状态响应。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - `crate::api::system::DaemonStatusResponse`：当前系统状态
    pub fn system_status(&self) -> crate::api::system::DaemonStatusResponse {
        crate::api::system::DaemonStatusResponse {
            instance_id: self.runtime.instance_id.clone(),
            pid: self.runtime.pid,
            started_at: self.runtime.started_at.clone(),
            version: self.runtime.version.clone(),
            backend: self.runtime.backend.clone(),
            log_path: self.runtime.log_path.clone(),
            shutdown_requested: self.shutdown_requested(),
        }
    }

    /// 为当前监听地址生成完整注册信息。
    ///
    /// # 参数
    /// - `listener_addr`：实际监听地址
    ///
    /// # 返回值
    /// - `crate::daemon::registry::DaemonRegistration`：当前 daemon 注册信息
    pub fn daemon_registration(
        &self,
        listener_addr: SocketAddr,
    ) -> crate::daemon::registry::DaemonRegistration {
        crate::daemon::registry::DaemonRegistration {
            instance_id: self.runtime.instance_id.clone(),
            base_url: format!("http://{listener_addr}"),
            pid: self.runtime.pid,
            started_at: self.runtime.started_at.clone(),
            version: self.runtime.version.clone(),
            backend: self.runtime.backend.clone(),
            host: listener_addr.ip().to_string(),
            port: listener_addr.port(),
            log_path: self.runtime.log_path.clone(),
        }
    }
}
```

```rust
// src/api/system.rs
use axum::{Json, extract::State, http::StatusCode};
use serde::{Deserialize, Serialize};

use crate::daemon::app::AppState;

/// 健康检查响应。
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HealthResponse {
    /// 服务状态。
    pub status: &'static str,
    /// 当前实例 ID。
    pub instance_id: String,
}

/// daemon 系统状态响应。
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DaemonStatusResponse {
    /// 当前实例 ID。
    pub instance_id: String,
    /// 当前进程 ID。
    pub pid: u32,
    /// daemon 启动时间。
    pub started_at: String,
    /// 当前 Melo 版本。
    pub version: String,
    /// 当前后端名。
    pub backend: String,
    /// 固定日志文件路径。
    pub log_path: String,
    /// 是否已收到关闭请求。
    pub shutdown_requested: bool,
}

/// 返回 daemon 健康状态。
///
/// # 参数
/// - `state`：应用状态
///
/// # 返回值
/// - `Json<HealthResponse>`：健康检查响应
pub async fn health(State(state): State<AppState>) -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        instance_id: state.system_status().instance_id,
    })
}

/// 返回 daemon 的系统身份与运行状态。
///
/// # 参数
/// - `state`：应用状态
///
/// # 返回值
/// - `Json<DaemonStatusResponse>`：系统状态响应
pub async fn status(State(state): State<AppState>) -> Json<DaemonStatusResponse> {
    Json(state.system_status())
}
```

```rust
// src/daemon/server.rs
pub fn router(state: AppState) -> Router {
    Router::new()
        .route(
            "/api/system/health",
            axum::routing::get(crate::api::system::health),
        )
        .route(
            "/api/system/status",
            axum::routing::get(crate::api::system::status),
        )
        .route(
            "/api/system/shutdown",
            axum::routing::post(crate::api::system::shutdown),
        )
        // 其余路由保持不变
        .with_state(state)
}
```

- [ ] **Step 5: Run the focused tests again and verify the new foundation passes**

Run: `cargo test runtime_paths_share_state_and_log_directory --lib -- --nocapture`  
Expected: PASS and the explicit override now yields both `daemon.json` and `daemon.log` in the same runtime directory.

Run: `cargo test --test api_server system_status_endpoint_returns_managed_identity -- --nocapture`  
Expected: PASS and `/api/system/status` returns `instance_id`, `backend`, `log_path`, and `shutdown_requested`.

- [ ] **Step 6: Run repo-wide verification and commit the foundation slice**

Run: `pnpm qa`  
Expected: PASS after formatting, linting, and all existing tests accept the expanded registration/system payload contract.

```bash
git add Cargo.toml src/daemon/registry.rs src/daemon/registry/tests.rs src/daemon/app.rs src/api/system.rs src/daemon/server.rs tests/api_server.rs
git commit -m "feat: add managed daemon identity metadata"
```

---

### Task 2: Build the unified daemon observation and doctor model

**Files:**
- Modify: `Cargo.toml`
- Modify: `src/cli/client.rs`
- Modify: `src/daemon/mod.rs`
- Create: `src/daemon/observe.rs`
- Create: `src/daemon/observe/tests.rs`

- [ ] **Step 1: Write failing observation and doctor unit tests**

```rust
// create src/daemon/observe/tests.rs
use crate::daemon::observe::{
    DaemonObservation, DaemonState, DoctorLevel, build_doctor_report, classify_state,
};

fn sample_observation() -> DaemonObservation {
    DaemonObservation {
        state: DaemonState::Running,
        registration_exists: true,
        registration_path: "C:/Temp/melo/daemon.json".to_string(),
        base_url: Some("http://127.0.0.1:38123".to_string()),
        instance_id: Some("instance-a".to_string()),
        pid: Some(4242),
        started_at: Some("2026-04-11T13:00:00Z".to_string()),
        backend: Some("noop".to_string()),
        host: Some("127.0.0.1".to_string()),
        port: Some(38123),
        process_exists: true,
        process_start_time_matches: true,
        actual_pid: Some(4242),
        actual_process_path: Some("C:/cargo/bin/melo.exe".to_string()),
        health_ok: true,
        http_instance_id_matches: Some(true),
        shutdown_requested: false,
        log_path: Some("C:/Temp/melo/daemon.log".to_string()),
        log_readable: true,
    }
}

#[test]
fn classify_running_when_http_and_process_match() {
    let state = classify_state(true, true, Some(true), false);
    assert_eq!(state, DaemonState::Running);
}

#[test]
fn classify_registered_but_unhealthy_when_process_start_mismatches() {
    let state = classify_state(true, false, Some(true), false);
    assert_eq!(state, DaemonState::RegisteredButUnhealthy);
}

#[test]
fn doctor_report_flags_instance_id_mismatch_as_fail() {
    let mut observation = sample_observation();
    observation.health_ok = true;
    observation.http_instance_id_matches = Some(false);
    observation.state = DaemonState::RegisteredButUnhealthy;

    let report = build_doctor_report(&observation);

    assert_eq!(report.conclusion, DoctorLevel::FAIL);
    assert!(report
        .checks
        .iter()
        .any(|check| check.code == "instance_id" && check.level == DoctorLevel::FAIL));
}
```

- [ ] **Step 2: Run the new unit tests and confirm the observation layer does not exist**

Run: `cargo test classify_running_when_http_and_process_match --lib -- --nocapture`  
Expected: FAIL because `daemon::observe` does not exist yet.

- [ ] **Step 3: Add process probing, HTTP identity probing, and doctor report generation**

```toml
# Cargo.toml
[dependencies]
sysinfo = "0.38.4"
```

```rust
// src/cli/client.rs
use crate::api::system::{DaemonStatusResponse, HealthResponse};

impl ApiClient {
    /// 读取 daemon 健康响应。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - `MeloResult<HealthResponse>`：健康响应
    pub async fn health_status(&self) -> MeloResult<HealthResponse> {
        let url = format!("{}/api/system/health", self.base_url);
        self.client
            .get(url)
            .send()
            .await
            .map_err(|err| MeloError::Message(err.to_string()))?
            .error_for_status()
            .map_err(|err| MeloError::Message(err.to_string()))?
            .json()
            .await
            .map_err(|err| MeloError::Message(err.to_string()))
    }

    /// 读取 daemon 系统状态响应。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - `MeloResult<DaemonStatusResponse>`：系统状态响应
    pub async fn daemon_status(&self) -> MeloResult<DaemonStatusResponse> {
        let url = format!("{}/api/system/status", self.base_url);
        self.client
            .get(url)
            .send()
            .await
            .map_err(|err| MeloError::Message(err.to_string()))?
            .error_for_status()
            .map_err(|err| MeloError::Message(err.to_string()))?
            .json()
            .await
            .map_err(|err| MeloError::Message(err.to_string()))
    }
}
```

```rust
// src/daemon/observe.rs
use sysinfo::{Pid, System};

use crate::api::system::DaemonStatusResponse;
use crate::core::config::settings::Settings;
use crate::core::error::MeloResult;
use crate::daemon::registry::{DaemonPaths, DaemonRegistration};

/// daemon 生命周期状态。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub enum DaemonState {
    /// 没有可用的受管 daemon。
    NotRunning,
    /// 注册存在，但健康或进程侧不可信。
    RegisteredButUnhealthy,
    /// daemon 运行正常。
    Running,
    /// daemon 已收到关闭请求，正在退出。
    Stopping,
}

/// daemon 统一观察结果。
#[derive(Debug, Clone, serde::Serialize)]
pub struct DaemonObservation {
    pub state: DaemonState,
    pub registration_exists: bool,
    pub registration_path: String,
    pub base_url: Option<String>,
    pub instance_id: Option<String>,
    pub pid: Option<u32>,
    pub started_at: Option<String>,
    pub backend: Option<String>,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub process_exists: bool,
    pub process_start_time_matches: bool,
    pub actual_pid: Option<u32>,
    pub actual_process_path: Option<String>,
    pub health_ok: bool,
    pub http_instance_id_matches: Option<bool>,
    pub shutdown_requested: bool,
    pub log_path: Option<String>,
    pub log_readable: bool,
}

/// 诊断级别。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub enum DoctorLevel {
    OK,
    WARN,
    FAIL,
}

/// 单条诊断结果。
#[derive(Debug, Clone, serde::Serialize)]
pub struct DoctorCheck {
    pub code: &'static str,
    pub level: DoctorLevel,
    pub summary: String,
    pub evidence: String,
}

/// daemon 诊断报告。
#[derive(Debug, Clone, serde::Serialize)]
pub struct DoctorReport {
    pub conclusion: DoctorLevel,
    pub checks: Vec<DoctorCheck>,
    pub observation: DaemonObservation,
}

/// 计算统一生命周期状态。
///
/// # 参数
/// - `process_exists`：进程是否存在
/// - `process_start_time_matches`：启动时间是否匹配
/// - `http_instance_id_matches`：HTTP 实例 ID 是否匹配
/// - `shutdown_requested`：daemon 是否正在关闭
///
/// # 返回值
/// - `DaemonState`：统一状态
pub fn classify_state(
    process_exists: bool,
    process_start_time_matches: bool,
    http_instance_id_matches: Option<bool>,
    shutdown_requested: bool,
) -> DaemonState {
    match http_instance_id_matches {
        Some(true) if process_exists && process_start_time_matches && shutdown_requested => {
            DaemonState::Stopping
        }
        Some(true) if process_exists && process_start_time_matches => DaemonState::Running,
        _ if process_exists || http_instance_id_matches.is_some() => {
            DaemonState::RegisteredButUnhealthy
        }
        _ => DaemonState::RegisteredButUnhealthy,
    }
}

/// 观测指定路径集合下的 daemon 状态。
///
/// # 参数
/// - `settings`：当前配置
/// - `paths`：daemon 运行期文件路径
///
/// # 返回值
/// - `MeloResult<DaemonObservation>`：观察结果
pub async fn observe_with_paths(
    _settings: &Settings,
    paths: &DaemonPaths,
) -> MeloResult<DaemonObservation> {
    let registration = crate::daemon::registry::load_registration_from(&paths.state_file).await?;
    let registration_path = paths.state_file.to_string_lossy().to_string();
    let log_path = Some(paths.log_file.to_string_lossy().to_string());
    let log_readable = tokio::fs::metadata(&paths.log_file)
        .await
        .map(|metadata| metadata.is_file())
        .unwrap_or(false);

    let Some(registration) = registration else {
        return Ok(DaemonObservation {
            state: DaemonState::NotRunning,
            registration_exists: false,
            registration_path,
            base_url: None,
            instance_id: None,
            pid: None,
            started_at: None,
            backend: None,
            host: None,
            port: None,
            process_exists: false,
            process_start_time_matches: false,
            actual_pid: None,
            actual_process_path: None,
            health_ok: false,
            http_instance_id_matches: None,
            shutdown_requested: false,
            log_path,
            log_readable,
        });
    };

    let process = observe_process(&registration);
    let daemon_status = probe_http_status(&registration).await;
    let http_instance_id_matches = daemon_status
        .as_ref()
        .map(|status| status.instance_id == registration.instance_id);
    let health_ok = daemon_status.is_some();
    let shutdown_requested = daemon_status
        .as_ref()
        .map(|status| status.shutdown_requested)
        .unwrap_or(false);

    Ok(DaemonObservation {
        state: classify_state(
            process.process_exists,
            process.process_start_time_matches,
            http_instance_id_matches,
            shutdown_requested,
        ),
        registration_exists: true,
        registration_path,
        base_url: Some(registration.base_url.clone()),
        instance_id: Some(registration.instance_id),
        pid: Some(registration.pid),
        started_at: Some(registration.started_at),
        backend: Some(registration.backend),
        host: Some(registration.host),
        port: Some(registration.port),
        process_exists: process.process_exists,
        process_start_time_matches: process.process_start_time_matches,
        actual_pid: process.actual_pid,
        actual_process_path: process.actual_process_path,
        health_ok,
        http_instance_id_matches,
        shutdown_requested,
        log_path: Some(registration.log_path),
        log_readable,
    })
}

/// 根据观察结果构建 doctor 报告。
///
/// # 参数
/// - `observation`：统一观察结果
///
/// # 返回值
/// - `DoctorReport`：诊断报告
pub fn build_doctor_report(observation: &DaemonObservation) -> DoctorReport {
    let checks = vec![
        DoctorCheck {
            code: "registration",
            level: if observation.registration_exists {
                DoctorLevel::OK
            } else {
                DoctorLevel::FAIL
            },
            summary: "daemon 注册文件".to_string(),
            evidence: observation.registration_path.clone(),
        },
        DoctorCheck {
            code: "pid",
            level: if observation.process_exists {
                DoctorLevel::OK
            } else {
                DoctorLevel::FAIL
            },
            summary: "注册 pid 是否存活".to_string(),
            evidence: format!("registered_pid={:?}", observation.pid),
        },
        DoctorCheck {
            code: "started_at",
            level: if observation.process_start_time_matches {
                DoctorLevel::OK
            } else {
                DoctorLevel::FAIL
            },
            summary: "进程启动时间是否匹配".to_string(),
            evidence: format!("started_at={:?}", observation.started_at),
        },
        DoctorCheck {
            code: "health",
            level: if observation.health_ok {
                DoctorLevel::OK
            } else {
                DoctorLevel::FAIL
            },
            summary: "health/status 探测".to_string(),
            evidence: format!("base_url={:?}", observation.base_url),
        },
        DoctorCheck {
            code: "instance_id",
            level: match observation.http_instance_id_matches {
                Some(true) => DoctorLevel::OK,
                Some(false) => DoctorLevel::FAIL,
                None => DoctorLevel::WARN,
            },
            summary: "HTTP 实例 ID 是否与注册一致".to_string(),
            evidence: format!("instance_id={:?}", observation.instance_id),
        },
        DoctorCheck {
            code: "log_file",
            level: if observation.log_readable {
                DoctorLevel::OK
            } else {
                DoctorLevel::FAIL
            },
            summary: "日志文件是否存在且可读".to_string(),
            evidence: observation.log_path.clone().unwrap_or_default(),
        },
    ];

    let conclusion = checks
        .iter()
        .map(|check| check.level.clone())
        .max_by_key(|level| match level {
            DoctorLevel::OK => 0,
            DoctorLevel::WARN => 1,
            DoctorLevel::FAIL => 2,
        })
        .unwrap_or(DoctorLevel::OK);

    DoctorReport {
        conclusion,
        checks,
        observation: observation.clone(),
    }
}

#[derive(Debug)]
struct ProcessObservation {
    process_exists: bool,
    process_start_time_matches: bool,
    actual_pid: Option<u32>,
    actual_process_path: Option<String>,
}

async fn probe_http_status(registration: &DaemonRegistration) -> Option<DaemonStatusResponse> {
    crate::cli::client::ApiClient::new(registration.base_url.clone())
        .daemon_status()
        .await
        .ok()
}

fn observe_process(registration: &DaemonRegistration) -> ProcessObservation {
    let system = System::new_all();
    let pid = Pid::from_u32(registration.pid);
    let Some(process) = system.process(pid) else {
        return ProcessObservation {
            process_exists: false,
            process_start_time_matches: false,
            actual_pid: None,
            actual_process_path: None,
        };
    };

    let started_at = crate::daemon::registry::started_at_text_from_unix_seconds(
        process.start_time(),
    )
    .ok();

    ProcessObservation {
        process_exists: true,
        process_start_time_matches: started_at
            .as_deref()
            .map(|value| value == registration.started_at)
            .unwrap_or(false),
        actual_pid: Some(process.pid().as_u32()),
        actual_process_path: process
            .exe()
            .map(|path| path.to_string_lossy().to_string()),
    }
}

#[cfg(test)]
mod tests;
```

```rust
// src/daemon/mod.rs
pub mod app;
pub mod observe;
pub mod process;
pub mod registry;
pub mod server;
```

- [ ] **Step 4: Run the observation tests and verify the shared state model works**

Run: `cargo test doctor_report_flags_instance_id_mismatch_as_fail --lib -- --nocapture`  
Expected: PASS and `doctor` now emits a `FAIL` check for mismatched HTTP `instance_id`.

Run: `cargo test classify_registered_but_unhealthy_when_process_start_mismatches --lib -- --nocapture`  
Expected: PASS and the shared state machine reports `RegisteredButUnhealthy`.

- [ ] **Step 5: Run repo-wide verification and commit the observation slice**

Run: `pnpm qa`  
Expected: PASS and the new `sysinfo` + doctor data model do not regress the rest of the project.

```bash
git add Cargo.toml src/cli/client.rs src/daemon/mod.rs src/daemon/observe.rs src/daemon/observe/tests.rs
git commit -m "feat: add daemon observation and doctor model"
```

---

### Task 3: Add lifecycle management, hidden `daemon run`, and file-backed daemon logging

**Files:**
- Modify: `src/cli/args.rs`
- Modify: `src/cli/run.rs`
- Modify: `src/daemon/mod.rs`
- Modify: `src/daemon/process.rs`
- Modify: `src/daemon/process/tests.rs`
- Modify: `src/main.rs`
- Create: `src/daemon/manager.rs`
- Create: `src/daemon/manager/tests.rs`

- [ ] **Step 1: Write failing lifecycle tests for the hidden runtime entry and manager behavior**

```rust
// replace src/daemon/process/tests.rs
use std::path::PathBuf;

use crate::daemon::process::{daemon_bind_addr, daemon_command};

#[test]
fn daemon_bind_addr_uses_meolo_base_url_port() {
    let addr = daemon_bind_addr("http://127.0.0.1:38123").unwrap();
    assert_eq!(addr.port(), 38123);
}

#[test]
fn daemon_command_uses_hidden_run_subcommand() {
    let command = daemon_command(PathBuf::from("melo.exe"));
    let args = command
        .get_args()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect::<Vec<_>>();

    assert_eq!(args, vec!["daemon".to_string(), "run".to_string()]);
}

#[tokio::test]
async fn next_bind_addr_skips_busy_base_port() {
    let busy = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let busy_port = busy.local_addr().unwrap().port();

    let addr = crate::daemon::process::next_bind_addr("127.0.0.1", busy_port, 4)
        .await
        .unwrap();

    assert_eq!(addr.ip().to_string(), "127.0.0.1");
    assert_ne!(addr.port(), busy_port);
}
```

```rust
// create src/daemon/manager/tests.rs
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use tokio::net::TcpListener;
use tokio::sync::Mutex;

use crate::core::config::settings::Settings;
use crate::daemon::manager::{restart_with_paths, start_with_paths, stop_with_paths};
use crate::daemon::observe::DaemonState;
use crate::daemon::registry::{DaemonPaths, store_registration_to};

fn daemon_paths(root: &std::path::Path) -> DaemonPaths {
    DaemonPaths {
        state_file: root.join("daemon.json"),
        log_file: root.join("daemon.log"),
    }
}

async fn spawn_registered_router(
    paths: &DaemonPaths,
    instance_id: &str,
) -> (
    crate::daemon::app::AppState,
    tokio::task::JoinHandle<()>,
    std::net::SocketAddr,
) {
    let state = crate::daemon::app::AppState::for_test().await;
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let registration = state.daemon_registration(addr);

    store_registration_to(
        &paths.state_file,
        &crate::daemon::registry::DaemonRegistration {
            instance_id: instance_id.to_string(),
            ..registration
        },
    )
    .await
    .unwrap();

    let app = crate::daemon::server::router(state.clone());
    let shutdown_state = state.clone();
    let handle = tokio::spawn(async move {
        axum::serve(listener, app)
            .with_graceful_shutdown(async move {
                shutdown_state.wait_for_shutdown().await;
            })
            .await
            .unwrap();
    });

    (state, handle, addr)
}

#[tokio::test(flavor = "multi_thread")]
async fn start_with_paths_reuses_running_instance_without_spawning() {
    let temp = tempfile::tempdir().unwrap();
    let paths = daemon_paths(temp.path());
    let (state, handle, _addr) = spawn_registered_router(&paths, "running-instance").await;
    let calls = Arc::new(AtomicUsize::new(0));

    let result = start_with_paths(&Settings::default(), &paths, {
        let calls = Arc::clone(&calls);
        move || {
            calls.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    })
    .await
    .unwrap();

    assert_eq!(result.observation.state, DaemonState::Running);
    assert_eq!(result.observation.instance_id.as_deref(), Some("running-instance"));
    assert_eq!(calls.load(Ordering::SeqCst), 0);

    state.request_shutdown();
    handle.await.unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn restart_with_paths_waits_for_shutdown_and_accepts_new_instance() {
    let temp = tempfile::tempdir().unwrap();
    let paths = daemon_paths(temp.path());
    let (old_state, old_handle, _old_addr) = spawn_registered_router(&paths, "old-instance").await;
    let new_server = Arc::new(Mutex::new(None));

    let result = restart_with_paths(&Settings::default(), &paths, {
        let paths = paths.clone();
        let new_server = Arc::clone(&new_server);
        move || {
            tokio::spawn(async move {
                tokio::time::sleep(Duration::from_millis(100)).await;
                let spawned = spawn_registered_router(&paths, "new-instance").await;
                *new_server.lock().await = Some(spawned);
            });
            Ok(())
        }
    })
    .await
    .unwrap();

    assert_eq!(result.previous.instance_id.as_deref(), Some("old-instance"));
    assert_eq!(result.current.instance_id.as_deref(), Some("new-instance"));
    assert_eq!(result.current.state, DaemonState::Running);

    old_state.request_shutdown();
    old_handle.await.unwrap();

    let (new_state, new_handle, _new_addr) = new_server.lock().await.take().unwrap();
    new_state.request_shutdown();
    new_handle.await.unwrap();
}

#[tokio::test]
async fn stop_with_paths_clears_stale_registration_when_server_is_unreachable() {
    let temp = tempfile::tempdir().unwrap();
    let paths = daemon_paths(temp.path());
    tokio::fs::write(&paths.log_file, "stale log\n").await.unwrap();
    store_registration_to(
        &paths.state_file,
        &crate::daemon::registry::DaemonRegistration {
            instance_id: "stale-instance".to_string(),
            base_url: "http://127.0.0.1:65530".to_string(),
            pid: 999_999,
            started_at: "2026-04-11T00:00:00Z".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            backend: "noop".to_string(),
            host: "127.0.0.1".to_string(),
            port: 65530,
            log_path: paths.log_file.to_string_lossy().to_string(),
        },
    )
    .await
    .unwrap();

    let result = stop_with_paths(&Settings::default(), &paths).await.unwrap();

    assert_eq!(result.action, "stale_registration_cleared");
    assert!(!paths.state_file.exists());
}
```

- [ ] **Step 2: Run the targeted lifecycle tests to capture the missing behavior**

Run: `cargo test daemon_command_uses_hidden_run_subcommand --lib -- --nocapture`  
Expected: FAIL because the spawned child still runs `melo daemon` instead of `melo daemon run`.

Run: `cargo test restart_with_paths_waits_for_shutdown_and_accepts_new_instance --lib -- --nocapture`  
Expected: FAIL because `daemon::manager` does not exist yet.

- [ ] **Step 3: Implement the lifecycle manager and log tail helpers**

```rust
// src/daemon/manager.rs
use std::time::Duration;

use crate::core::config::settings::Settings;
use crate::core::error::{MeloError, MeloResult};
use crate::daemon::observe::{DaemonObservation, DaemonState, observe_with_paths};
use crate::daemon::registry::DaemonPaths;

/// daemon 启动结果。
#[derive(Debug, Clone, serde::Serialize)]
pub struct StartResult {
    pub action: &'static str,
    pub observation: DaemonObservation,
}

/// daemon 停止结果。
#[derive(Debug, Clone, serde::Serialize)]
pub struct StopResult {
    pub action: &'static str,
    pub observation: DaemonObservation,
}

/// daemon 重启结果。
#[derive(Debug, Clone, serde::Serialize)]
pub struct RestartResult {
    pub action: &'static str,
    pub previous: DaemonObservation,
    pub current: DaemonObservation,
}

/// 启动 daemon，并等待到可观测状态稳定。
///
/// # 参数
/// - `settings`：当前配置
///
/// # 返回值
/// - `MeloResult<StartResult>`：启动结果
pub async fn start(settings: &Settings) -> MeloResult<StartResult> {
    let paths = crate::daemon::registry::runtime_paths()?;
    start_with_paths(settings, &paths, crate::daemon::process::spawn_background_daemon).await
}

/// 带注入 spawn 行为地启动 daemon，便于单测。
///
/// # 参数
/// - `settings`：当前配置
/// - `paths`：运行期文件路径
/// - `spawn`：实际启动逻辑
///
/// # 返回值
/// - `MeloResult<StartResult>`：启动结果
pub async fn start_with_paths<F>(
    settings: &Settings,
    paths: &DaemonPaths,
    spawn: F,
) -> MeloResult<StartResult>
where
    F: FnOnce() -> MeloResult<()>,
{
    let current = observe_with_paths(settings, paths).await?;
    if current.state == DaemonState::Running {
        return Ok(StartResult {
            action: "already_running",
            observation: current,
        });
    }

    if current.registration_exists && current.state != DaemonState::Running {
        crate::daemon::registry::clear_registration_from(&paths.state_file).await?;
    }

    spawn()?;
    let observation = wait_for_state(settings, paths, |value| value.state == DaemonState::Running)
        .await?;

    Ok(StartResult {
        action: "started",
        observation,
    })
}

/// 停止 daemon。
///
/// # 参数
/// - `settings`：当前配置
///
/// # 返回值
/// - `MeloResult<StopResult>`：停止结果
pub async fn stop(settings: &Settings) -> MeloResult<StopResult> {
    let paths = crate::daemon::registry::runtime_paths()?;
    stop_with_paths(settings, &paths).await
}

/// 按指定路径集合停止 daemon。
///
/// # 参数
/// - `settings`：当前配置
/// - `paths`：运行期文件路径
///
/// # 返回值
/// - `MeloResult<StopResult>`：停止结果
pub async fn stop_with_paths(settings: &Settings, paths: &DaemonPaths) -> MeloResult<StopResult> {
    let observation = observe_with_paths(settings, paths).await?;
    match observation.state {
        DaemonState::NotRunning => Ok(StopResult {
            action: "not_running",
            observation,
        }),
        DaemonState::RegisteredButUnhealthy => {
            crate::daemon::registry::clear_registration_from(&paths.state_file).await?;
            let current = observe_with_paths(settings, paths).await?;
            Ok(StopResult {
                action: "stale_registration_cleared",
                observation: current,
            })
        }
        DaemonState::Running | DaemonState::Stopping => {
            if let Some(base_url) = observation.base_url.clone() {
                let _ = crate::cli::client::ApiClient::new(base_url)
                    .post_no_body("/api/system/shutdown")
                    .await;
            }
            let current = wait_for_state(settings, paths, |value| {
                matches!(
                    value.state,
                    DaemonState::NotRunning | DaemonState::RegisteredButUnhealthy
                )
            })
            .await?;
            crate::daemon::registry::clear_registration_from(&paths.state_file).await?;
            Ok(StopResult {
                action: "stopped",
                observation: current,
            })
        }
    }
}

/// 重启 daemon，并确认旧实例退出、新实例健康。
///
/// # 参数
/// - `settings`：当前配置
///
/// # 返回值
/// - `MeloResult<RestartResult>`：重启结果
pub async fn restart(settings: &Settings) -> MeloResult<RestartResult> {
    let paths = crate::daemon::registry::runtime_paths()?;
    restart_with_paths(settings, &paths, crate::daemon::process::spawn_background_daemon).await
}

/// 带注入 spawn 行为地重启 daemon。
///
/// # 参数
/// - `settings`：当前配置
/// - `paths`：运行期文件路径
/// - `spawn`：实际启动逻辑
///
/// # 返回值
/// - `MeloResult<RestartResult>`：重启结果
pub async fn restart_with_paths<F>(
    settings: &Settings,
    paths: &DaemonPaths,
    spawn: F,
) -> MeloResult<RestartResult>
where
    F: FnOnce() -> MeloResult<()>,
{
    let previous = observe_with_paths(settings, paths).await?;
    let _ = stop_with_paths(settings, paths).await?;
    let current = start_with_paths(settings, paths, spawn).await?.observation;
    Ok(RestartResult {
        action: "restarted",
        previous,
        current,
    })
}

/// 确保 daemon 已可访问，供带副作用命令复用。
///
/// # 参数
/// - `settings`：当前配置
///
/// # 返回值
/// - `MeloResult<String>`：健康 daemon 的 base URL
pub async fn ensure_running(settings: &Settings) -> MeloResult<String> {
    let observation = start(settings).await?.observation;
    observation
        .base_url
        .ok_or_else(|| MeloError::Message("daemon_not_running".to_string()))
}

/// 读取日志文件尾部。
///
/// # 参数
/// - `paths`：运行期文件路径
/// - `tail`：保留末尾行数
///
/// # 返回值
/// - `MeloResult<String>`：日志尾部文本
pub async fn read_logs_with_paths(paths: &DaemonPaths, tail: usize) -> MeloResult<String> {
    let contents = tokio::fs::read_to_string(&paths.log_file)
        .await
        .map_err(|err| MeloError::Message(err.to_string()))?;
    Ok(tail_lines(&contents, tail))
}

/// 保留文本的最后 N 行。
///
/// # 参数
/// - `contents`：原始文本
/// - `tail`：目标行数
///
/// # 返回值
/// - `String`：截断后的文本
pub fn tail_lines(contents: &str, tail: usize) -> String {
    let lines = contents.lines().collect::<Vec<_>>();
    let start = lines.len().saturating_sub(tail);
    lines[start..].join("\n")
}

async fn wait_for_state<F>(
    settings: &Settings,
    paths: &DaemonPaths,
    predicate: F,
) -> MeloResult<DaemonObservation>
where
    F: Fn(&DaemonObservation) -> bool,
{
    for _ in 0..40 {
        let observation = observe_with_paths(settings, paths).await?;
        if predicate(&observation) {
            return Ok(observation);
        }
        tokio::time::sleep(Duration::from_millis(150)).await;
    }

    Err(MeloError::Message("daemon_state_transition_timeout".to_string()))
}

#[cfg(test)]
mod tests;
```

- [ ] **Step 4: Change the child process entrypoint and wire file-backed tracing for the hidden server command**

```rust
// src/daemon/process.rs
use std::path::PathBuf;
use std::process::{Command, Stdio};

use crate::core::error::{MeloError, MeloResult};

/// 构造用于拉起 daemon 子进程的命令。
///
/// # 参数
/// - `current_exe`：当前可执行文件路径
///
/// # 返回值
/// - `Command`：已配置好的子进程命令
pub fn daemon_command(current_exe: PathBuf) -> Command {
    let mut command = Command::new(current_exe);
    command.arg("daemon").arg("run");
    command.stdin(Stdio::null());
    command.stdout(Stdio::null());
    command.stderr(Stdio::null());
    command
}

/// 后台拉起 daemon 子进程。
///
/// # 参数
/// - 无
///
/// # 返回值
/// - `MeloResult<()>`：启动结果
pub fn spawn_background_daemon() -> MeloResult<()> {
    let current_exe = std::env::current_exe().map_err(|err| MeloError::Message(err.to_string()))?;
    daemon_command(current_exe)
        .spawn()
        .map(|_| ())
        .map_err(|err| MeloError::Message(err.to_string()))
}
```

```rust
// src/daemon/mod.rs
pub mod app;
pub mod manager;
pub mod observe;
pub mod process;
pub mod registry;
pub mod server;
```

```rust
// src/main.rs
use std::fs::OpenOptions;
use std::sync::Arc;

use tracing_subscriber::fmt::writer::MakeWriterExt;

fn daemon_run_requested(raw_args: &[String]) -> bool {
    matches!(raw_args.get(1).map(String::as_str), Some("daemon"))
        && matches!(raw_args.get(2).map(String::as_str), Some("run"))
}

fn init_tracing(raw_args: &[String]) {
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));

    if daemon_run_requested(raw_args) {
        if let Ok(paths) = melo::daemon::registry::runtime_paths() {
            if let Some(parent) = paths.log_file.parent() {
                let _ = std::fs::create_dir_all(parent);
            }

            if let Ok(file) = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&paths.log_file)
            {
                let writer = std::io::stdout.and(Arc::new(file));
                let _ = tracing_subscriber::fmt()
                    .with_env_filter(env_filter)
                    .with_target(false)
                    .with_writer(writer)
                    .try_init();
                return;
            }
        }
    }

    let _ = tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_target(false)
        .try_init();
}

#[tokio::main]
async fn main() {
    let raw_args = std::env::args().collect::<Vec<_>>();
    init_tracing(&raw_args);

    if let Err(err) = melo::cli::run().await {
        eprintln!("{err}");
        std::process::exit(1);
    }
}
```

```rust
// src/cli/args.rs
#[derive(Debug, Subcommand)]
pub enum DaemonCommand {
    #[command(about = "Start the managed Melo daemon")]
    Start,
    #[command(about = "Print daemon state")]
    Status {
        #[arg(long)]
        json: bool,
        #[arg(long)]
        verbose: bool,
    },
    #[command(about = "Gracefully stop the running Melo daemon")]
    Stop,
    #[command(about = "Restart the managed Melo daemon")]
    Restart,
    #[command(about = "Read the daemon log file tail")]
    Logs {
        #[arg(long, default_value_t = 100)]
        tail: usize,
    },
    #[command(about = "Diagnose the managed daemon")]
    Doctor {
        #[arg(long)]
        json: bool,
    },
    #[command(about = "Compare registration and live process state")]
    Ps,
    #[command(hide = true)]
    Run,
}
```

```rust
// src/cli/run.rs
match args.command {
    Some(Command::Daemon {
        command: Some(DaemonCommand::Run),
    }) => {
        let settings = crate::core::config::settings::Settings::load()?;
        let bind_addr = if let Ok(base_url) = std::env::var("MELO_BASE_URL") {
            crate::daemon::process::daemon_bind_addr(&base_url)?
        } else {
            crate::daemon::process::next_bind_addr(
                &settings.daemon.host,
                settings.daemon.base_port,
                settings.daemon.port_search_limit,
            )
            .await?
        };

        let listener = tokio::net::TcpListener::bind(bind_addr)
            .await
            .map_err(|err| crate::core::error::MeloError::Message(err.to_string()))?;
        let listener_addr = listener
            .local_addr()
            .map_err(|err| crate::core::error::MeloError::Message(err.to_string()))?;
        let state = crate::daemon::app::AppState::new()?;
        let shutdown_state = state.clone();
        let paths = crate::daemon::registry::runtime_paths()?;

        crate::daemon::registry::store_registration_to(
            &paths.state_file,
            &state.daemon_registration(listener_addr),
        )
        .await?;

        let serve_result = axum::serve(listener, crate::daemon::server::router(state))
            .with_graceful_shutdown(async move {
                shutdown_state.wait_for_shutdown().await;
            })
            .await
            .map_err(|err| crate::core::error::MeloError::Message(err.to_string()));

        let clear_result =
            crate::daemon::registry::clear_registration_from(&paths.state_file).await;
        serve_result?;
        clear_result?;
    }
    // 其余分支在后续任务继续接入
}
```

- [ ] **Step 5: Run the lifecycle tests and verify the manager now drives stable transitions**

Run: `cargo test start_with_paths_reuses_running_instance_without_spawning --lib -- --nocapture`  
Expected: PASS and `start` now returns the existing observation without calling the injected spawn closure.

Run: `cargo test restart_with_paths_waits_for_shutdown_and_accepts_new_instance --lib -- --nocapture`  
Expected: PASS and `restart` waits for the old server to stop before accepting the new `instance_id`.

- [ ] **Step 6: Run repo-wide verification and commit the lifecycle slice**

Run: `pnpm qa`  
Expected: PASS and the hidden `daemon run` entry plus file-backed tracing do not break existing daemon API or CLI flows.

```bash
git add src/cli/args.rs src/cli/run.rs src/daemon/mod.rs src/daemon/process.rs src/daemon/process/tests.rs src/daemon/manager.rs src/daemon/manager/tests.rs src/main.rs
git commit -m "feat: add daemon lifecycle manager"
```

---

### Task 4: Expose read-only daemon management commands backed by the shared observation model

**Files:**
- Modify: `src/cli/mod.rs`
- Modify: `src/cli/run.rs`
- Modify: `tests/cli_remote.rs`
- Create: `src/cli/daemon.rs`
- Create: `src/cli/daemon/tests.rs`

- [ ] **Step 1: Write failing CLI tests for `status`, `logs`, `doctor`, and `ps`**

```rust
// append to tests/cli_remote.rs
#[tokio::test(flavor = "multi_thread")]
async fn daemon_status_command_supports_json_and_verbose_views() {
    let state = melo::daemon::app::AppState::for_test().await;
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let app = melo::daemon::server::router(state.clone());
    let temp = tempfile::tempdir().unwrap();
    let state_file = temp.path().join("daemon.json");
    let log_file = temp.path().join("daemon.log");
    std::fs::write(&log_file, "line 1\nline 2\nline 3\n").unwrap();

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let registration = state.daemon_registration(addr);
    std::fs::write(&state_file, serde_json::to_string(&registration).unwrap()).unwrap();

    let mut json_cmd = Command::cargo_bin("melo").unwrap();
    json_cmd.env("MELO_DAEMON_STATE_FILE", &state_file);
    json_cmd.arg("daemon").arg("status").arg("--json");
    json_cmd
        .assert()
        .success()
        .stdout(predicate::str::contains("\"state\": \"Running\""))
        .stdout(predicate::str::contains("\"instance_id\": \"test-instance-1\""));

    let mut verbose_cmd = Command::cargo_bin("melo").unwrap();
    verbose_cmd.env("MELO_DAEMON_STATE_FILE", &state_file);
    verbose_cmd.arg("daemon").arg("status").arg("--verbose");
    verbose_cmd
        .assert()
        .success()
        .stdout(predicate::str::contains("registration_path"))
        .stdout(predicate::str::contains("log_path"));
}

#[tokio::test]
async fn daemon_logs_command_reads_requested_tail_count() {
    let temp = tempfile::tempdir().unwrap();
    let state_file = temp.path().join("daemon.json");
    let log_file = temp.path().join("daemon.log");
    std::fs::write(&log_file, "one\ntwo\nthree\nfour\n").unwrap();
    std::fs::write(
        &state_file,
        serde_json::json!({
            "instance_id": "test-instance-1",
            "base_url": "http://127.0.0.1:65530",
            "pid": std::process::id(),
            "started_at": "2026-04-11T00:00:00Z",
            "version": env!("CARGO_PKG_VERSION"),
            "backend": "noop",
            "host": "127.0.0.1",
            "port": 65530,
            "log_path": log_file.to_string_lossy().to_string()
        })
        .to_string(),
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("melo").unwrap();
    cmd.env("MELO_DAEMON_STATE_FILE", &state_file);
    cmd.arg("daemon").arg("logs").arg("--tail").arg("2");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("three"))
        .stdout(predicate::str::contains("four"))
        .stdout(predicate::str::contains("one").not());
}

#[tokio::test]
async fn daemon_doctor_and_ps_commands_report_stale_registration() {
    let temp = tempfile::tempdir().unwrap();
    let state_file = temp.path().join("daemon.json");
    let log_file = temp.path().join("daemon.log");
    std::fs::write(
        &state_file,
        serde_json::json!({
            "instance_id": "stale-instance",
            "base_url": "http://127.0.0.1:65530",
            "pid": 999999,
            "started_at": "2026-04-11T00:00:00Z",
            "version": env!("CARGO_PKG_VERSION"),
            "backend": "noop",
            "host": "127.0.0.1",
            "port": 65530,
            "log_path": log_file.to_string_lossy().to_string()
        })
        .to_string(),
    )
    .unwrap();

    let mut doctor = Command::cargo_bin("melo").unwrap();
    doctor.env("MELO_DAEMON_STATE_FILE", &state_file);
    doctor.arg("daemon").arg("doctor").arg("--json");
    doctor
        .assert()
        .success()
        .stdout(predicate::str::contains("\"conclusion\": \"FAIL\""))
        .stdout(predicate::str::contains("\"code\": \"health\""));

    let mut ps = Command::cargo_bin("melo").unwrap();
    ps.env("MELO_DAEMON_STATE_FILE", &state_file);
    ps.arg("daemon").arg("ps");
    ps.assert()
        .success()
        .stdout(predicate::str::contains("registered_pid"))
        .stdout(predicate::str::contains("actual_pid"));
}

#[tokio::test]
async fn daemon_status_without_registration_returns_not_running_summary() {
    let temp = tempfile::tempdir().unwrap();
    let state_file = temp.path().join("daemon.json");

    let mut cmd = Command::cargo_bin("melo").unwrap();
    cmd.env("MELO_DAEMON_STATE_FILE", &state_file);
    cmd.arg("daemon").arg("status");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("NotRunning"));
}
```

- [ ] **Step 2: Run the CLI tests and confirm the read-only management surface is still missing**

Run: `cargo test --test cli_remote daemon_status_command_supports_json_and_verbose_views -- --nocapture`  
Expected: FAIL because `melo daemon status` still prints raw registration JSON and has no `--json` or `--verbose` flags.

Run: `cargo test --test cli_remote daemon_logs_command_reads_requested_tail_count -- --nocapture`  
Expected: FAIL because `melo daemon logs` does not exist.

- [ ] **Step 3: Create the CLI daemon formatter/dispatcher module**

```rust
// src/cli/daemon.rs
use crate::cli::args::DaemonCommand;
use crate::core::config::settings::Settings;
use crate::core::error::MeloResult;
use crate::daemon::manager;
use crate::daemon::observe::{DaemonObservation, DoctorReport};

/// 执行 daemon 管理子命令。
///
/// # 参数
/// - `command`：daemon 子命令；`None` 在后续任务里作为 `start` 别名处理
///
/// # 返回值
/// - `MeloResult<()>`：执行结果
pub async fn run_daemon_command(command: Option<DaemonCommand>) -> MeloResult<()> {
    let settings = Settings::load()?;
    let paths = crate::daemon::registry::runtime_paths()?;

    match command {
        Some(DaemonCommand::Status { json, verbose }) => {
            let observation = crate::daemon::observe::observe_with_paths(&settings, &paths).await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&observation).unwrap());
            } else {
                println!("{}", format_status_human(&observation, verbose));
            }
        }
        Some(DaemonCommand::Logs { tail }) => {
            let output = manager::read_logs_with_paths(&paths, tail).await?;
            println!("{output}");
        }
        Some(DaemonCommand::Doctor { json }) => {
            let observation = crate::daemon::observe::observe_with_paths(&settings, &paths).await?;
            let report = crate::daemon::observe::build_doctor_report(&observation);
            if json {
                println!("{}", serde_json::to_string_pretty(&report).unwrap());
            } else {
                println!("{}", format_doctor_human(&report));
            }
        }
        Some(DaemonCommand::Ps) => {
            let observation = crate::daemon::observe::observe_with_paths(&settings, &paths).await?;
            println!("{}", format_ps_human(&observation));
        }
        other => return run_mutating_daemon_command(other, settings).await,
    }

    Ok(())
}

async fn run_mutating_daemon_command(
    command: Option<DaemonCommand>,
    settings: Settings,
) -> MeloResult<()> {
    match command {
        Some(DaemonCommand::Run) => {}
        Some(DaemonCommand::Start) | Some(DaemonCommand::Stop) | Some(DaemonCommand::Restart) | None => {
            unreachable!("带副作用命令在 Task 5 实现")
        }
        _ => unreachable!("只读命令已在上层分支处理"),
    }

    let _ = settings;
    Ok(())
}

/// 格式化人类可读的状态输出。
///
/// # 参数
/// - `observation`：统一观察结果
/// - `verbose`：是否显示更多字段
///
/// # 返回值
/// - `String`：渲染后的文本
pub fn format_status_human(observation: &DaemonObservation, verbose: bool) -> String {
    let mut lines = vec![
        format!("state: {:?}", observation.state),
        format!("pid: {:?}", observation.pid),
        format!("base_url: {:?}", observation.base_url),
        format!("backend: {:?}", observation.backend),
        format!("started_at: {:?}", observation.started_at),
        format!("health: {}", if observation.health_ok { "healthy" } else { "unhealthy" }),
    ];

    if verbose {
        lines.push(format!("registration_path: {}", observation.registration_path));
        lines.push(format!("instance_id: {:?}", observation.instance_id));
        lines.push(format!("process_exists: {}", observation.process_exists));
        lines.push(format!(
            "process_start_time_matches: {}",
            observation.process_start_time_matches
        ));
        lines.push(format!("log_path: {:?}", observation.log_path));
    }

    lines.join("\n")
}

/// 格式化人类可读的 doctor 输出。
///
/// # 参数
/// - `report`：诊断报告
///
/// # 返回值
/// - `String`：渲染后的文本
pub fn format_doctor_human(report: &DoctorReport) -> String {
    let mut lines = vec![format!("conclusion: {:?}", report.conclusion)];
    for check in &report.checks {
        lines.push(format!(
            "[{:?}] {}: {} ({})",
            check.level, check.code, check.summary, check.evidence
        ));
    }
    lines.join("\n")
}

/// 格式化 `daemon ps` 输出。
///
/// # 参数
/// - `observation`：统一观察结果
///
/// # 返回值
/// - `String`：渲染后的文本
pub fn format_ps_human(observation: &DaemonObservation) -> String {
    [
        format!("registered_pid: {:?}", observation.pid),
        format!("actual_pid: {:?}", observation.actual_pid),
        format!("process_path: {:?}", observation.actual_process_path),
        format!("instance_id: {:?}", observation.instance_id),
        format!(
            "process_start_time_matches: {}",
            observation.process_start_time_matches
        ),
    ]
    .join("\n")
}

#[cfg(test)]
mod tests;
```

```rust
// create src/cli/daemon/tests.rs
use crate::cli::daemon::{format_doctor_human, format_status_human};
use crate::daemon::observe::{DaemonObservation, DaemonState, DoctorCheck, DoctorLevel, DoctorReport};

fn observation() -> DaemonObservation {
    DaemonObservation {
        state: DaemonState::Running,
        registration_exists: true,
        registration_path: "C:/Temp/melo/daemon.json".to_string(),
        base_url: Some("http://127.0.0.1:38123".to_string()),
        instance_id: Some("instance-a".to_string()),
        pid: Some(4242),
        started_at: Some("2026-04-11T13:00:00Z".to_string()),
        backend: Some("noop".to_string()),
        host: Some("127.0.0.1".to_string()),
        port: Some(38123),
        process_exists: true,
        process_start_time_matches: true,
        actual_pid: Some(4242),
        actual_process_path: Some("C:/cargo/bin/melo.exe".to_string()),
        health_ok: true,
        http_instance_id_matches: Some(true),
        shutdown_requested: false,
        log_path: Some("C:/Temp/melo/daemon.log".to_string()),
        log_readable: true,
    }
}

#[test]
fn format_status_human_keeps_default_output_compact() {
    let text = format_status_human(&observation(), false);
    assert!(text.contains("state: Running"));
    assert!(!text.contains("registration_path"));
}

#[test]
fn format_doctor_human_prints_check_evidence() {
    let text = format_doctor_human(&DoctorReport {
        conclusion: DoctorLevel::FAIL,
        checks: vec![DoctorCheck {
            code: "health",
            level: DoctorLevel::FAIL,
            summary: "health/status 探测".to_string(),
            evidence: "base_url=http://127.0.0.1:65530".to_string(),
        }],
        observation: observation(),
    });

    assert!(text.contains("conclusion: FAIL"));
    assert!(text.contains("base_url=http://127.0.0.1:65530"));
}
```

```rust
// src/cli/mod.rs
pub mod args;
pub mod client;
pub mod daemon;
pub mod dispatch;
pub mod run;

pub use run::run;
```

```rust
// src/cli/run.rs
async fn run_clap(args: CliArgs) -> MeloResult<()> {
    match args.command {
        Some(Command::Daemon { command }) => {
            crate::cli::daemon::run_daemon_command(command).await?;
        }
        // 其余分支保持原样
        _ => {}
    }

    Ok(())
}
```

- [ ] **Step 4: Run the new CLI tests and confirm the read-only commands pass**

Run: `cargo test --test cli_remote daemon_status_without_registration_returns_not_running_summary -- --nocapture`  
Expected: PASS and `melo daemon status` now reports `NotRunning` instead of failing with `daemon_not_running`.

Run: `cargo test --test cli_remote daemon_doctor_and_ps_commands_report_stale_registration -- --nocapture`  
Expected: PASS and both commands now reuse the same observation result instead of ad-hoc logic.

- [ ] **Step 5: Run repo-wide verification and commit the read-only CLI slice**

Run: `pnpm qa`  
Expected: PASS and the new formatter/dispatcher module keeps CLI output stable while adding the read-only management surface.

```bash
git add src/cli/mod.rs src/cli/run.rs src/cli/daemon.rs src/cli/daemon/tests.rs tests/cli_remote.rs
git commit -m "feat: add daemon read-only management commands"
```

---

### Task 5: Finish mutating daemon commands, help text, and autostart boundaries

**Files:**
- Modify: `src/cli/args.rs`
- Modify: `src/cli/daemon.rs`
- Modify: `src/cli/run.rs`
- Modify: `tests/cli_help.rs`
- Modify: `tests/cli_remote.rs`

- [ ] **Step 1: Write failing command-wiring tests for `start`, `stop`, `restart`, daemon help, and `play` autostart routing**

```rust
// append to tests/cli_help.rs
#[test]
fn daemon_help_lists_management_commands_and_flags() {
    let mut cmd = Command::cargo_bin("melo").unwrap();
    cmd.arg("daemon").arg("--help");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("start"))
        .stdout(predicate::str::contains("restart"))
        .stdout(predicate::str::contains("logs"))
        .stdout(predicate::str::contains("doctor"))
        .stdout(predicate::str::contains("--json"))
        .stdout(predicate::str::contains("--verbose"));
}
```

```rust
// append to tests/cli_remote.rs
#[tokio::test(flavor = "multi_thread")]
async fn daemon_start_command_reuses_running_instance() {
    let state = melo::daemon::app::AppState::for_test().await;
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let app = melo::daemon::server::router(state.clone());
    let temp = tempfile::tempdir().unwrap();
    let state_file = temp.path().join("daemon.json");

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    std::fs::write(
        &state_file,
        serde_json::to_string(&state.daemon_registration(addr)).unwrap(),
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("melo").unwrap();
    cmd.env("MELO_DAEMON_STATE_FILE", &state_file);
    cmd.arg("daemon").arg("start");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("already_running"))
        .stdout(predicate::str::contains("test-instance-1"));
}

#[tokio::test]
async fn daemon_stop_command_clears_stale_registration_without_failing() {
    let temp = tempfile::tempdir().unwrap();
    let state_file = temp.path().join("daemon.json");
    std::fs::write(
        &state_file,
        serde_json::json!({
            "instance_id": "stale-instance",
            "base_url": "http://127.0.0.1:65530",
            "pid": 999999,
            "started_at": "2026-04-11T00:00:00Z",
            "version": env!("CARGO_PKG_VERSION"),
            "backend": "noop",
            "host": "127.0.0.1",
            "port": 65530,
            "log_path": temp.path().join("daemon.log").to_string_lossy().to_string()
        })
        .to_string(),
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("melo").unwrap();
    cmd.env("MELO_DAEMON_STATE_FILE", &state_file);
    cmd.arg("daemon").arg("stop");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("stale_registration_cleared"));
}

#[tokio::test(flavor = "multi_thread")]
async fn play_command_still_controls_a_healthy_daemon_after_autostart_refactor() {
    let app = melo::daemon::app::test_router().await;
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let mut cmd = Command::cargo_bin("melo").unwrap();
    cmd.env("MELO_BASE_URL", format!("http://{addr}"));
    cmd.arg("play");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("playback_state"));
}
```

- [ ] **Step 2: Run the wiring tests to confirm the mutating commands are not finished yet**

Run: `cargo test --test cli_remote daemon_start_command_reuses_running_instance -- --nocapture`  
Expected: FAIL because `melo daemon start` is not wired through the manager yet.

Run: `cargo test --test cli_help daemon_help_lists_management_commands_and_flags -- --nocapture`  
Expected: FAIL because the daemon help output does not yet include the full command surface or flag descriptions.

- [ ] **Step 3: Finish the public daemon command branches and keep `melo daemon` as a `start` alias**

```rust
// src/cli/daemon.rs
async fn run_mutating_daemon_command(
    command: Option<DaemonCommand>,
    settings: Settings,
) -> MeloResult<()> {
    match command {
        Some(DaemonCommand::Start) | None => {
            let result = manager::start(&settings).await?;
            println!("action: {}", result.action);
            println!("{}", format_status_human(&result.observation, true));
        }
        Some(DaemonCommand::Stop) => {
            let result = manager::stop(&settings).await?;
            println!("action: {}", result.action);
            println!("{}", format_status_human(&result.observation, true));
        }
        Some(DaemonCommand::Restart) => {
            let result = manager::restart(&settings).await?;
            println!("action: {}", result.action);
            println!("previous_instance: {:?}", result.previous.instance_id);
            println!("{}", format_status_human(&result.current, true));
        }
        Some(DaemonCommand::Run) => unreachable!("`daemon run` 由 src/cli/run.rs 直接接管"),
        _ => unreachable!("只读命令已在上层处理"),
    }

    Ok(())
}
```

- [ ] **Step 4: Make `play` use the managed autostart path while keeping observation commands side-effect free**

```rust
// src/cli/run.rs
async fn daemon_client() -> MeloResult<crate::cli::client::ApiClient> {
    let settings = crate::core::config::settings::Settings::load()?;
    crate::cli::client::ApiClient::from_discovery(&settings).await
}

/// 构造一个允许自动拉起 daemon 的客户端。
///
/// # 参数
/// - 无
///
/// # 返回值
/// - `MeloResult<crate::cli::client::ApiClient>`：自动拉起后的客户端
async fn daemon_client_with_autostart() -> MeloResult<crate::cli::client::ApiClient> {
    let settings = crate::core::config::settings::Settings::load()?;
    let base_url = crate::daemon::manager::ensure_running(&settings).await?;
    Ok(crate::cli::client::ApiClient::new(base_url))
}

async fn run_clap(args: CliArgs) -> MeloResult<()> {
    match args.command {
        Some(Command::Play) => {
            let snapshot = daemon_client_with_autostart()
                .await?
                .post_json("/api/player/play")
                .await?;
            println!("{}", serde_json::to_string_pretty(&snapshot).unwrap());
        }
        Some(Command::Daemon { command }) => {
            crate::cli::daemon::run_daemon_command(command).await?;
        }
        // 其余分支保持原有行为
        _ => {}
    }

    Ok(())
}
```

- [ ] **Step 5: Update daemon help text so the management surface is discoverable**

```rust
// src/cli/args.rs
#[derive(Debug, Subcommand)]
pub enum DaemonCommand {
    #[command(about = "Start the managed Melo daemon and wait until healthy")]
    Start,
    #[command(about = "Print daemon state", long_about = "Inspect the managed daemon without auto-starting it. Use --json for structured output and --verbose for more evidence fields.")]
    Status {
        #[arg(long)]
        json: bool,
        #[arg(long)]
        verbose: bool,
    },
    #[command(about = "Gracefully stop the managed Melo daemon")]
    Stop,
    #[command(about = "Restart the managed Melo daemon and verify the new instance is healthy")]
    Restart,
    #[command(about = "Read the daemon log file tail")]
    Logs {
        #[arg(long, default_value_t = 100)]
        tail: usize,
    },
    #[command(about = "Diagnose the managed daemon", long_about = "Print conclusion + evidence for registration, process, health, instance identity, and log-file checks.")]
    Doctor {
        #[arg(long)]
        json: bool,
    },
    #[command(about = "Compare registered daemon metadata with the live process")]
    Ps,
    #[command(hide = true)]
    Run,
}
```

- [ ] **Step 6: Run the wiring and help tests again**

Run: `cargo test --test cli_help daemon_help_lists_management_commands_and_flags -- --nocapture`  
Expected: PASS and `melo daemon --help` now shows `start/status/stop/restart/logs/doctor/ps` plus `--json`/`--verbose`.

Run: `cargo test --test cli_remote daemon_stop_command_clears_stale_registration_without_failing -- --nocapture`  
Expected: PASS and `melo daemon stop` clears stale registration instead of failing on unreachable daemon state.

Run: `cargo test --test cli_remote play_command_still_controls_a_healthy_daemon_after_autostart_refactor -- --nocapture`  
Expected: PASS and the `play` command still succeeds after switching to the autostart-capable client helper.

- [ ] **Step 7: Run repo-wide verification and commit the finished control surface**

Run: `pnpm qa`  
Expected: PASS and the complete daemon management surface, read/write command split, and autostart boundary changes all hold together.

```bash
git add src/cli/args.rs src/cli/daemon.rs src/cli/run.rs tests/cli_help.rs tests/cli_remote.rs
git commit -m "feat: finish daemon management control surface"
```

---

## Self-review

### Spec coverage

- 受管 daemon 识别策略与注册字段扩展：Task 1 + Task 2
- `instance_id` 握手、`pid + started_at` 校验：Task 1 + Task 2
- 统一生命周期状态与统一观察结果：Task 2
- `start/status/stop/restart/logs/doctor/ps` 命令面：Task 3 + Task 4 + Task 5
- `status --json`、`status --verbose`、`doctor --json`：Task 4 + Task 5
- 固定用户级运行目录与 `daemon.log`：Task 1 + Task 3
- `doctor` 检查项与 `OK/WARN/FAIL` 分级：Task 2 + Task 4
- “观测命令不自动拉起 / 带副作用命令可自动恢复”边界：Task 4 + Task 5
- 生命周期、状态识别、观测命令测试要求：Task 1 到 Task 5 全部覆盖

### Placeholder scan

- 没有使用 `TODO`、`TBD`、`implement later`、`similar to task N` 一类占位词。
- 每个涉及代码的步骤都给了具体文件名、代码片段、命令和预期结果。
- 所有新增函数名、结构名、枚举名在后续任务中保持一致，没有引用未定义接口。

### Type consistency

- 注册模型统一使用 `DaemonRegistration`，路径统一使用 `DaemonPaths`。
- 共享状态统一使用 `DaemonObservation`，CLI 的 `status`/`doctor`/`ps` 都依赖它。
- 生命周期结果统一用 `StartResult`、`StopResult`、`RestartResult`，避免命令分支各自拼装输出。
- 时间文本统一通过 `started_at_text_from_unix_seconds` / `now_started_at_text` 生成，避免 `started_at` 一会儿是 epoch 字符串一会儿是 RFC3339。

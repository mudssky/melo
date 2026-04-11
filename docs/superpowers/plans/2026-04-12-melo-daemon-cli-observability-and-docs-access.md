# Melo Daemon CLI 观测体验与文档访问控制 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 改善 Melo daemon 相关 CLI 在未启动和运行态下的观测体验，新增默认跟随日志、文档命令与文档访问控制，并统一配置文件/数据库路径解析策略。

**Architecture:** 在 CLI 层新增“观察类命令的 daemon 可用性解析”小层，统一把未运行/不可达状态映射成友好提示，而不是直接暴露 API 错误。配置层增加平台标准 Melo 根目录、配置文件与数据库路径覆盖和相对路径锚点解析；daemon 路由层基于文档访问策略控制 `/api/docs` 与 `/api/openapi.json` 的可见性。

**Tech Stack:** Rust, Clap, Axum, Tokio, Reqwest, Serde, pnpm, Vitest

---

## File Structure

- Create: `src/core/config/paths.rs`
  - 统一封装平台默认 Melo 根目录、默认配置文件路径、默认数据库路径与相对路径解析。
- Modify: `src/core/config/mod.rs`
  - 导出新的 `paths` 模块。
- Modify: `src/core/config/settings.rs`
  - 支持 `MELO_CONFIG_PATH` / `MELO_DB_PATH`、基于配置文件目录解析相对路径、增加文档访问配置。
- Create: `src/core/config/paths/tests.rs`
  - 验证平台默认路径和路径解析规则。
- Modify: `src/cli/args.rs`
  - 为 daemon logs 与 daemon docs 增加新参数，并视需要为全局 `--config` / `--db-path` 留入口。
- Modify: `src/cli/run.rs`
  - 引入观察类命令的友好提示入口，`status` 显示 docs 信息。
- Modify: `src/cli/daemon.rs`
  - 实现 `daemon logs` 默认跟随、`--snapshot`、`daemon docs`、统一友好提示。
- Create: `src/cli/observe.rs`
  - 封装观察类命令使用的 daemon 可用性解析和文案输出。
- Modify: `src/cli/mod.rs`
  - 导出新 `observe` 模块。
- Modify: `src/daemon/server.rs`
  - 根据 docs 配置和请求来源决定是否暴露文档路由。
- Modify: `src/daemon/app.rs`
  - 暴露文档 URL 计算所需的运行态信息。
- Modify: `src/api/system.rs`
  - 视需要扩展状态输出中的 docs 可见性信息。
- Modify: `config.example.toml`
  - 增加 docs 配置示例和新的路径说明。
- Modify: `tests/config_loading.rs`
  - 覆盖路径解析与 docs 配置加载。
- Modify: `tests/cli_remote.rs`
  - 覆盖 `status`、`daemon logs`、`daemon docs` 和观察类友好提示。
- Modify: `tests/api_server.rs`
  - 覆盖 docs 路由在 `local` / `disabled` 模式下的访问控制。

### Task 1: 收敛配置文件与数据库路径解析策略

**Files:**
- Create: `src/core/config/paths.rs`
- Modify: `src/core/config/mod.rs`
- Modify: `src/core/config/settings.rs`
- Modify: `tests/config_loading.rs`

- [ ] **Step 1: 写失败测试，锁定相对路径应相对配置文件目录解析**

```rust
#[test]
fn settings_resolve_database_path_relative_to_config_file_directory() {
    let temp = tempdir().unwrap();
    let config_dir = temp.path().join("melo-root");
    std::fs::create_dir_all(&config_dir).unwrap();
    let path = config_dir.join("config.toml");
    fs::write(
        &path,
        r#"
[database]
path = "melo.db"
"#,
    )
    .unwrap();

    let settings = Settings::load_from_path(&path).unwrap();
    assert_eq!(
        settings.database.path.as_std_path(),
        config_dir.join("melo.db").as_path()
    );
}
```

```rust
#[test]
fn settings_allow_database_path_override_from_env() {
    let temp = tempdir().unwrap();
    let config_path = temp.path().join("config.toml");
    let db_path = temp.path().join("override.db");
    fs::write(&config_path, "").unwrap();

    unsafe {
        std::env::set_var("MELO_DB_PATH", &db_path);
    }
    let settings = Settings::load_from_path(&config_path).unwrap();
    unsafe {
        std::env::remove_var("MELO_DB_PATH");
    }

    assert_eq!(settings.database.path.as_std_path(), db_path.as_path());
}
```

- [ ] **Step 2: 运行失败测试，确认当前实现仍按工作目录处理相对路径**

Run: `rtk cargo test -q --test config_loading`

Expected: FAIL，至少有数据库路径解析断言不通过。

- [ ] **Step 3: 新增平台 Melo 根目录与默认路径解析辅助**

```rust
// src/core/config/paths.rs
use std::path::{Path, PathBuf};

/// 返回平台默认 Melo 根目录。
///
/// # 参数
/// - 无
///
/// # 返回值
/// - `PathBuf`：平台标准 Melo 根目录
pub fn default_melo_root() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| std::env::current_dir().expect("current dir unavailable"))
        .join("melo")
}

/// 返回默认配置文件路径。
///
/// # 参数
/// - 无
///
/// # 返回值
/// - `PathBuf`：默认配置文件路径
pub fn default_config_path() -> PathBuf {
    default_melo_root().join("config.toml")
}

/// 返回默认数据库文件路径。
///
/// # 参数
/// - 无
///
/// # 返回值
/// - `PathBuf`：默认数据库路径
pub fn default_database_path() -> PathBuf {
    default_melo_root().join("melo.db")
}

/// 基于配置文件目录解析相对路径。
///
/// # 参数
/// - `config_path`：配置文件绝对路径
/// - `value`：配置中的路径值
///
/// # 返回值
/// - `PathBuf`：解析后的绝对路径
pub fn resolve_from_config_dir(config_path: &Path, value: &Path) -> PathBuf {
    if value.is_absolute() {
        return value.to_path_buf();
    }

    config_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(value)
}
```

- [ ] **Step 4: 在 `Settings` 中引入新的覆盖优先级和绝对路径收口**

```rust
pub fn load() -> MeloResult<Self> {
    let path = std::env::var("MELO_CONFIG_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| crate::core::config::paths::default_config_path());
    Self::load_from_path(path)
}

pub fn load_from_path(path: impl AsRef<std::path::Path>) -> MeloResult<Self> {
    let config_path = dunce::canonicalize(path.as_ref()).unwrap_or_else(|_| path.as_ref().to_path_buf());
    let builder = config::Config::builder()
        .add_source(config::File::from(config_path.as_path()).required(false))
        .set_default("database.path", "melo.db")?;
    let mut settings: Self = builder.build()?.try_deserialize()?;

    let resolved_db = std::env::var("MELO_DB_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            crate::core::config::paths::resolve_from_config_dir(
                &config_path,
                settings.database.path.as_std_path(),
            )
        });
    settings.database.path = Utf8PathBuf::from_path_buf(resolved_db)
        .map_err(|_| MeloError::Message("database path must be utf-8".to_string()))?;
    Ok(settings)
}
```

- [ ] **Step 5: 运行配置测试，确认路径策略稳定**

Run: `rtk cargo test -q --test config_loading`

Expected: PASS

- [ ] **Step 6: 提交路径解析与配置覆盖改造**

```bash
rtk git add src/core/config/paths.rs src/core/config/mod.rs src/core/config/settings.rs tests/config_loading.rs Cargo.toml Cargo.lock
rtk git commit -m "feat(config): resolve runtime paths from config root"
```

### Task 2: 为 docs 配置增加 `disabled/local/network` 模式

**Files:**
- Modify: `src/core/config/settings.rs`
- Modify: `config.example.toml`
- Modify: `tests/config_loading.rs`

- [ ] **Step 1: 写失败测试，锁定 docs 配置解析**

```rust
#[test]
fn settings_load_docs_visibility_mode() {
    let temp = tempdir().unwrap();
    let path = temp.path().join("config.toml");
    fs::write(
        &path,
        r#"
[daemon]
docs = "network"
"#,
    )
    .unwrap();

    let settings = Settings::load_from_path(&path).unwrap();
    assert_eq!(settings.daemon.docs.as_str(), "network");
}
```

- [ ] **Step 2: 运行失败测试，确认 daemon 配置尚未包含 docs 模式**

Run: `rtk cargo test -q --test config_loading settings_load_docs_visibility_mode`

Expected: FAIL，反序列化缺字段或默认值不匹配。

- [ ] **Step 3: 为 daemon 配置增加 docs 可见性枚举**

```rust
#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DaemonDocsMode {
    Disabled,
    Local,
    Network,
}

impl DaemonDocsMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::Local => "local",
            Self::Network => "network",
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct DaemonSettings {
    pub host: String,
    pub base_port: u16,
    pub port_search_limit: u16,
    pub docs: DaemonDocsMode,
}

impl Default for DaemonSettings {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            base_port: 38123,
            port_search_limit: 32,
            docs: DaemonDocsMode::Local,
        }
    }
}
```

- [ ] **Step 4: 更新示例配置，明确 docs 默认策略**

```toml
[daemon]
host = "127.0.0.1"
base_port = 38123
port_search_limit = 32
docs = "local"
```

- [ ] **Step 5: 运行配置测试**

Run: `rtk cargo test -q --test config_loading`

Expected: PASS

- [ ] **Step 6: 提交 docs 配置模型**

```bash
rtk git add src/core/config/settings.rs config.example.toml tests/config_loading.rs
rtk git commit -m "feat(config): add daemon docs visibility mode"
```

### Task 3: 为观察类命令增加统一友好提示层

**Files:**
- Create: `src/cli/observe.rs`
- Modify: `src/cli/mod.rs`
- Modify: `src/cli/run.rs`
- Modify: `tests/cli_remote.rs`

- [ ] **Step 1: 写失败测试，锁定 daemon 未启动时 `status` 不应暴露底层接口错误**

```rust
#[tokio::test(flavor = "multi_thread")]
async fn status_command_shows_friendly_hint_when_daemon_is_unavailable() {
    let temp = tempfile::tempdir().unwrap();
    let state_file = temp.path().join("missing-daemon.json");

    let mut cmd = Command::cargo_bin("melo").unwrap();
    cmd.env("MELO_DAEMON_STATE_FILE", &state_file);
    cmd.arg("status");

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("daemon"))
        .stderr(predicate::str::contains("melo daemon start"))
        .stderr(predicate::str::contains("api_error").not());
}
```

- [ ] **Step 2: 运行失败测试，确认当前仍然直接输出底层错误**

Run: `rtk cargo test -q --test cli_remote status_command_shows_friendly_hint_when_daemon_is_unavailable`

Expected: FAIL，stderr 中出现底层发现或 API 错误。

- [ ] **Step 3: 新增观察类命令的 daemon 可用性解析层**

```rust
// src/cli/observe.rs
use crate::core::error::{MeloError, MeloResult};

pub enum ObservedDaemon {
    Running { base_url: String },
    Unavailable { reason: String, hint: String },
}

pub async fn observe_read_only_daemon() -> MeloResult<ObservedDaemon> {
    let settings = crate::core::config::settings::Settings::load()?;
    match crate::cli::client::ApiClient::from_discovery(&settings).await {
        Ok(client) if client.health().await.is_ok() => Ok(ObservedDaemon::Running {
            base_url: client.base_url().to_string(),
        }),
        _ => Ok(ObservedDaemon::Unavailable {
            reason: "daemon is not running".to_string(),
            hint: "run `melo daemon start`".to_string(),
        }),
    }
}
```

- [ ] **Step 4: 让 `status` / `queue show` / `player mode show` 先接入友好提示**

```rust
match crate::cli::observe::observe_read_only_daemon().await? {
    ObservedDaemon::Running { .. } => {
        let client = daemon_client().await?;
        let snapshot = client.status().await?;
        println!("{}", serde_json::to_string_pretty(&snapshot).unwrap());
    }
    ObservedDaemon::Unavailable { reason, hint } => {
        eprintln!("{reason}");
        eprintln!("{hint}");
        return Err(MeloError::Message("daemon_unavailable".to_string()));
    }
}
```

- [ ] **Step 5: 运行 CLI 测试**

Run: `rtk cargo test -q --test cli_remote`

Expected: PASS，且未启动 daemon 时的观察类命令不再暴露 `api_error`。

- [ ] **Step 6: 提交观察类命令提示层**

```bash
rtk git add src/cli/observe.rs src/cli/mod.rs src/cli/run.rs tests/cli_remote.rs
rtk git commit -m "feat(cli): add friendly read-only daemon hints"
```

### Task 4: 实现 `daemon logs` 默认跟随与 `--snapshot`

**Files:**
- Modify: `src/cli/args.rs`
- Modify: `src/cli/daemon.rs`
- Modify: `src/daemon/manager.rs`
- Modify: `tests/cli_remote.rs`

- [ ] **Step 1: 写失败测试，锁定 logs 新参数和一次性模式**

```rust
#[tokio::test]
async fn daemon_logs_snapshot_prints_existing_tail_without_waiting() {
    let temp = tempfile::tempdir().unwrap();
    let state_file = temp.path().join("daemon.json");
    let log_file = temp.path().join("daemon.log");
    std::fs::write(&log_file, "one\ntwo\nthree\n").unwrap();
    std::fs::write(
        &state_file,
        serde_json::json!({
            "instance_id": "test-instance-1",
            "base_url": "http://127.0.0.1:65530",
            "pid": std::process::id(),
            "started_at": "2026-04-12T00:00:00Z",
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
    cmd.arg("daemon").arg("logs").arg("--snapshot").arg("--tail").arg("2");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("two"))
        .stdout(predicate::str::contains("three"));
}
```

- [ ] **Step 2: 运行失败测试，确认 `--snapshot` 尚不存在**

Run: `rtk cargo test -q --test cli_remote daemon_logs_snapshot_prints_existing_tail_without_waiting`

Expected: FAIL，Clap 不认识 `--snapshot`。

- [ ] **Step 3: 为 `daemon logs` 增加 `snapshot` 标志并实现跟随逻辑**

```rust
// src/cli/args.rs
Logs {
    #[arg(long, default_value_t = 100)]
    tail: usize,
    #[arg(long, default_value_t = false)]
    snapshot: bool,
}
```

```rust
// src/cli/daemon.rs
Some(DaemonCommand::Logs { tail, snapshot }) => {
    if snapshot {
        let output = manager::read_logs_with_paths(&paths, tail).await?;
        println!("{output}");
    } else {
        manager::follow_logs_with_paths(&paths, tail, &mut std::io::stdout()).await?;
    }
}
```

- [ ] **Step 4: 在 manager 中增加最小可用的文件跟随**

```rust
pub async fn follow_logs_with_paths(
    paths: &DaemonPaths,
    tail: usize,
    writer: &mut impl std::io::Write,
) -> MeloResult<()> {
    let mut seen = String::new();
    loop {
        let contents = tokio::fs::read_to_string(&paths.log_file).await.unwrap_or_default();
        if seen.is_empty() {
            writeln!(writer, "{}", tail_lines(&contents, tail)).ok();
        } else if contents.len() > seen.len() {
            write!(writer, "{}", &contents[seen.len()..]).ok();
        }
        seen = contents;
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    }
}
```

- [ ] **Step 5: 运行 CLI 测试**

Run: `rtk cargo test -q --test cli_remote daemon_logs_snapshot_prints_existing_tail_without_waiting daemon_logs_command_reads_requested_tail_count`

Expected: PASS

- [ ] **Step 6: 提交日志跟随改造**

```bash
rtk git add src/cli/args.rs src/cli/daemon.rs src/daemon/manager.rs tests/cli_remote.rs
rtk git commit -m "feat(cli): follow daemon logs by default"
```

### Task 5: 新增 `melo daemon docs` 与 `status` 中的 docs URL

**Files:**
- Modify: `src/cli/args.rs`
- Modify: `src/cli/daemon.rs`
- Modify: `src/cli/run.rs`
- Modify: `src/daemon/app.rs`
- Modify: `tests/cli_remote.rs`

- [ ] **Step 1: 写失败测试，锁定 `daemon docs` 命令行为**

```rust
#[tokio::test(flavor = "multi_thread")]
async fn daemon_docs_print_outputs_local_docs_url() {
    let state = melo::daemon::app::AppState::for_test().await;
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let registration = state.daemon_registration(addr);
    let app = melo::daemon::server::router(state);
    let temp = tempfile::tempdir().unwrap();
    let state_file = temp.path().join("daemon.json");
    std::fs::write(&state_file, serde_json::to_string(&registration).unwrap()).unwrap();

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let mut cmd = Command::cargo_bin("melo").unwrap();
    cmd.env("MELO_DAEMON_STATE_FILE", &state_file);
    cmd.arg("daemon").arg("docs").arg("--print");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("/api/docs/"));
}
```

- [ ] **Step 2: 运行失败测试，确认 `docs` 子命令尚不存在**

Run: `rtk cargo test -q --test cli_remote daemon_docs_print_outputs_local_docs_url`

Expected: FAIL，Clap 不认识 `docs` 子命令。

- [ ] **Step 3: 为 daemon 命令增加 `docs` 子命令与参数**

```rust
Docs {
    #[arg(long, default_value_t = false)]
    print: bool,
    #[arg(long, default_value_t = false)]
    openapi: bool,
}
```

- [ ] **Step 4: 实现 `daemon docs` 与 `status` 的 docs URL 渲染**

```rust
Some(DaemonCommand::Docs { print, openapi }) => {
    match crate::cli::observe::observe_read_only_daemon().await? {
        ObservedDaemon::Running { docs_url, openapi_url, .. } => {
            let url = if openapi { openapi_url } else { docs_url };
            if print {
                println!("{url}");
            } else {
                opener::open(url).map_err(|err| MeloError::Message(err.to_string()))?;
            }
        }
        ObservedDaemon::Unavailable { reason, hint } => {
            eprintln!("{reason}");
            eprintln!("{hint}");
            return Err(MeloError::Message("daemon_unavailable".to_string()));
        }
    }
}
```

- [ ] **Step 5: 让 `melo status` 在运行时展示 docs 字段**

```rust
println!("{}", serde_json::to_string_pretty(&serde_json::json!({
    "snapshot": snapshot,
    "docs": docs_url,
})).unwrap());
```

- [ ] **Step 6: 运行 CLI 测试**

Run: `rtk cargo test -q --test cli_remote`

Expected: PASS，且 `daemon docs --print` 与 `status` 的 docs 字段断言通过。

- [ ] **Step 7: 提交 docs 命令与状态输出**

```bash
rtk git add src/cli/args.rs src/cli/daemon.rs src/cli/run.rs src/daemon/app.rs tests/cli_remote.rs Cargo.toml Cargo.lock
rtk git commit -m "feat(cli): add daemon docs command"
```

### Task 6: 为 docs 路由增加 `disabled/local/network` 访问控制

**Files:**
- Modify: `src/daemon/server.rs`
- Modify: `src/daemon/app.rs`
- Modify: `tests/api_server.rs`

- [ ] **Step 1: 写失败测试，锁定 `local` 模式下拒绝非 loopback 访问**

```rust
#[tokio::test]
async fn docs_route_is_disabled_when_docs_mode_is_disabled() {
    let mut settings = melo::core::config::settings::Settings::default();
    settings.daemon.docs = melo::core::config::settings::DaemonDocsMode::Disabled;
    let app = melo::daemon::app::test_router_with_settings(settings).await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/docs/")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}
```

- [ ] **Step 2: 运行失败测试，确认当前无条件暴露 docs**

Run: `rtk cargo test -q --test api_server docs_route_is_disabled_when_docs_mode_is_disabled`

Expected: FAIL，当前返回不是 `404`。

- [ ] **Step 3: 在 server 中按 docs 模式条件注册路由**

```rust
let mut router = Router::new()
    .route("/api/system/health", axum::routing::get(crate::api::system::health));

if state.settings.daemon.docs != DaemonDocsMode::Disabled {
    router = router.merge(SwaggerUi::new("/api/docs").url(
        "/api/openapi.json",
        crate::api::docs::MeloOpenApi::openapi(),
    ));
}
```

- [ ] **Step 4: 对 `local` 模式增加 loopback 访问限制**

```rust
async fn docs_guard(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<std::net::SocketAddr>,
    request: Request,
    next: Next,
) -> Response {
    match state.settings.daemon.docs {
        DaemonDocsMode::Disabled => StatusCode::NOT_FOUND.into_response(),
        DaemonDocsMode::Local if !addr.ip().is_loopback() => StatusCode::FORBIDDEN.into_response(),
        _ => next.run(request).await,
    }
}
```

- [ ] **Step 5: 运行 API 测试**

Run: `rtk cargo test -q --test api_server`

Expected: PASS

- [ ] **Step 6: 提交 docs 访问控制**

```bash
rtk git add src/daemon/server.rs src/daemon/app.rs tests/api_server.rs
rtk git commit -m "feat(api): guard docs routes by visibility mode"
```

### Task 7: 全量验证与收尾

**Files:**
- Modify: `tests/cli_remote.rs`
- Modify: `tests/api_server.rs`
- Modify: `tests/config_loading.rs`
- Modify: `config.example.toml`

- [ ] **Step 1: 运行配置、API 与 CLI 测试**

Run: `rtk cargo test -q --test config_loading --test api_server --test cli_remote`

Expected: PASS

- [ ] **Step 2: 运行前端脚本测试**

Run: `pnpm test:dev-cli`

Expected: PASS

- [ ] **Step 3: 运行项目总体验证**

Run: `pnpm qa`

Expected: PASS

- [ ] **Step 4: 提交最终整合结果**

```bash
rtk git status --short
rtk git add src/core/config src/cli src/daemon src/api/system.rs config.example.toml tests/config_loading.rs tests/api_server.rs tests/cli_remote.rs Cargo.toml Cargo.lock
rtk git commit -m "feat(cli): improve daemon observability and docs access"
```

## Self-Review

- Spec coverage:
  - 观察类命令友好提示：Task 3
  - `daemon logs` 默认跟随：Task 4
  - `daemon docs` 默认打开与 `--print` / `--openapi`：Task 5
  - docs `disabled/local/network` 配置：Task 2、Task 6
  - `status` 显示 docs URL：Task 5
  - 配置文件/数据库路径锚点与覆盖：Task 1
- Placeholder scan:
  - 已避免使用 `TBD`、`TODO`、`later` 等占位词。
- Type consistency:
  - 统一使用 `DaemonDocsMode`、`ObservedDaemon`、`MELO_CONFIG_PATH`、`MELO_DB_PATH` 和 `--snapshot` 作为后续任务中的核心名称。

# Melo Database Bootstrap On Daemon Start Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 让 daemon 在启动阶段统一完成数据库目录创建与 schema 初始化，保证首次使用 `melo <文件>` 时不会因为数据库父目录不存在而失败。

**Architecture:** 复用现有 `DatabaseBootstrap` 作为数据库运行时准备入口，不把副作用下沉到 `connect()`。在 `AppState::new()` 中先执行数据库准备，再构造业务服务；同时补 CLI/daemon 回归测试，锁定“目录不存在也能启动”和“失败时错误停留在启动阶段”的行为。

**Tech Stack:** Rust, Tokio, SeaORM, SQLite, Axum, Assert Cmd, pnpm

---

## File Structure

- Modify: `src/daemon/app.rs`
  - 在生产 daemon 状态构造前统一调用数据库 bootstrap。
- Modify: `src/core/db/bootstrap.rs`
  - 如有必要，补更清晰的准备入口或错误包装，但不改变它“建目录 + 迁移”的核心职责。
- Modify: `src/cli/run.rs`
  - 仅在需要时补更友好的启动失败错误透传；不额外加入 direct-open 私有初始化分支。
- Modify: `tests/db_bootstrap.rs`
  - 增加“父目录不存在时自动创建”的回归测试。
- Modify: `tests/cli_remote.rs`
  - 增加 direct-open / daemon 启动链路下数据库目录缺失仍可完成启动的集成测试。

### Task 1: 锁定数据库 bootstrap 的目录创建契约

**Files:**
- Modify: `tests/db_bootstrap.rs`
- Modify: `src/core/db/bootstrap.rs`

- [ ] **Step 1: 写失败测试，锁定缺失父目录时也能完成初始化**

```rust
#[tokio::test]
async fn db_init_creates_missing_parent_directory_before_connecting() {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("nested/runtime/melo.db");
    assert!(!db_path.parent().unwrap().exists());

    let settings = Settings::for_test(db_path.clone());
    DatabaseBootstrap::new(&settings).init().await.unwrap();

    assert!(db_path.parent().unwrap().exists());
    assert!(db_path.exists());
}
```

- [ ] **Step 2: 运行测试，确认当前行为是否已被覆盖**

Run: `rtk cargo test -q --test db_bootstrap`

Expected: PASS 或新增测试 FAIL；无论结果如何都要记录现状，确保这条契约被纳入测试。

- [ ] **Step 3: 如有需要，收口 bootstrap 入口命名或错误包装**

```rust
impl<'a> DatabaseBootstrap<'a> {
    /// 为 daemon 与 CLI 运行时准备数据库。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - `MeloResult<()>`：准备结果
    pub async fn prepare_runtime_database(&self) -> MeloResult<()> {
        self.init().await.map_err(|err| {
            MeloError::Message(format!("failed to prepare database: {err}"))
        })
    }
}
```

- [ ] **Step 4: 运行测试，确认 bootstrap 契约稳定**

Run: `rtk cargo test -q --test db_bootstrap`

Expected: PASS

- [ ] **Step 5: 提交 bootstrap 契约改动**

```bash
rtk git add tests/db_bootstrap.rs src/core/db/bootstrap.rs
rtk git commit -m "test(db): lock bootstrap directory creation"
```

### Task 2: 在 daemon 启动前统一准备数据库

**Files:**
- Modify: `src/daemon/app.rs`
- Modify: `src/core/db/bootstrap.rs`

- [ ] **Step 1: 写失败测试，锁定生产态 `AppState::new()` 会先准备数据库**

```rust
#[tokio::test]
async fn daemon_app_state_new_bootstraps_database_before_services_use_it() {
    let temp = tempfile::tempdir().unwrap();
    let config_path = temp.path().join("config.toml");
    std::fs::write(
        &config_path,
        r#"
[database]
path = "runtime/melo.db"
"#,
    )
    .unwrap();

    unsafe {
        std::env::set_var("MELO_CONFIG_PATH", &config_path);
    }
    let state = melo::daemon::app::AppState::new().await;
    unsafe {
        std::env::remove_var("MELO_CONFIG_PATH");
    }

    assert!(state.is_ok());
    assert!(temp.path().join("runtime").exists());
    assert!(temp.path().join("runtime/melo.db").exists());
}
```

- [ ] **Step 2: 运行测试，确认当前 daemon 启动前尚未统一 bootstrap**

Run: `rtk cargo test -q daemon_app_state_new_bootstraps_database_before_services_use_it`

Expected: FAIL，如果当前路径恰好已能通过，也要继续实现显式调用，让责任边界清晰可见。

- [ ] **Step 3: 在 `AppState::new()` 中把数据库准备放到业务服务构造前**

```rust
pub fn new() -> MeloResult<Self> {
    let settings = Settings::load()?;
    crate::core::db::bootstrap::DatabaseBootstrap::new(&settings)
        .prepare_runtime_database()
        .await?;
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
```

- [ ] **Step 4: 如编译器要求，调整 `AppState::new()` 的异步签名与调用点**

```rust
pub async fn new() -> MeloResult<Self> { /* ... */ }
```

```rust
let state = crate::daemon::app::AppState::new().await?;
```

- [ ] **Step 5: 运行 daemon / bootstrap 相关测试**

Run: `rtk cargo test -q --test db_bootstrap`

Expected: PASS

- [ ] **Step 6: 提交 daemon 启动链路改动**

```bash
rtk git add src/daemon/app.rs src/core/db/bootstrap.rs src/cli/run.rs tests/db_bootstrap.rs
rtk git commit -m "feat(daemon): bootstrap database before startup"
```

### Task 3: 为 direct-open 增加数据库目录缺失回归测试

**Files:**
- Modify: `tests/cli_remote.rs`
- Modify: `src/cli/run.rs`

- [ ] **Step 1: 写失败测试，锁定 direct-open 首次使用不会因数据库目录缺失而失败**

```rust
#[test]
fn direct_open_bootstraps_database_when_parent_directory_is_missing() {
    let temp = tempfile::tempdir().unwrap();
    let music_file = temp.path().join("Always Online - 林俊杰.flac");
    std::fs::write(&music_file, b"not-a-real-flac").unwrap();
    let config_path = temp.path().join("config.toml");
    std::fs::write(
        &config_path,
        r#"
[database]
path = "nested/runtime/melo.db"
"#,
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("melo").unwrap();
    cmd.env("MELO_CONFIG_PATH", &config_path);
    cmd.arg(&music_file);

    cmd.assert().success();
    assert!(temp.path().join("nested/runtime").exists());
}
```

- [ ] **Step 2: 运行测试，确认当前失败模式与用户报告一致**

Run: `rtk cargo test -q --test cli_remote direct_open_bootstraps_database_when_parent_directory_is_missing`

Expected: FAIL，并出现数据库连接或 daemon 启动链路相关错误。

- [ ] **Step 3: 若 direct-open 因测试夹具需要，改为断言“已跨过数据库错误阶段”**

```rust
cmd.assert()
    .failure()
    .stderr(predicate::str::contains("unable to open database file").not());

assert!(temp.path().join("nested/runtime").exists());
```

- [ ] **Step 4: 运行 CLI 测试，确认 direct-open 回归稳定**

Run: `rtk cargo test -q --test cli_remote`

Expected: PASS，且 stderr 不再包含 `unable to open database file`

- [ ] **Step 5: 提交 CLI 回归测试**

```bash
rtk git add tests/cli_remote.rs src/cli/run.rs
rtk git commit -m "test(cli): cover direct open database bootstrap"
```

### Task 4: 统一数据库准备失败的错误语义

**Files:**
- Modify: `src/core/db/bootstrap.rs`
- Modify: `src/cli/run.rs`
- Modify: `tests/cli_remote.rs`

- [ ] **Step 1: 写失败测试，锁定数据库准备失败时的错误前缀**

```rust
#[test]
fn daemon_start_reports_database_prepare_failure_when_path_is_invalid() {
    let temp = tempfile::tempdir().unwrap();
    let config_path = temp.path().join("config.toml");
    std::fs::write(
        &config_path,
        r#"
[database]
path = ""
"#,
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("melo").unwrap();
    cmd.env("MELO_CONFIG_PATH", &config_path);
    cmd.arg("daemon").arg("start");

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("failed to prepare database"));
}
```

- [ ] **Step 2: 运行测试，确认当前错误文案仍然过于底层**

Run: `rtk cargo test -q --test cli_remote daemon_start_reports_database_prepare_failure_when_path_is_invalid`

Expected: FAIL，stderr 中缺少 `failed to prepare database`

- [ ] **Step 3: 在 bootstrap 层统一包装错误文案**

```rust
fn map_prepare_error(stage: &str, err: impl std::fmt::Display) -> MeloError {
    MeloError::Message(format!("failed to prepare database: {stage}: {err}"))
}
```

```rust
if let Some(parent) = path.parent() {
    fs::create_dir_all(parent).map_err(|err| map_prepare_error("create_dir", err))?;
}

let connection = connect(self.settings)
    .await
    .map_err(|err| map_prepare_error("connect", err))?;
crate::core::db::migrator::Migrator::up(&connection, None)
    .await
    .map_err(|err| map_prepare_error("migrate", err))?;
```

- [ ] **Step 4: 运行 CLI 测试，确认用户能看到稳定错误语义**

Run: `rtk cargo test -q --test cli_remote`

Expected: PASS

- [ ] **Step 5: 提交错误语义改动**

```bash
rtk git add src/core/db/bootstrap.rs tests/cli_remote.rs src/cli/run.rs
rtk git commit -m "feat(cli): clarify database bootstrap failures"
```

### Task 5: 全量验证与收尾

**Files:**
- Modify: `tests/db_bootstrap.rs`
- Modify: `tests/cli_remote.rs`
- Modify: `src/daemon/app.rs`
- Modify: `src/core/db/bootstrap.rs`

- [ ] **Step 1: 运行 Rust 定向测试**

Run: `rtk cargo test -q --test db_bootstrap --test cli_remote`

Expected: PASS

- [ ] **Step 2: 运行项目总体验证**

Run: `pnpm qa`

Expected: PASS

- [ ] **Step 3: 检查工作区变更**

Run: `rtk git status --short`

Expected: 仅包含本次数据库 bootstrap 相关改动

- [ ] **Step 4: 提交最终整合结果**

```bash
rtk git add src/core/db/bootstrap.rs src/daemon/app.rs src/cli/run.rs tests/db_bootstrap.rs tests/cli_remote.rs Cargo.toml Cargo.lock
rtk git commit -m "feat(daemon): bootstrap database before first use"
```

## Self-Review

- Spec coverage:
  - daemon 启动阶段统一数据库准备：Task 2
  - 首次使用 direct-open 自动初始化：Task 3
  - 数据库准备失败使用更明确错误：Task 4
  - 不把初始化下沉到连接层：通过 Task 2 保持在 `AppState::new()` 收口
- Placeholder scan:
  - 已移除 `TODO` / `TBD` / “自行处理”等占位描述；每个任务都包含文件、测试和命令。
- Type consistency:
  - 计划统一使用 `DatabaseBootstrap`、`prepare_runtime_database`、`AppState::new()`、`failed to prepare database` 作为核心名称，后续实现需保持一致。

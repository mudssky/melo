# Melo Daemon Registration, MPV Backend, and TUI Help Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add global daemon registration with configurable high-port auto-avoidance, broaden direct-open format support, ship a runnable `mpv` backend plus `auto` backend selection, and make the TUI clearly show queue/help/startup state.

**Architecture:** Introduce a user-scoped daemon registration file that records the active daemon endpoint, then route all CLI/TUI discovery through that registry instead of hard-coding `127.0.0.1:8080`. Centralize supported audio extension logic in one module, keep `PlaybackBackend` as the single control abstraction, add a backend factory that resolves `auto -> mpv -> rodio`, and enrich the TUI with a lightweight launch context, queue list, and help popup without rebuilding the entire interface model.

**Tech Stack:** Rust 2024, Clap 4, Tokio, Axum, Reqwest, Ratatui, Crossterm, SeaORM-backed config/tests, external `mpv` IPC on Windows named pipes, `pnpm qa`

---

## File structure impact

### Existing files to modify

- Modify: `Cargo.toml`
- Modify: `src/core/config/settings.rs`
- Modify: `src/core/model/player.rs`
- Modify: `src/daemon/app.rs`
- Modify: `src/daemon/mod.rs`
- Modify: `src/daemon/process.rs`
- Modify: `src/domain/library/service.rs`
- Modify: `src/domain/open/mod.rs`
- Modify: `src/domain/open/service.rs`
- Modify: `src/domain/player/backend.rs`
- Modify: `src/domain/player/mod.rs`
- Modify: `src/domain/player/service.rs`
- Modify: `src/cli/client.rs`
- Modify: `src/cli/run.rs`
- Modify: `src/tui/app.rs`
- Modify: `src/tui/run.rs`
- Modify: `src/tui/ui/content.rs`
- Modify: `src/tui/ui/layout.rs`
- Modify: `src/tui/ui/popup.rs`
- Modify: `tests/config_loading.rs`
- Modify: `tests/open_api.rs`
- Modify: `tests/cli_remote.rs`
- Modify: `tests/tui_app.rs`
- Modify: `tests/api_server.rs`
- Modify: `config.example.toml`

### New files to create

- Create: `src/daemon/registry.rs`
- Create: `src/daemon/registry/tests.rs`
- Create: `src/domain/open/formats.rs`
- Create: `src/domain/open/formats/tests.rs`
- Create: `src/domain/player/factory.rs`
- Create: `src/domain/player/factory/tests.rs`
- Create: `src/domain/player/mpv_backend.rs`
- Create: `src/domain/player/mpv_backend/tests.rs`

### Responsibilities

- `src/daemon/registry.rs`
  Resolve the global daemon state file path, persist active daemon metadata, and load/clear stale registrations.
- `src/daemon/process.rs`
  Choose a bind address from the configured high-port range, register the running daemon, and let clients discover or restart it.
- `src/domain/open/formats.rs`
  Centralize case-insensitive supported audio extension checks for both direct-open and library scanning.
- `src/domain/player/factory.rs`
  Resolve `player.backend = auto|rodio|mpv` into a concrete backend implementation.
- `src/domain/player/mpv_backend.rs`
  Manage the `mpv` child process, JSON IPC command flow, event parsing, and playback callbacks.
- `src/tui/run.rs`
  Carry launch context into the TUI, render queue/help state, and show startup notices instead of swallowing them.

---

### Task 1: Extend configuration and add daemon registration foundation

**Files:**
- Create: `src/daemon/registry.rs`
- Create: `src/daemon/registry/tests.rs`
- Modify: `src/daemon/mod.rs`
- Modify: `src/core/config/settings.rs`
- Modify: `tests/config_loading.rs`
- Modify: `config.example.toml`

- [ ] **Step 1: Write the failing config and registry tests**

```rust
// append to tests/config_loading.rs
#[test]
fn settings_load_daemon_backend_and_tui_fields() {
    let temp = tempdir().unwrap();
    let path = temp.path().join("config.toml");
    fs::write(
        &path,
        r#"
[database]
path = "local/melo.db"

[daemon]
host = "127.0.0.1"
base_port = 38123
port_search_limit = 12

[player]
backend = "auto"
volume = 70
restore_last_session = true
resume_after_restore = false

[player.mpv]
path = "C:/Tools/mpv.exe"
ipc_dir = "auto"

[tui]
show_footer_hints = false
"#,
    )
    .unwrap();

    let settings = Settings::load_from_path(&path).unwrap();

    assert_eq!(settings.daemon.host, "127.0.0.1");
    assert_eq!(settings.daemon.base_port, 38123);
    assert_eq!(settings.daemon.port_search_limit, 12);
    assert_eq!(settings.player.backend, "auto");
    assert_eq!(settings.player.mpv.path, "C:/Tools/mpv.exe");
    assert_eq!(settings.player.mpv.ipc_dir, "auto");
    assert!(!settings.tui.show_footer_hints);
}
```

```rust
// src/daemon/registry/tests.rs
use std::path::PathBuf;

use crate::daemon::registry::{state_file_path_from_env, DaemonRegistration};

#[test]
fn state_file_path_prefers_explicit_override() {
    let root = PathBuf::from("C:/Temp/melo-tests");
    let path = state_file_path_from_env(Some(root.clone()), None, None).unwrap();
    assert_eq!(path, root.join("daemon.json"));
}

#[test]
fn state_file_path_defaults_under_localappdata() {
    let path = state_file_path_from_env(None, Some(PathBuf::from("C:/Users/test/AppData/Local")), None)
        .unwrap();
    assert_eq!(path, PathBuf::from("C:/Users/test/AppData/Local").join("melo").join("daemon.json"));
}

#[test]
fn registration_round_trips_json() {
    let registration = DaemonRegistration {
        base_url: "http://127.0.0.1:38123".to_string(),
        pid: 4242,
        started_at: "2026-04-11T13:00:00Z".to_string(),
        version: "0.1.0".to_string(),
        backend: "mpv".to_string(),
        host: "127.0.0.1".to_string(),
        port: 38123,
    };

    let json = serde_json::to_string(&registration).unwrap();
    let decoded: DaemonRegistration = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.base_url, "http://127.0.0.1:38123");
    assert_eq!(decoded.backend, "mpv");
}
```

- [ ] **Step 2: Run the focused tests to verify they fail**

Run: `cargo test --test config_loading settings_load_daemon_backend_and_tui_fields -- --nocapture`  
Expected: FAIL because `Settings` does not yet expose `daemon`, `player.backend`, `player.mpv`, or `tui`.

Run: `cargo test state_file_path_prefers_explicit_override --lib -- --nocapture`  
Expected: FAIL because `daemon::registry` and `DaemonRegistration` do not exist yet.

- [ ] **Step 3: Implement the new settings model and daemon registry module**

```rust
// src/core/config/settings.rs
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct DaemonSettings {
    pub host: String,
    pub base_port: u16,
    pub port_search_limit: u16,
}

impl Default for DaemonSettings {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            base_port: 38123,
            port_search_limit: 32,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct MpvSettings {
    pub path: String,
    pub ipc_dir: String,
    pub extra_args: Vec<String>,
}

impl Default for MpvSettings {
    fn default() -> Self {
        Self {
            path: "mpv".to_string(),
            ipc_dir: "auto".to_string(),
            extra_args: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct PlayerSettings {
    pub backend: String,
    pub volume: u8,
    pub restore_last_session: bool,
    pub resume_after_restore: bool,
    pub mpv: MpvSettings,
}

impl Default for PlayerSettings {
    fn default() -> Self {
        Self {
            backend: "auto".to_string(),
            volume: 100,
            restore_last_session: true,
            resume_after_restore: false,
            mpv: MpvSettings::default(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct TuiSettings {
    pub show_footer_hints: bool,
}

impl Default for TuiSettings {
    fn default() -> Self {
        Self {
            show_footer_hints: true,
        }
    }
}
```

```rust
// src/daemon/registry.rs
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct DaemonRegistration {
    pub base_url: String,
    pub pid: u32,
    pub started_at: String,
    pub version: String,
    pub backend: String,
    pub host: String,
    pub port: u16,
}

pub fn state_file_path_from_env(
    explicit: Option<std::path::PathBuf>,
    local_app_data: Option<std::path::PathBuf>,
    home_dir: Option<std::path::PathBuf>,
) -> crate::core::error::MeloResult<std::path::PathBuf> {
    if let Some(path) = explicit {
        return Ok(path);
    }

    if let Some(root) = local_app_data {
        return Ok(root.join("melo").join("daemon.json"));
    }

    let home = home_dir.ok_or_else(|| {
        crate::core::error::MeloError::Message("daemon_state_path_unavailable".to_string())
    })?;
    Ok(home.join(".local").join("share").join("melo").join("daemon.json"))
}

#[cfg(test)]
mod tests;
```

```toml
# config.example.toml
[daemon]
host = "127.0.0.1"
base_port = 38123
port_search_limit = 32

[player]
backend = "auto"
volume = 100
restore_last_session = true
resume_after_restore = false

[player.mpv]
path = "mpv"
ipc_dir = "auto"
extra_args = []

[tui]
show_footer_hints = true
```

- [ ] **Step 4: Run the focused tests and verify they pass**

Run: `cargo test --test config_loading settings_load_daemon_backend_and_tui_fields -- --nocapture`  
Expected: PASS and the new daemon/backend/TUI config fields deserialize from TOML.

Run: `cargo test state_file_path_prefers_explicit_override --lib -- --nocapture`  
Expected: PASS and the registry path helpers resolve deterministic override and LocalAppData locations.

- [ ] **Step 5: Run project QA before committing**

Run: `pnpm qa`  
Expected: PASS including formatting, clippy, and the full test suite.

- [ ] **Step 6: Commit the config/registry foundation**

```bash
git add src/core/config/settings.rs src/daemon/mod.rs src/daemon/registry.rs src/daemon/registry/tests.rs tests/config_loading.rs config.example.toml
git commit -m "feat: add daemon registration config foundation"
```

---

### Task 2: Make daemon startup and client discovery registry-driven

**Files:**
- Modify: `src/daemon/process.rs`
- Modify: `src/daemon/process/tests.rs`
- Modify: `src/cli/client.rs`
- Modify: `src/cli/run.rs`
- Modify: `tests/cli_remote.rs`

- [ ] **Step 1: Write the failing port-scan and registry-discovery tests**

```rust
// append to src/daemon/process/tests.rs
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
// append to tests/cli_remote.rs
#[tokio::test(flavor = "multi_thread")]
async fn status_command_uses_registered_daemon_url() {
    let app = melo::daemon::app::test_router().await;
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let temp = tempfile::tempdir().unwrap();
    let state_file = temp.path().join("daemon.json");

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    std::fs::write(
        &state_file,
        serde_json::json!({
            "base_url": format!("http://{addr}"),
            "pid": std::process::id(),
            "started_at": "2026-04-11T13:30:00Z",
            "version": env!("CARGO_PKG_VERSION"),
            "backend": "rodio",
            "host": "127.0.0.1",
            "port": addr.port()
        })
        .to_string(),
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("melo").unwrap();
    cmd.env("MELO_DAEMON_STATE_FILE", &state_file);
    cmd.arg("status");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("playback_state"));
}
```

- [ ] **Step 2: Run the focused tests to verify they fail**

Run: `cargo test next_bind_addr_skips_busy_base_port --lib -- --nocapture`  
Expected: FAIL because `next_bind_addr()` does not exist yet.

Run: `cargo test --test cli_remote status_command_uses_registered_daemon_url -- --nocapture`  
Expected: FAIL because CLI discovery still ignores the daemon registration file.

- [ ] **Step 3: Implement port scanning, registration persistence, and client resolution**

```rust
// src/daemon/process.rs
pub async fn next_bind_addr(host: &str, base_port: u16, search_limit: u16) -> MeloResult<SocketAddr> {
    for offset in 0..=search_limit {
        let port = base_port.saturating_add(offset);
        let candidate: SocketAddr = format!("{host}:{port}")
            .parse()
            .map_err(|err: std::net::AddrParseError| MeloError::Message(err.to_string()))?;
        if tokio::net::TcpListener::bind(candidate).await.is_ok() {
            return Ok(candidate);
        }
    }

    Err(MeloError::Message("daemon_port_unavailable".to_string()))
}

pub async fn resolve_base_url(settings: &crate::core::config::settings::Settings) -> MeloResult<String> {
    if let Ok(explicit) = std::env::var("MELO_BASE_URL") {
        return Ok(explicit);
    }

    if let Some(registration) = crate::daemon::registry::load_registration().await? {
        let client = crate::cli::client::ApiClient::new(registration.base_url.clone());
        if client.health().await.is_ok() {
            return Ok(registration.base_url);
        }
        crate::daemon::registry::clear_registration().await?;
    }

    Ok(format!("http://{}:{}", settings.daemon.host, settings.daemon.base_port))
}
```

```rust
// src/cli/client.rs
pub async fn from_discovery(
    settings: &crate::core::config::settings::Settings,
) -> MeloResult<Self> {
    let base_url = crate::daemon::process::resolve_base_url(settings).await?;
    Ok(Self::new(base_url))
}
```

```rust
// src/cli/run.rs
async fn daemon_client() -> crate::core::error::MeloResult<crate::cli::client::ApiClient> {
    let settings = crate::core::config::settings::Settings::load()?;
    crate::cli::client::ApiClient::from_discovery(&settings).await
}
```

- [ ] **Step 4: Run the focused tests and verify they pass**

Run: `cargo test next_bind_addr_skips_busy_base_port --lib -- --nocapture`  
Expected: PASS and busy base ports are skipped in favor of the next free high port.

Run: `cargo test --test cli_remote status_command_uses_registered_daemon_url -- --nocapture`  
Expected: PASS and `melo status` now honors the registered daemon URL instead of assuming `127.0.0.1:8080`.

- [ ] **Step 5: Run project QA before committing**

Run: `pnpm qa`  
Expected: PASS including formatting, clippy, and the full test suite.

- [ ] **Step 6: Commit the registry-driven startup/discovery slice**

```bash
git add src/daemon/process.rs src/daemon/process/tests.rs src/cli/client.rs src/cli/run.rs tests/cli_remote.rs
git commit -m "feat: add registry-driven daemon discovery"
```

---

### Task 3: Unify supported audio formats and surface direct-open errors clearly

**Files:**
- Create: `src/domain/open/formats.rs`
- Create: `src/domain/open/formats/tests.rs`
- Modify: `src/domain/open/mod.rs`
- Modify: `src/domain/open/service.rs`
- Modify: `src/domain/library/service.rs`
- Modify: `src/api/open.rs`
- Modify: `src/cli/client.rs`
- Modify: `src/cli/run.rs`
- Modify: `tests/open_api.rs`
- Modify: `tests/cli_remote.rs`

- [ ] **Step 1: Write the failing format and CLI error-visibility tests**

```rust
// src/domain/open/formats/tests.rs
use std::path::Path;

use crate::domain::open::formats::is_supported_audio_path;

#[test]
fn supports_case_insensitive_audio_extensions() {
    assert!(is_supported_audio_path(Path::new("Always Online.FLAC")));
    assert!(is_supported_audio_path(Path::new("always-online.Mp3")));
    assert!(is_supported_audio_path(Path::new("always-online.m4a")));
    assert!(is_supported_audio_path(Path::new("always-online.AAC")));
}

#[test]
fn rejects_non_audio_extensions() {
    assert!(!is_supported_audio_path(Path::new("cover.jpg")));
}
```

```rust
// append to tests/cli_remote.rs
#[tokio::test(flavor = "multi_thread")]
async fn explicit_open_command_prints_stable_error_body() {
    let app = melo::daemon::app::test_router().await;
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let mut cmd = Command::cargo_bin("melo").unwrap();
    cmd.env("MELO_BASE_URL", format!("http://{addr}"));
    cmd.arg("cover.jpg");

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("unsupported_open_format"));
}
```

- [ ] **Step 2: Run the focused tests to verify they fail**

Run: `cargo test supports_case_insensitive_audio_extensions --lib -- --nocapture`  
Expected: FAIL because `domain::open::formats` does not exist yet and `.m4a/.aac` are not supported.

Run: `cargo test --test cli_remote explicit_open_command_prints_stable_error_body -- --nocapture`  
Expected: FAIL because the CLI still collapses daemon open failures into a generic HTTP error.

- [ ] **Step 3: Implement centralized format helpers and explicit open error propagation**

```rust
// src/domain/open/formats.rs
pub fn is_supported_audio_path(path: &std::path::Path) -> bool {
    matches!(
        path.extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.to_ascii_lowercase())
            .as_deref(),
        Some("flac" | "mp3" | "ogg" | "wav" | "m4a" | "aac")
    )
}

#[cfg(test)]
mod tests;
```

```rust
// src/domain/open/service.rs
pub fn classify_target(path: &Path) -> MeloResult<OpenTarget> {
    if path.is_dir() {
        return Ok(OpenTarget::Directory(path.to_path_buf()));
    }

    if crate::domain::open::formats::is_supported_audio_path(path) {
        return Ok(OpenTarget::AudioFile(path.to_path_buf()));
    }

    Err(MeloError::Message("unsupported_open_format".to_string()))
}
```

```rust
// src/cli/client.rs
pub async fn open_target(&self, target: String, mode: &str) -> MeloResult<OpenResponse> {
    let url = format!("{}/api/open", self.base_url);
    let response = self
        .client
        .post(url)
        .json(&serde_json::json!({ "target": target, "mode": mode }))
        .send()
        .await
        .map_err(|err| MeloError::Message(err.to_string()))?;

    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(MeloError::Message(if body.is_empty() {
            format!("open_request_failed:{status}")
        } else {
            body
        }));
    }

    serde_json::from_str(&body).map_err(|err| MeloError::Message(err.to_string()))
}
```

- [ ] **Step 4: Run the focused tests and verify they pass**

Run: `cargo test supports_case_insensitive_audio_extensions --lib -- --nocapture`  
Expected: PASS and `.FLAC`, `.Mp3`, `.m4a`, and `.AAC` are all accepted.

Run: `cargo test --test cli_remote explicit_open_command_prints_stable_error_body -- --nocapture`  
Expected: PASS and explicit `melo cover.jpg` now prints `unsupported_open_format`.

- [ ] **Step 5: Run project QA before committing**

Run: `pnpm qa`  
Expected: PASS including formatting, clippy, and the full test suite.

- [ ] **Step 6: Commit the format/error-contract slice**

```bash
git add src/domain/open/mod.rs src/domain/open/formats.rs src/domain/open/formats/tests.rs src/domain/open/service.rs src/domain/library/service.rs src/api/open.rs src/cli/client.rs src/cli/run.rs tests/open_api.rs tests/cli_remote.rs
git commit -m "feat: add broad direct-open format support"
```

---

### Task 4: Add backend factory, backend selection, and backend name propagation

**Files:**
- Create: `src/domain/player/factory.rs`
- Create: `src/domain/player/factory/tests.rs`
- Modify: `src/domain/player/backend.rs`
- Modify: `src/domain/player/mod.rs`
- Modify: `src/core/model/player.rs`
- Modify: `src/domain/player/service.rs`
- Modify: `src/domain/player/service/tests.rs`
- Modify: `src/daemon/app.rs`
- Modify: `tests/api_server.rs`

- [ ] **Step 1: Write the failing backend-selection and snapshot tests**

```rust
// src/domain/player/factory/tests.rs
use crate::core::config::settings::{MpvSettings, PlayerSettings};
use crate::domain::player::factory::{resolve_backend_choice, BackendChoice};

#[test]
fn auto_prefers_mpv_when_probe_succeeds() {
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
    assert_eq!(choice, BackendChoice::Mpv);
}

#[test]
fn auto_falls_back_to_rodio_when_mpv_missing() {
    let choice = resolve_backend_choice(&PlayerSettings::default(), || false).unwrap();
    assert_eq!(choice, BackendChoice::Rodio);
}
```

```rust
// append to src/domain/player/service/tests.rs
#[tokio::test]
async fn snapshot_includes_backend_name() {
    let backend = std::sync::Arc::new(crate::domain::player::backend::NoopBackend::default());
    let service = crate::domain::player::service::PlayerService::new(backend);

    let snapshot = service.snapshot().await;
    assert_eq!(snapshot.backend_name, "noop");
}
```

- [ ] **Step 2: Run the focused tests to verify they fail**

Run: `cargo test auto_prefers_mpv_when_probe_succeeds --lib -- --nocapture`  
Expected: FAIL because `player::factory` and `BackendChoice` do not exist yet.

Run: `cargo test snapshot_includes_backend_name --lib -- --nocapture`  
Expected: FAIL because `PlayerSnapshot` has no `backend_name` field and `PlaybackBackend` exposes no backend identity.

- [ ] **Step 3: Implement backend-choice resolution and backend name propagation**

```rust
// src/domain/player/backend.rs
pub trait PlaybackBackend: Send + Sync {
    fn backend_name(&self) -> &'static str;
    fn load_and_play(&self, path: &std::path::Path, generation: u64) -> crate::core::error::MeloResult<()>;
    fn pause(&self) -> crate::core::error::MeloResult<()>;
    fn resume(&self) -> crate::core::error::MeloResult<()>;
    fn stop(&self) -> crate::core::error::MeloResult<()>;
    fn subscribe_runtime_events(&self) -> PlaybackRuntimeReceiver;
    fn current_position(&self) -> Option<std::time::Duration>;
    fn set_volume(&self, factor: f32) -> crate::core::error::MeloResult<()>;
}

impl PlaybackBackend for NoopBackend {
    fn backend_name(&self) -> &'static str {
        "noop"
    }
    // ...existing methods...
}
```

```rust
// src/domain/player/factory.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendChoice {
    Rodio,
    Mpv,
}

pub fn resolve_backend_choice(
    settings: &crate::core::config::settings::PlayerSettings,
    mpv_available: impl Fn() -> bool,
) -> crate::core::error::MeloResult<BackendChoice> {
    match settings.backend.as_str() {
        "rodio" => Ok(BackendChoice::Rodio),
        "mpv" => {
            if mpv_available() {
                Ok(BackendChoice::Mpv)
            } else {
                Err(crate::core::error::MeloError::Message("mpv_backend_unavailable".to_string()))
            }
        }
        _ => {
            if mpv_available() {
                Ok(BackendChoice::Mpv)
            } else {
                Ok(BackendChoice::Rodio)
            }
        }
    }
}

#[cfg(test)]
mod tests;
```

```rust
// src/core/model/player.rs
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct PlayerSnapshot {
    pub backend_name: String,
    pub playback_state: String,
    // ...existing fields...
}
```

- [ ] **Step 4: Run the focused tests and verify they pass**

Run: `cargo test auto_prefers_mpv_when_probe_succeeds --lib -- --nocapture`  
Expected: PASS and `auto` resolves to `mpv` when the probe says `mpv` is available.

Run: `cargo test snapshot_includes_backend_name --lib -- --nocapture`  
Expected: PASS and player snapshots now expose the active backend identity.

- [ ] **Step 5: Run project QA before committing**

Run: `pnpm qa`  
Expected: PASS including formatting, clippy, and the full test suite.

- [ ] **Step 6: Commit the backend-factory slice**

```bash
git add src/domain/player/backend.rs src/domain/player/factory.rs src/domain/player/factory/tests.rs src/domain/player/mod.rs src/core/model/player.rs src/domain/player/service.rs src/domain/player/service/tests.rs src/daemon/app.rs tests/api_server.rs
git commit -m "feat: add backend auto-selection"
```

---

### Task 5: Implement a runnable MPV backend

**Files:**
- Create: `src/domain/player/mpv_backend.rs`
- Create: `src/domain/player/mpv_backend/tests.rs`
- Modify: `Cargo.toml`
- Modify: `src/domain/player/mod.rs`
- Modify: `src/domain/player/factory.rs`
- Modify: `src/daemon/app.rs`

- [ ] **Step 1: Write the failing MPV backend tests**

```rust
// src/domain/player/mpv_backend/tests.rs
use crate::domain::player::mpv_backend::{build_mpv_command, parse_mpv_event};
use crate::domain::player::runtime::PlaybackRuntimeEvent;

#[test]
fn build_mpv_command_includes_windows_ipc_server_argument() {
    let command = build_mpv_command(
        "C:/Tools/mpv.exe",
        "\\\\.\\pipe\\melo-mpv-test",
        &["--no-video".to_string()],
    );
    let args = command
        .get_args()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect::<Vec<_>>();

    assert!(args.iter().any(|arg| arg == "--idle=yes"));
    assert!(args.iter().any(|arg| arg == "--no-video"));
    assert!(args.iter().any(|arg| arg.contains("--input-ipc-server=\\\\.\\pipe\\melo-mpv-test")));
}

#[test]
fn parse_end_file_event_turns_into_track_end() {
    let event = parse_mpv_event(
        r#"{"event":"end-file","reason":"eof"}"#,
        7,
    )
    .unwrap();

    assert_eq!(event, Some(PlaybackRuntimeEvent::TrackEnded { generation: 7 }));
}
```

- [ ] **Step 2: Run the focused tests to verify they fail**

Run: `cargo test build_mpv_command_includes_windows_ipc_server_argument --lib -- --nocapture`  
Expected: FAIL because `mpv_backend` does not exist yet.

Run: `cargo test parse_end_file_event_turns_into_track_end --lib -- --nocapture`  
Expected: FAIL because the MPV event parser does not exist yet.

- [ ] **Step 3: Implement the MPV backend and hook it into backend selection**

```rust
// src/domain/player/mpv_backend.rs
pub fn build_mpv_command(path: &str, ipc_path: &str, extra_args: &[String]) -> std::process::Command {
    let mut command = std::process::Command::new(path);
    command.arg("--idle=yes");
    command.arg("--no-terminal");
    command.arg("--force-window=no");
    command.arg(format!("--input-ipc-server={ipc_path}"));
    for arg in extra_args {
        command.arg(arg);
    }
    command
}

pub fn parse_mpv_event(
    line: &str,
    generation: u64,
) -> crate::core::error::MeloResult<Option<crate::domain::player::runtime::PlaybackRuntimeEvent>> {
    let value: serde_json::Value =
        serde_json::from_str(line).map_err(|err| crate::core::error::MeloError::Message(err.to_string()))?;
    if value.get("event").and_then(|event| event.as_str()) == Some("end-file") {
        return Ok(Some(crate::domain::player::runtime::PlaybackRuntimeEvent::TrackEnded {
            generation,
        }));
    }
    Ok(None)
}

pub struct MpvBackend {
    // child process handle, IPC path, runtime sender, playback position cache, etc.
}

#[cfg(test)]
mod tests;
```

```rust
// src/domain/player/factory.rs
pub fn build_backend(
    settings: &crate::core::config::settings::Settings,
) -> crate::core::error::MeloResult<std::sync::Arc<dyn crate::domain::player::backend::PlaybackBackend>> {
    match resolve_backend_choice(&settings.player, || crate::domain::player::mpv_backend::mpv_exists(&settings.player.mpv.path))? {
        BackendChoice::Rodio => Ok(std::sync::Arc::new(crate::domain::player::rodio_backend::RodioBackend::new()?)),
        BackendChoice::Mpv => Ok(std::sync::Arc::new(crate::domain::player::mpv_backend::MpvBackend::new(settings.clone())?)),
    }
}
```

- [ ] **Step 4: Run the focused tests and verify they pass**

Run: `cargo test build_mpv_command_includes_windows_ipc_server_argument --lib -- --nocapture`  
Expected: PASS and the MPV child command includes the named-pipe IPC argument and required runtime flags.

Run: `cargo test parse_end_file_event_turns_into_track_end --lib -- --nocapture`  
Expected: PASS and MPV `end-file` events are translated into `PlaybackRuntimeEvent::TrackEnded`.

- [ ] **Step 5: Run project QA before committing**

Run: `pnpm qa`  
Expected: PASS including formatting, clippy, and the full test suite.

- [ ] **Step 6: Commit the MPV backend slice**

```bash
git add Cargo.toml src/domain/player/mod.rs src/domain/player/factory.rs src/domain/player/mpv_backend.rs src/domain/player/mpv_backend/tests.rs src/daemon/app.rs
git commit -m "feat: add mpv playback backend"
```

---

### Task 6: Make the TUI show queue state, help popup, and startup notices

**Files:**
- Modify: `src/tui/app.rs`
- Modify: `src/tui/run.rs`
- Modify: `src/tui/ui/content.rs`
- Modify: `src/tui/ui/layout.rs`
- Modify: `src/tui/ui/popup.rs`
- Modify: `src/cli/run.rs`
- Modify: `tests/tui_app.rs`
- Modify: `src/tui/run/tests.rs`

- [ ] **Step 1: Write the failing TUI visibility and help tests**

```rust
// append to tests/tui_app.rs
#[test]
fn question_mark_toggles_help_popup() {
    let mut app = melo::tui::app::App::new_for_test();
    assert!(!app.show_help);

    let action = app.handle_key(crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::Char('?'),
        crossterm::event::KeyModifiers::NONE,
    ));

    assert_eq!(action, Some(melo::tui::event::Action::OpenHelp));
    assert!(app.show_help);
}

#[test]
fn footer_hints_can_be_hidden() {
    let mut app = melo::tui::app::App::new_for_test();
    app.footer_hints_enabled = false;
    assert!(!app.footer_status().contains("? Help"));
}

#[test]
fn queue_panel_renders_loaded_titles() {
    let mut app = melo::tui::app::App::new_for_test();
    app.queue_titles = vec!["Blue Bird".to_string(), "Always Online".to_string()];

    let content = app.render_queue_lines();
    assert!(content.iter().any(|line| line.contains("Blue Bird")));
    assert!(content.iter().any(|line| line.contains("Always Online")));
}
```

```rust
// append to src/tui/run/tests.rs
#[test]
fn startup_notice_is_included_in_status_line() {
    let mut app = crate::tui::app::App::new_for_test();
    app.startup_notice = Some("open_scan_failed".to_string());

    assert!(app.footer_status().contains("open_scan_failed"));
}
```

- [ ] **Step 2: Run the focused tests to verify they fail**

Run: `cargo test question_mark_toggles_help_popup --test tui_app -- --nocapture`  
Expected: FAIL because the app does not track `show_help` or queue lines yet.

Run: `cargo test startup_notice_is_included_in_status_line --lib -- --nocapture`  
Expected: FAIL because the TUI app has no launch notice field.

- [ ] **Step 3: Implement queue rendering, help overlay state, and launch-context plumbing**

```rust
// src/tui/app.rs
pub struct App {
    pub player: PlayerSnapshot,
    pub active_view: ActiveView,
    pub focus: FocusArea,
    pub source_label: Option<String>,
    pub startup_notice: Option<String>,
    pub footer_hints_enabled: bool,
    pub show_help: bool,
    pub queue_titles: Vec<String>,
}

impl App {
    pub fn render_queue_lines(&self) -> Vec<String> {
        if self.queue_titles.is_empty() {
            return vec!["No tracks loaded".to_string()];
        }
        self.queue_titles
            .iter()
            .enumerate()
            .map(|(index, title)| {
                if self.player.queue_index == Some(index) {
                    format!("> {title}")
                } else {
                    format!("  {title}")
                }
            })
            .collect()
    }
}
```

```rust
// src/tui/ui/popup.rs
pub fn help_lines() -> Vec<&'static str> {
    vec![
        "Playback",
        "Space: Play/Pause",
        ">: Next",
        "<: Previous",
        "General",
        "?: Toggle Help",
        "q: Close Help / Quit",
    ]
}
```

```rust
// src/tui/run.rs
pub struct LaunchContext {
    pub source_label: Option<String>,
    pub startup_notice: Option<String>,
    pub footer_hints_enabled: bool,
}

pub async fn start(base_url: String, context: LaunchContext) -> MeloResult<()> {
    // initialize terminal...
    let mut app = crate::tui::app::App::new_for_test();
    app.source_label = context.source_label;
    app.startup_notice = context.startup_notice;
    app.footer_hints_enabled = context.footer_hints_enabled;
    // refresh snapshot and queue_titles before draw...
}
```

- [ ] **Step 4: Run the focused tests and verify they pass**

Run: `cargo test question_mark_toggles_help_popup --test tui_app -- --nocapture`  
Expected: PASS and `?` toggles a real help popup state.

Run: `cargo test startup_notice_is_included_in_status_line --lib -- --nocapture`  
Expected: PASS and startup/open failures are visible from the footer status line.

- [ ] **Step 5: Run project QA before committing**

Run: `pnpm qa`  
Expected: PASS including formatting, clippy, and the full test suite.

- [ ] **Step 6: Commit the TUI visibility/help slice**

```bash
git add src/tui/app.rs src/tui/run.rs src/tui/ui/content.rs src/tui/ui/layout.rs src/tui/ui/popup.rs src/cli/run.rs tests/tui_app.rs src/tui/run/tests.rs
git commit -m "feat: add queue-aware tui help overlay"
```

---

## Self-review notes

### Spec coverage

- daemon 注册、首选端口与自动避让：Task 1 + Task 2
- `MELO_BASE_URL` 仅作覆盖、统一 daemon 发现：Task 2
- 大小写不敏感扩展名与 `m4a/aac`：Task 3
- direct-open 错误正文与显式路径失败：Task 3
- `PlaybackBackend` 正式化、`auto` 解析与 backend_name：Task 4
- 可运行 `mpv` 后端：Task 5
- TUI queue 列表、帮助弹层、底部提示开关、启动错误提示：Task 6
- `config.example.toml` 新增 daemon/player/tui 域：Task 1

### Placeholder scan

- 没有使用待补实现、占位标记或“后续再补代码”之类空泛语句
- 每个任务都给出了实际文件路径、测试代码、命令和期望结果
- 没有写“按需处理错误”这种抽象步骤，错误契约已经在 Task 3 中具体化

### Type consistency

- daemon 注册统一使用 `DaemonRegistration`
- 统一后端选择类型使用 `BackendChoice`
- 广格式判断统一入口使用 `domain::open::formats::is_supported_audio_path`
- TUI 启动上下文统一使用 `LaunchContext`

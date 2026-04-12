# Melo Startup Verbose and Unified Logging Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a unified logging system for CLI and daemon, make `melo --verbose` expose startup-chain progress and daemon logs in the current terminal, and fix global logging flags so they no longer get misclassified as direct-open paths.

**Architecture:** Introduce a central `core::logging` module that resolves config + CLI overrides into component-specific logging options, initializes pretty terminal output plus JSON file output, and provides a file-follow helper for attaching daemon logs to the active CLI terminal. Add a lightweight global-flag pre-parser before dispatch so startup commands can honor `--verbose`, `--log-level`, `--no-log-prefix`, and `--daemon-log-level` without breaking existing Clap or direct-open flows. Keep daemon file logs as the source of truth and only allow daemon log-level escalation for daemons launched by the current command in this phase.

**Tech Stack:** Rust 2024, `tracing`, `tracing-subscriber` with `env-filter` + `json`, Tokio, Clap 4, existing daemon manager/process helpers, `assert_cmd`, `tempfile`, `serde_json`, `pnpm qa`

---

## File Structure

### Logging config and runtime resolution

- Create: `src/core/logging.rs`
  - Responsibility: logging enums, resolved component options, tracing initialization, prefix formatting, JSON file output setup, daemon log follow helper.
- Create: `src/core/logging/tests.rs`
  - Responsibility: unit tests for override precedence, prefix behavior, JSON file output validity, daemon runtime-override gating.
- Modify: `src/core/mod.rs`
  - Responsibility: export the logging module.
- Modify: `src/core/config/settings.rs`
  - Responsibility: add `[logging]`, `[logging.cli]`, `[logging.daemon]` config structs, defaults, and config builder defaults.
- Modify: `config.example.toml`
  - Responsibility: document logging config and new startup verbose controls.
- Modify: `tests/config_loading.rs`
  - Responsibility: integration coverage for logging config parsing.
- Modify: `Cargo.toml`
  - Responsibility: enable `tracing-subscriber` JSON formatting support.

### Global CLI logging flags and dispatch integration

- Create: `src/cli/global_flags.rs`
  - Responsibility: pre-parse global logging flags from raw argv before dispatch/Clap and return sanitized argv plus CLI logging overrides.
- Create: `src/cli/global_flags/tests.rs`
  - Responsibility: unit tests for stripping logging flags without breaking default launch, direct-open, or Clap flows.
- Modify: `src/cli/mod.rs`
  - Responsibility: export the new global-flag parser module.
- Modify: `src/cli/args.rs`
  - Responsibility: expose global logging flags in top-level CLI help via Clap.
- Modify: `src/cli/dispatch.rs`
  - Responsibility: consume sanitized argv so `--verbose` and friends are never mistaken for paths.
- Modify: `src/cli/dispatch/tests.rs`
  - Responsibility: keep dispatch expectations aligned with the new pre-parser.
- Modify: `tests/cli_help.rs`
  - Responsibility: help output coverage for global logging flags.

### Unified tracing bootstrap and runtime propagation

- Modify: `src/main.rs`
  - Responsibility: pre-parse logging flags before tracing init, load settings early, initialize unified tracing, and call CLI execution with prepared args + logging overrides.
- Modify: `src/cli/run.rs`
  - Responsibility: accept prepared argv/logging context instead of reparsing raw args in isolation.
- Modify: `src/daemon/process.rs`
  - Responsibility: allow auto-started daemon child processes to inherit command-scoped daemon log-level override env vars.
- Modify: `src/daemon/process/tests.rs`
  - Responsibility: unit coverage for daemon child command env propagation.
- Modify: `tests/cli_remote.rs`
  - Responsibility: integration coverage that `daemon run` writes JSON daemon logs and startup commands expose verbose stage logs.

### Verbose startup stages and daemon log mirroring

- Modify: `src/cli/run.rs`
  - Responsibility: emit stage logs for default launch, explicit direct-open, TUI connection, and failures; support prefix suppression.
- Modify: `src/daemon/manager.rs`
  - Responsibility: add a cancellable helper that tails daemon log file from a starting offset into the current CLI terminal without blocking the command result.
- Modify: `src/core/logging.rs`
  - Responsibility: provide daemon log mirror lifecycle helpers and prefix-aware terminal line rendering.
- Modify: `tests/cli_remote.rs`
  - Responsibility: integration tests for `--verbose` startup stages, prefix suppression, and daemon log excerpt display on autostart failure.

### Docs and command-surface updates

- Modify: `README.md`
  - Responsibility: document the new global logging flags and unified logging behavior once implemented.
- Modify: `config.example.toml`
  - Responsibility: keep user-facing logging examples in sync with the implementation.

## Task 1: Add Logging Config and Resolved Logging Options

**Files:**
- Create: `src/core/logging.rs`
- Create: `src/core/logging/tests.rs`
- Modify: `src/core/mod.rs`
- Modify: `src/core/config/settings.rs`
- Modify: `Cargo.toml`
- Modify: `config.example.toml`
- Test: `tests/config_loading.rs`

- [ ] **Step 1: Write the failing config-loading and override-resolution tests**

Append this test to `tests/config_loading.rs`:

```rust
#[test]
fn settings_load_logging_defaults_and_component_overrides() {
    let temp = tempdir().unwrap();
    let path = temp.path().join("config.toml");
    fs::write(
        &path,
        r#"
[database]
path = "local/melo.db"

[logging]
level = "warning"
terminal_format = "pretty"
file_format = "json"
prefix_enabled = false
cli_prefix = "term"
daemon_prefix = "svc"

[logging.cli]
file_enabled = true
file_path = "logs/cli.log"

[logging.daemon]
file_enabled = true
file_path = "logs/daemon.log"
allow_runtime_level_override = true
"#,
    )
    .unwrap();

    let settings = Settings::load_from_path(&path).unwrap();

    assert_eq!(settings.logging.level.as_str(), "warning");
    assert_eq!(settings.logging.terminal_format.as_str(), "pretty");
    assert_eq!(settings.logging.file_format.as_str(), "json");
    assert!(!settings.logging.prefix_enabled);
    assert_eq!(settings.logging.cli_prefix, "term");
    assert_eq!(settings.logging.daemon_prefix, "svc");
    assert!(settings.logging.cli.file_enabled);
    assert_eq!(
        settings.logging.cli.file_path.as_deref(),
        Some("logs/cli.log")
    );
    assert!(settings.logging.daemon.file_enabled);
    assert_eq!(
        settings.logging.daemon.file_path.as_deref(),
        Some("logs/daemon.log")
    );
    assert!(settings.logging.daemon.allow_runtime_level_override);
}
```

Create `src/core/logging/tests.rs`:

```rust
use crate::core::config::settings::{LoggingLevel, Settings};
use crate::core::logging::{CliLogOverrides, LogComponent, resolve_logging_options};

#[test]
fn resolve_logging_options_prefers_cli_override_then_component_then_global() {
    let mut settings = Settings::default();
    settings.logging.level = LoggingLevel::Warning;
    settings.logging.prefix_enabled = true;
    settings.logging.cli.level = Some(LoggingLevel::Info);
    settings.logging.cli.prefix_enabled = Some(false);

    let resolved = resolve_logging_options(
        &settings,
        LogComponent::Cli,
        &CliLogOverrides {
            verbose: false,
            log_level: Some(LoggingLevel::Trace),
            no_log_prefix: false,
            daemon_log_level: None,
        },
    );

    assert_eq!(resolved.level, LoggingLevel::Trace);
    assert!(!resolved.prefix_enabled);
}

#[test]
fn resolve_logging_options_blocks_daemon_runtime_override_when_component_disables_it() {
    let mut settings = Settings::default();
    settings.logging.level = LoggingLevel::Warning;
    settings.logging.daemon.level = Some(LoggingLevel::Warning);
    settings.logging.daemon.allow_runtime_level_override = false;

    let resolved = resolve_logging_options(
        &settings,
        LogComponent::Daemon,
        &CliLogOverrides {
            verbose: false,
            log_level: None,
            no_log_prefix: false,
            daemon_log_level: Some(LoggingLevel::Trace),
        },
    );

    assert_eq!(resolved.level, LoggingLevel::Warning);
    assert!(resolved.daemon_runtime_override_blocked);
}
```

- [ ] **Step 2: Run the focused tests to confirm they fail**

Run:

```bash
rtk cargo test -q --test config_loading settings_load_logging_defaults_and_component_overrides
rtk cargo test -q resolve_logging_options_prefers_cli_override_then_component_then_global --lib
rtk cargo test -q resolve_logging_options_blocks_daemon_runtime_override_when_component_disables_it --lib
```

Expected:

- `config_loading` fails because `Settings` has no `logging` field.
- `--lib` fails because `core::logging` module and its types do not exist yet.

- [ ] **Step 3: Implement the logging config model, defaults, and resolved options**

Update `Cargo.toml`:

```toml
tracing-subscriber = { version = "0.3.23", features = ["env-filter", "json"] }
```

Add the logging enums and config structs to `src/core/config/settings.rs`:

```rust
#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum LoggingLevel {
    Error,
    #[default]
    Warning,
    Info,
    Debug,
    Trace,
}

impl LoggingLevel {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warning => "warning",
            Self::Info => "info",
            Self::Debug => "debug",
            Self::Trace => "trace",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum LoggingFormat {
    #[default]
    Pretty,
    Json,
}

impl LoggingFormat {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pretty => "pretty",
            Self::Json => "json",
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct LoggingComponentSettings {
    pub level: Option<LoggingLevel>,
    pub file_enabled: bool,
    pub file_path: Option<String>,
    pub prefix_enabled: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct DaemonLoggingSettings {
    pub level: Option<LoggingLevel>,
    pub file_enabled: bool,
    pub file_path: Option<String>,
    pub prefix_enabled: Option<bool>,
    pub allow_runtime_level_override: bool,
}

impl Default for DaemonLoggingSettings {
    fn default() -> Self {
        Self {
            level: None,
            file_enabled: true,
            file_path: Some("logs/daemon.log".to_string()),
            prefix_enabled: None,
            allow_runtime_level_override: true,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct LoggingSettings {
    pub level: LoggingLevel,
    pub terminal_format: LoggingFormat,
    pub file_format: LoggingFormat,
    pub prefix_enabled: bool,
    pub cli_prefix: String,
    pub daemon_prefix: String,
    pub cli: LoggingComponentSettings,
    pub daemon: DaemonLoggingSettings,
}
```

Create `src/core/logging.rs`:

```rust
use crate::core::config::settings::{LoggingLevel, Settings};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogComponent {
    Cli,
    Daemon,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CliLogOverrides {
    pub verbose: bool,
    pub log_level: Option<LoggingLevel>,
    pub no_log_prefix: bool,
    pub daemon_log_level: Option<LoggingLevel>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedLoggingOptions {
    pub level: LoggingLevel,
    pub prefix_enabled: bool,
    pub prefix_text: String,
    pub file_enabled: bool,
    pub file_path: Option<String>,
    pub daemon_runtime_override_blocked: bool,
}
```

Continue `src/core/logging.rs` with the resolver:

```rust
pub fn resolve_logging_options(
    settings: &Settings,
    component: LogComponent,
    overrides: &CliLogOverrides,
) -> ResolvedLoggingOptions {
    match component {
        LogComponent::Cli => {
            let level = overrides
                .log_level
                .or(settings.logging.cli.level)
                .unwrap_or(settings.logging.level);
            let prefix_enabled = if overrides.no_log_prefix {
                false
            } else {
                settings
                    .logging
                    .cli
                    .prefix_enabled
                    .unwrap_or(settings.logging.prefix_enabled)
            };
            ResolvedLoggingOptions {
                level,
                prefix_enabled,
                prefix_text: settings.logging.cli_prefix.clone(),
                file_enabled: settings.logging.cli.file_enabled,
                file_path: settings.logging.cli.file_path.clone(),
                daemon_runtime_override_blocked: false,
            }
        }
        LogComponent::Daemon => {
            let blocked = overrides.daemon_log_level.is_some()
                && !settings.logging.daemon.allow_runtime_level_override;
            let level = if blocked {
                settings
                    .logging
                    .daemon
                    .level
                    .unwrap_or(settings.logging.level)
            } else {
                overrides
                    .daemon_log_level
                    .or(settings.logging.daemon.level)
                    .unwrap_or(settings.logging.level)
            };
            ResolvedLoggingOptions {
                level,
                prefix_enabled: settings
                    .logging
                    .daemon
                    .prefix_enabled
                    .unwrap_or(settings.logging.prefix_enabled),
                prefix_text: settings.logging.daemon_prefix.clone(),
                file_enabled: settings.logging.daemon.file_enabled,
                file_path: settings.logging.daemon.file_path.clone(),
                daemon_runtime_override_blocked: blocked,
            }
        }
    }
}

#[cfg(test)]
mod tests;
```

Export the module from `src/core/mod.rs` and document the config block in `config.example.toml`.

- [ ] **Step 4: Run the focused tests to verify the config and resolver pass**

Run:

```bash
rtk cargo test -q --test config_loading settings_load_logging_defaults_and_component_overrides
rtk cargo test -q resolve_logging_options_prefers_cli_override_then_component_then_global --lib
rtk cargo test -q resolve_logging_options_blocks_daemon_runtime_override_when_component_disables_it --lib
```

Expected: all three commands PASS.

- [ ] **Step 5: Commit the config and resolver slice**

```bash
rtk git add Cargo.toml src/core/logging.rs src/core/logging/tests.rs src/core/mod.rs src/core/config/settings.rs config.example.toml tests/config_loading.rs
rtk git commit -m "feat(logging): add unified logging config and resolver"
```

## Task 2: Parse Global Logging Flags Before Dispatch and Surface Them in Help

**Files:**
- Create: `src/cli/global_flags.rs`
- Create: `src/cli/global_flags/tests.rs`
- Modify: `src/cli/mod.rs`
- Modify: `src/cli/args.rs`
- Modify: `src/cli/dispatch.rs`
- Modify: `src/cli/dispatch/tests.rs`
- Modify: `tests/cli_help.rs`

- [ ] **Step 1: Write the failing pre-parser and help tests**

Create `src/cli/global_flags/tests.rs`:

```rust
use std::ffi::OsString;

use crate::cli::dispatch::{Dispatch, dispatch_args};
use crate::cli::global_flags::prepare_args;
use crate::core::config::settings::LoggingLevel;

#[test]
fn prepare_args_keeps_default_launch_when_only_verbose_is_present() {
    let prepared = prepare_args(&[OsString::from("melo"), OsString::from("--verbose")]).unwrap();

    assert!(prepared.logging.verbose);
    assert_eq!(prepared.dispatch_args, vec![OsString::from("melo")]);
    assert_eq!(dispatch_args(&prepared.dispatch_args), Dispatch::DefaultLaunch);
}

#[test]
fn prepare_args_preserves_direct_open_target_after_logging_flags() {
    let prepared = prepare_args(&[
        OsString::from("melo"),
        OsString::from("--verbose"),
        OsString::from("D:/Music"),
    ])
    .unwrap();

    assert_eq!(
        dispatch_args(&prepared.dispatch_args),
        Dispatch::DirectOpen("D:/Music".to_string())
    );
}

#[test]
fn prepare_args_extracts_explicit_levels_and_prefix_toggle() {
    let prepared = prepare_args(&[
        OsString::from("melo"),
        OsString::from("--log-level"),
        OsString::from("debug"),
        OsString::from("--daemon-log-level"),
        OsString::from("trace"),
        OsString::from("--no-log-prefix"),
        OsString::from("status"),
    ])
    .unwrap();

    assert_eq!(prepared.logging.log_level, Some(LoggingLevel::Debug));
    assert_eq!(prepared.logging.daemon_log_level, Some(LoggingLevel::Trace));
    assert!(prepared.logging.no_log_prefix);
    assert_eq!(
        prepared.clap_args,
        vec![OsString::from("melo"), OsString::from("status")]
    );
}
```

Append this test to `tests/cli_help.rs`:

```rust
#[test]
fn root_help_lists_global_logging_flags() {
    let mut cmd = Command::cargo_bin("melo").unwrap();
    cmd.arg("--help");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("--verbose"))
        .stdout(predicate::str::contains("--log-level"))
        .stdout(predicate::str::contains("--no-log-prefix"))
        .stdout(predicate::str::contains("--daemon-log-level"));
}
```

- [ ] **Step 2: Run the focused tests to confirm they fail**

Run:

```bash
rtk cargo test -q prepare_args_keeps_default_launch_when_only_verbose_is_present --lib
rtk cargo test -q prepare_args_preserves_direct_open_target_after_logging_flags --lib
rtk cargo test -q --test cli_help root_help_lists_global_logging_flags
```

Expected:

- `--lib` fails because `cli::global_flags` and `prepare_args(...)` do not exist.
- `cli_help` fails because root help does not expose the new logging flags.

- [ ] **Step 3: Implement the global logging pre-parser and top-level Clap flags**

Create `src/cli/global_flags.rs`:

```rust
use std::ffi::OsString;

use crate::core::config::settings::LoggingLevel;
use crate::core::error::{MeloError, MeloResult};
use crate::core::logging::CliLogOverrides;

#[derive(Debug, Clone)]
pub struct PreparedArgs {
    pub clap_args: Vec<OsString>,
    pub dispatch_args: Vec<OsString>,
    pub logging: CliLogOverrides,
}
```

Continue `src/cli/global_flags.rs` with the parser:

```rust
pub fn prepare_args(raw_args: &[OsString]) -> MeloResult<PreparedArgs> {
    let mut clap_args = Vec::with_capacity(raw_args.len());
    let mut dispatch_args = Vec::with_capacity(raw_args.len());
    let mut logging = CliLogOverrides::default();

    if let Some(program) = raw_args.first() {
        clap_args.push(program.clone());
        dispatch_args.push(program.clone());
    }

    let mut index = 1usize;
    while index < raw_args.len() {
        let Some(current) = raw_args[index].to_str() else {
            clap_args.push(raw_args[index].clone());
            dispatch_args.push(raw_args[index].clone());
            index += 1;
            continue;
        };

        match current {
            "--verbose" => {
                logging.verbose = true;
                index += 1;
            }
            "--no-log-prefix" => {
                logging.no_log_prefix = true;
                index += 1;
            }
            "--log-level" => {
                let value = raw_args
                    .get(index + 1)
                    .and_then(|item| item.to_str())
                    .ok_or_else(|| MeloError::Message("missing_log_level_value".to_string()))?;
                logging.log_level = Some(parse_level(value)?);
                index += 2;
            }
            "--daemon-log-level" => {
                let value = raw_args
                    .get(index + 1)
                    .and_then(|item| item.to_str())
                    .ok_or_else(|| MeloError::Message("missing_daemon_log_level_value".to_string()))?;
                logging.daemon_log_level = Some(parse_level(value)?);
                index += 2;
            }
            _ => {
                clap_args.push(raw_args[index].clone());
                dispatch_args.push(raw_args[index].clone());
                index += 1;
            }
        }
    }

    Ok(PreparedArgs {
        clap_args,
        dispatch_args,
        logging,
    })
}

fn parse_level(value: &str) -> MeloResult<LoggingLevel> {
    match value {
        "error" => Ok(LoggingLevel::Error),
        "warning" => Ok(LoggingLevel::Warning),
        "info" => Ok(LoggingLevel::Info),
        "debug" => Ok(LoggingLevel::Debug),
        "trace" => Ok(LoggingLevel::Trace),
        _ => Err(MeloError::Message(format!("unsupported_log_level:{value}"))),
    }
}
```

Add global logging flags to `src/cli/args.rs`:

```rust
use clap::{Args, Parser, Subcommand};

#[derive(Debug, Clone, Args, Default)]
pub struct GlobalLogArgs {
    #[arg(long, global = true)]
    pub verbose: bool,
    #[arg(long, global = true)]
    pub log_level: Option<String>,
    #[arg(long, global = true)]
    pub no_log_prefix: bool,
    #[arg(long, global = true)]
    pub daemon_log_level: Option<String>,
}

#[derive(Debug, Parser)]
pub struct CliArgs {
    #[command(flatten)]
    pub logging: GlobalLogArgs,
    #[command(subcommand)]
    pub command: Option<Command>,
}
```

Export `global_flags` from `src/cli/mod.rs`, and make later callers pass `prepared.dispatch_args` into `dispatch_args(...)`.

- [ ] **Step 4: Run the focused tests to verify the parser and help output pass**

Run:

```bash
rtk cargo test -q prepare_args_keeps_default_launch_when_only_verbose_is_present --lib
rtk cargo test -q prepare_args_extracts_explicit_levels_and_prefix_toggle --lib
rtk cargo test -q --test cli_help root_help_lists_global_logging_flags
```

Expected: all three commands PASS.

- [ ] **Step 5: Commit the global-flag parsing slice**

```bash
rtk git add src/cli/global_flags.rs src/cli/global_flags/tests.rs src/cli/mod.rs src/cli/args.rs src/cli/dispatch/tests.rs tests/cli_help.rs
rtk git commit -m "feat(cli): parse global logging flags before dispatch"
```

## Task 3: Initialize Unified Tracing With Pretty Terminal Output and JSON File Output

**Files:**
- Modify: `Cargo.toml`
- Modify: `src/core/logging.rs`
- Modify: `src/core/logging/tests.rs`
- Modify: `src/main.rs`
- Modify: `src/cli/run.rs`
- Modify: `src/daemon/process.rs`
- Modify: `src/daemon/process/tests.rs`
- Modify: `tests/cli_remote.rs`

- [ ] **Step 1: Write the failing tracing-bootstrap tests**

Append this test to `src/core/logging/tests.rs`:

```rust
use std::sync::{Arc, Mutex};

use crate::core::config::settings::{LoggingFormat, LoggingLevel, Settings};
use crate::core::logging::{
    CliLogOverrides, LogComponent, RuntimeLogContext, init_tracing_for_test,
};

#[test]
fn init_tracing_for_test_writes_json_file_log_with_component_field() {
    let temp = tempfile::tempdir().unwrap();
    let mut settings = Settings::default();
    settings.logging.level = LoggingLevel::Info;
    settings.logging.file_format = LoggingFormat::Json;
    settings.logging.cli.file_enabled = true;
    settings.logging.cli.file_path = Some(
        temp.path()
            .join("cli.log")
            .to_string_lossy()
            .to_string(),
    );

    let terminal = Arc::new(Mutex::new(Vec::new()));
    let guard = init_tracing_for_test(
        &settings,
        LogComponent::Cli,
        &CliLogOverrides::default(),
        RuntimeLogContext {
            session_id: "session-1".into(),
            command_id: "command-1".into(),
        },
        terminal,
    );

    tracing::info!(target: "melo::tests", component = "cli", "boot");
    drop(guard);

    let contents = std::fs::read_to_string(temp.path().join("cli.log")).unwrap();
    let line = contents.lines().last().unwrap();
    let json: serde_json::Value = serde_json::from_str(line).unwrap();
    assert_eq!(json["component"], "cli");
    assert_eq!(json["level"], "INFO");
    assert_eq!(json["fields"]["message"], "boot");
}
```

Append this integration test to `tests/cli_remote.rs`:

```rust
#[test]
fn daemon_run_writes_json_logs_to_daemon_file() {
    let temp = tempfile::tempdir().unwrap();
    let state_file = temp.path().join("daemon.json");
    let config_path = temp.path().join("config.toml");
    std::fs::write(
        &config_path,
        r#"
[database]
path = "bad<>/melo.db"

[logging]
level = "info"
file_format = "json"

[logging.daemon]
file_enabled = true
file_path = "daemon.log"
"#,
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("melo").unwrap();
    cmd.env("MELO_CONFIG_PATH", &config_path);
    cmd.env("MELO_DAEMON_STATE_FILE", &state_file);
    cmd.arg("daemon").arg("run");
    cmd.assert().failure();

    let contents = std::fs::read_to_string(temp.path().join("daemon.log")).unwrap();
    let line = contents.lines().last().unwrap();
    let json: serde_json::Value = serde_json::from_str(line).unwrap();
    assert_eq!(json["component"], "daemon");
}
```

- [ ] **Step 2: Run the focused tests to confirm they fail**

Run:

```bash
rtk cargo test -q init_tracing_for_test_writes_json_file_log_with_component_field --lib
rtk cargo test -q --test cli_remote daemon_run_writes_json_logs_to_daemon_file
```

Expected:

- the unit test fails because `init_tracing_for_test(...)` does not exist.
- the integration test fails because daemon logs are still written in the old ad-hoc format.

- [ ] **Step 3: Implement unified tracing bootstrap and runtime context wiring**

Expand `src/core/logging.rs` with runtime init support:

```rust
use std::fs::OpenOptions;
use std::sync::{Arc, Mutex};

use tracing_subscriber::fmt::writer::BoxMakeWriter;
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Debug, Clone)]
pub struct RuntimeLogContext {
    pub session_id: String,
    pub command_id: String,
}

pub fn env_filter_for(level: LoggingLevel) -> EnvFilter {
    EnvFilter::new(level.as_str())
}

pub fn init_tracing(
    settings: &Settings,
    component: LogComponent,
    overrides: &CliLogOverrides,
    context: RuntimeLogContext,
) {
    let resolved = resolve_logging_options(settings, component, overrides);
    let terminal_layer = tracing_subscriber::fmt::layer()
        .with_target(false)
        .with_ansi(true)
        .with_writer(std::io::stderr)
        .event_format(build_terminal_format(&resolved, &context, component));

    let subscriber = tracing_subscriber::registry()
        .with(env_filter_for(resolved.level))
        .with(terminal_layer);

    if resolved.file_enabled {
        if let Some(path) = resolve_log_file_path(settings, &resolved.file_path) {
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if let Ok(file) = OpenOptions::new().create(true).append(true).open(&path) {
                let file_layer = tracing_subscriber::fmt::layer()
                    .json()
                    .with_writer(BoxMakeWriter::new(file))
                    .with_target(true)
                    .with_current_span(false)
                    .with_span_list(false);
                subscriber.with(file_layer).init();
                return;
            }
        }
    }

    subscriber.init();
}

fn resolve_log_file_path(settings: &Settings, configured: &Option<String>) -> Option<std::path::PathBuf> {
    configured.as_ref().map(|value| {
        let config_path = std::env::var("MELO_CONFIG_PATH")
            .or_else(|_| std::env::var("MELO_CONFIG"))
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|_| crate::core::config::paths::default_config_path());
        crate::core::config::paths::resolve_from_config_dir(&config_path, std::path::Path::new(value))
    })
}

fn build_terminal_format(
    _resolved: &ResolvedLoggingOptions,
    _context: &RuntimeLogContext,
    _component: LogComponent,
) -> tracing_subscriber::fmt::format::Format<tracing_subscriber::fmt::format::Compact> {
    tracing_subscriber::fmt::format().compact()
}
```

Add a test-only dispatcher helper to `src/core/logging.rs`:

```rust
pub fn init_tracing_for_test(
    settings: &Settings,
    component: LogComponent,
    overrides: &CliLogOverrides,
    context: RuntimeLogContext,
    terminal_sink: Arc<Mutex<Vec<u8>>>,
) -> tracing::dispatcher::DefaultGuard {
    let resolved = resolve_logging_options(settings, component, overrides);
    let terminal_layer = tracing_subscriber::fmt::layer()
        .with_target(false)
        .with_ansi(false)
        .with_writer(BoxMakeWriter::new(TestWriter(terminal_sink)))
        .event_format(build_terminal_format(&resolved, &context, component));
    let subscriber = tracing_subscriber::registry()
        .with(env_filter_for(resolved.level))
        .with(terminal_layer);
    let dispatch = tracing::Dispatch::new(subscriber);
    tracing::dispatcher::set_default(&dispatch)
}

struct TestWriter(Arc<Mutex<Vec<u8>>>);
```

Modify `src/main.rs` to pre-parse args before tracing init:

```rust
#[tokio::main]
async fn main() {
    let raw_args = std::env::args_os().collect::<Vec<_>>();
    let prepared = melo::cli::global_flags::prepare_args(&raw_args).unwrap_or_else(|err| {
        eprintln!("{err}");
        std::process::exit(1);
    });
    let settings = melo::core::config::settings::Settings::load().unwrap_or_default();

    let component = if matches!(prepared.clap_args.get(1).and_then(|arg| arg.to_str()), Some("daemon"))
        && matches!(prepared.clap_args.get(2).and_then(|arg| arg.to_str()), Some("run"))
    {
        melo::core::logging::LogComponent::Daemon
    } else {
        melo::core::logging::LogComponent::Cli
    };

    melo::core::logging::init_tracing(
        &settings,
        component,
        &prepared.logging,
        melo::core::logging::RuntimeLogContext {
            session_id: uuid::Uuid::new_v4().to_string(),
            command_id: uuid::Uuid::new_v4().to_string(),
        },
    );

    if let Err(err) = melo::cli::run::run_prepared(prepared).await {
        eprintln!("{err}");
        std::process::exit(1);
    }
}
```

Update `src/cli/run.rs` to expose the new entrypoint:

```rust
pub async fn run() -> MeloResult<()> {
    let raw_args = std::env::args_os().collect::<Vec<_>>();
    let prepared = crate::cli::global_flags::prepare_args(&raw_args)?;
    run_prepared(prepared).await
}
```

Update `src/daemon/process.rs` to prepare child env propagation:

```rust
pub struct DaemonLaunchOverrides {
    pub daemon_log_level: Option<String>,
    pub command_id: Option<String>,
}

pub fn daemon_command(current_exe: PathBuf, overrides: &DaemonLaunchOverrides) -> Command {
    let mut command = Command::new(current_exe);
    command.arg("daemon").arg("run");
    if let Some(level) = &overrides.daemon_log_level {
        command.env("MELO_DAEMON_LOG_LEVEL_OVERRIDE", level);
    }
    if let Some(command_id) = &overrides.command_id {
        command.env("MELO_COMMAND_ID", command_id);
    }
    command.stdin(Stdio::null());
    command.stdout(Stdio::null());
    command.stderr(Stdio::null());
    command
}
```

- [ ] **Step 4: Run the focused tests to verify the new tracing bootstrap works**

Run:

```bash
rtk cargo test -q init_tracing_for_test_writes_json_file_log_with_component_field --lib
rtk cargo test -q --test cli_remote daemon_run_writes_json_logs_to_daemon_file
```

Expected: both commands PASS.

- [ ] **Step 5: Commit the unified tracing bootstrap slice**

```bash
rtk git add Cargo.toml src/core/logging.rs src/core/logging/tests.rs src/main.rs src/daemon/process.rs src/daemon/process/tests.rs tests/cli_remote.rs
rtk git commit -m "feat(logging): bootstrap unified tracing for cli and daemon"
```

## Task 4: Emit Verbose Startup Stages and Mirror Daemon Logs Into the Active Terminal

**Files:**
- Modify: `src/cli/run.rs`
- Modify: `src/daemon/manager.rs`
- Modify: `src/core/logging.rs`
- Modify: `tests/cli_remote.rs`

- [ ] **Step 1: Write the failing verbose startup-chain integration tests**

Append these tests to `tests/cli_remote.rs`:

```rust
#[tokio::test(flavor = "multi_thread")]
async fn verbose_explicit_open_prints_stage_logs_before_business_error() {
    let app = melo::daemon::app::test_router().await;
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let mut cmd = Command::cargo_bin("melo").unwrap();
    cmd.env("MELO_BASE_URL", format!("http://{addr}"));
    cmd.arg("--verbose").arg("cover.jpg");

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("[cli]"))
        .stderr(predicate::str::contains("loading_settings"))
        .stderr(predicate::str::contains("opening_explicit_target"));
}

#[test]
fn verbose_default_launch_prints_daemon_prepare_failure_excerpt() {
    let temp = tempfile::tempdir().unwrap();
    let state_file = temp.path().join("daemon.json");
    let config_path = temp.path().join("config.toml");
    std::fs::write(
        &config_path,
        r#"
[database]
path = "bad<>/melo.db"

[open]
scan_current_dir = false
"#,
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("melo").unwrap();
    cmd.env("MELO_CONFIG_PATH", &config_path);
    cmd.env("MELO_DAEMON_STATE_FILE", &state_file);
    cmd.arg("--verbose");

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("[cli]"))
        .stderr(predicate::str::contains("starting_daemon"))
        .stderr(predicate::str::contains("[daemon]"))
        .stderr(predicate::str::contains("failed to prepare database"));
}

#[tokio::test(flavor = "multi_thread")]
async fn verbose_flag_can_disable_terminal_prefixes() {
    let app = melo::daemon::app::test_router().await;
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let mut cmd = Command::cargo_bin("melo").unwrap();
    cmd.env("MELO_BASE_URL", format!("http://{addr}"));
    cmd.arg("--verbose").arg("--no-log-prefix").arg("cover.jpg");

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("opening_explicit_target"))
        .stderr(predicate::str::contains("[cli]").not())
        .stderr(predicate::str::contains("[daemon]").not());
}
```

- [ ] **Step 2: Run the focused tests to confirm they fail**

Run:

```bash
rtk cargo test -q --test cli_remote verbose_explicit_open_prints_stage_logs_before_business_error
rtk cargo test -q --test cli_remote verbose_default_launch_prints_daemon_prepare_failure_excerpt
rtk cargo test -q --test cli_remote verbose_flag_can_disable_terminal_prefixes
```

Expected:

- the tests fail because startup stages are not emitted yet.
- the daemon log excerpt test fails because CLI does not mirror `daemon.log` into the current terminal.

- [ ] **Step 3: Implement startup-stage logging, prefix control, and daemon log mirroring**

Add a cancellable daemon log mirror helper to `src/core/logging.rs`:

```rust
pub struct DaemonLogMirror {
    shutdown: tokio::sync::oneshot::Sender<()>,
    join: tokio::task::JoinHandle<()>,
}

pub fn attach_daemon_log_mirror(
    path: std::path::PathBuf,
    prefix_enabled: bool,
    prefix_text: String,
) -> DaemonLogMirror {
    let (shutdown_tx, mut shutdown_rx) = tokio::sync::oneshot::channel();
    let join = tokio::spawn(async move {
        let mut seen = std::fs::metadata(&path)
            .map(|metadata| metadata.len() as usize)
            .unwrap_or_default();

        loop {
            tokio::select! {
                _ = &mut shutdown_rx => break,
                _ = tokio::time::sleep(std::time::Duration::from_millis(150)) => {}
            }

            let contents = tokio::fs::read_to_string(&path).await.unwrap_or_default();
            if contents.len() <= seen {
                continue;
            }

            for line in contents[seen..].lines() {
                if prefix_enabled {
                    eprintln!("[{}] {}", prefix_text, line);
                } else {
                    eprintln!("{line}");
                }
            }
            seen = contents.len();
        }
    });

    DaemonLogMirror { shutdown: shutdown_tx, join }
}

pub fn daemon_log_path(settings: &Settings) -> std::path::PathBuf {
    resolve_log_file_path(
        settings,
        &resolve_logging_options(settings, LogComponent::Daemon, &CliLogOverrides::default())
            .file_path,
    )
    .unwrap_or_else(|| {
        crate::daemon::registry::runtime_paths()
            .map(|paths| paths.log_file)
            .unwrap_or_else(|_| std::path::PathBuf::from("daemon.log"))
    })
}
```

Wire startup stages into `src/cli/run.rs`:

```rust
pub async fn run_prepared(prepared: crate::cli::global_flags::PreparedArgs) -> MeloResult<()> {
    tracing::info!(target: "melo::cli::startup", "loading_settings");
    let raw_args = prepared.clap_args.clone();

    match crate::cli::dispatch::dispatch_args(&prepared.dispatch_args) {
        crate::cli::dispatch::Dispatch::DefaultLaunch => {
            let settings = crate::core::config::settings::Settings::load()?;
            let resolved_cli = crate::core::logging::resolve_logging_options(
                &settings,
                crate::core::logging::LogComponent::Cli,
                &prepared.logging,
            );

            let mirror = if prepared.logging.verbose {
                Some(crate::core::logging::attach_daemon_log_mirror(
                    crate::core::logging::daemon_log_path(&settings),
                    resolved_cli.prefix_enabled,
                    settings.logging.daemon_prefix.clone(),
                ))
            } else {
                None
            };

            tracing::info!(target: "melo::cli::startup", "resolving_base_url");
            let ensured =
                crate::daemon::manager::ensure_running_with_logging(&settings, &prepared.logging)
                    .await?;
            let base_url = ensured.base_url;
            tracing::info!(target: "melo::cli::startup", "opening_cwd_directly");
            drop(mirror);
            return crate::tui::run::start(
                base_url,
                crate::tui::run::LaunchContext {
                    source_label: None,
                    startup_notice: None,
                    footer_hints_enabled: settings.tui.show_footer_hints,
                },
            )
            .await;
        }
        crate::cli::dispatch::Dispatch::DirectOpen(target) => {
            tracing::info!(target: "melo::cli::startup", target = %target, "opening_explicit_target");
            let settings = crate::core::config::settings::Settings::load()?;
            let ensured =
                crate::daemon::manager::ensure_running_with_logging(&settings, &prepared.logging)
                    .await?;
            let base_url = ensured.base_url;
            let opened = crate::cli::client::ApiClient::new(base_url.clone())
                .open_target(target, "path_file")
                .await?;
            return crate::tui::run::start(
                base_url,
                crate::tui::run::LaunchContext {
                    source_label: Some(opened.source_label),
                    startup_notice: None,
                    footer_hints_enabled: settings.tui.show_footer_hints,
                },
            )
            .await;
        }
        crate::cli::dispatch::Dispatch::Clap => {}
    }

    let args = CliArgs::parse_from(raw_args);
    run_clap(args).await
}
```

Add a helper to `src/daemon/manager.rs` so startup commands can keep existing behavior while exposing stage logs:

```rust
pub struct EnsuredDaemon {
    pub base_url: String,
    pub already_running: bool,
}

pub async fn ensure_running_with_logging(
    settings: &Settings,
    _overrides: &crate::core::logging::CliLogOverrides,
) -> MeloResult<EnsuredDaemon> {
    tracing::info!(target: "melo::cli::startup", "starting_daemon");
    let paths = crate::daemon::registry::runtime_paths()?;
    let current = observe_with_paths(settings, &paths).await?;
    let observation = start(settings).await?.observation;
    Ok(EnsuredDaemon {
        base_url: observation
            .base_url
            .ok_or_else(|| MeloError::Message("daemon_not_running".to_string()))?,
        already_running: current.state == DaemonState::Running,
    })
}
```

- [ ] **Step 4: Run the focused tests to verify verbose startup logging and prefix controls pass**

Run:

```bash
rtk cargo test -q --test cli_remote verbose_explicit_open_prints_stage_logs_before_business_error
rtk cargo test -q --test cli_remote verbose_default_launch_prints_daemon_prepare_failure_excerpt
rtk cargo test -q --test cli_remote verbose_flag_can_disable_terminal_prefixes
```

Expected: all three commands PASS.

- [ ] **Step 5: Commit the startup-chain verbose slice**

```bash
rtk git add src/cli/run.rs src/core/logging.rs src/daemon/manager.rs tests/cli_remote.rs
rtk git commit -m "feat(cli): show verbose startup stages and daemon logs"
```

## Task 5: Add Explicit Daemon Log-Level Override for Auto-Started Daemons and Update Docs

**Files:**
- Modify: `src/core/logging.rs`
- Modify: `src/cli/run.rs`
- Modify: `config.example.toml`
- Modify: `README.md`
- Modify: `tests/cli_remote.rs`

- [ ] **Step 1: Write the failing daemon-override and docs tests**

Append this test to `src/core/logging/tests.rs`:

```rust
#[test]
fn daemon_override_notice_reports_running_daemon_scope_limit() {
    let settings = Settings::default();
    let notice = crate::core::logging::daemon_override_notice(
        &settings,
        &CliLogOverrides {
            verbose: true,
            log_level: None,
            no_log_prefix: false,
            daemon_log_level: Some(LoggingLevel::Trace),
        },
        true,
    );

    assert_eq!(
        notice,
        Some("daemon_log_level_override_not_applied_to_running_daemon")
    );
}
```

Append this integration test to `tests/cli_remote.rs`:

```rust
#[tokio::test(flavor = "multi_thread")]
async fn daemon_log_level_override_reports_scope_limit_when_daemon_is_already_running() {
    let app = melo::daemon::app::test_router().await;
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let mut cmd = Command::cargo_bin("melo").unwrap();
    cmd.env("MELO_BASE_URL", format!("http://{addr}"));
    cmd.arg("--verbose")
        .arg("--daemon-log-level")
        .arg("trace")
        .arg("play");

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains(
            "daemon_log_level_override_not_applied_to_running_daemon",
        ));
}
```

- [ ] **Step 2: Run the focused tests to confirm they fail**

Run:

```bash
rtk cargo test -q daemon_override_notice_reports_running_daemon_scope_limit --lib
rtk cargo test -q --test cli_remote daemon_log_level_override_reports_scope_limit_when_daemon_is_already_running
```

Expected:

- the unit test fails because `daemon_override_notice(...)` does not exist yet.
- the integration test fails because CLI does not surface the scope-limit notice when daemon is already running.

- [ ] **Step 3: Implement explicit daemon override behavior and document the auto-start-only scope**

Make the scope explicit in `src/core/logging.rs` by adding a user-facing note helper:

```rust
pub fn daemon_override_notice(
    settings: &Settings,
    overrides: &CliLogOverrides,
    daemon_already_running: bool,
) -> Option<&'static str> {
    if overrides.daemon_log_level.is_none() {
        return None;
    }
    if daemon_already_running {
        return Some("daemon_log_level_override_not_applied_to_running_daemon");
    }
    if !settings.logging.daemon.allow_runtime_level_override {
        return Some("daemon_log_level_override_disabled_by_config");
    }
    None
}
```

When a startup command discovers that daemon is already healthy, surface the notice in `src/cli/run.rs`:

```rust
let ensured = crate::daemon::manager::ensure_running_with_logging(&settings, &prepared.logging).await?;

if let Some(notice) = crate::core::logging::daemon_override_notice(
    &settings,
    &prepared.logging,
    ensured.already_running,
) {
    tracing::warn!(target: "melo::cli::startup", "{notice}");
}
```

Update `config.example.toml`:

```toml
[logging.daemon]
file_enabled = true
file_path = "logs/daemon.log"
# 是否允许命令通过 --daemon-log-level 临时提升级别。
allow_runtime_level_override = true
```

Update `README.md` with a short section like:

```md
### Logging and Verbose Diagnostics

- `melo --verbose` enables the most detailed terminal diagnostics for the current command
- `--log-level <level>` precisely overrides the current CLI log level
- `--daemon-log-level <level>` only applies to daemons launched by the current command in this phase
- `--no-log-prefix` hides `[cli]` / `[daemon]` prefixes in terminal output
```

- [ ] **Step 4: Run the focused tests to verify daemon override wiring passes**

Run:

```bash
rtk cargo test -q daemon_override_notice_reports_running_daemon_scope_limit --lib
rtk cargo test -q --test cli_remote daemon_log_level_override_reports_scope_limit_when_daemon_is_already_running
rtk cargo test -q --test config_loading settings_load_logging_defaults_and_component_overrides
```

Expected: all three commands PASS.

- [ ] **Step 5: Commit the daemon-override and docs slice**

```bash
rtk git add src/core/logging.rs src/cli/run.rs config.example.toml README.md tests/cli_remote.rs
rtk git commit -m "feat(logging): add daemon log level override for autostart"
```

## Verification Checklist

- [ ] Run the focused regression suite:

```bash
rtk cargo test -q --test config_loading
rtk cargo test -q --test cli_help
rtk cargo test -q --test cli_remote
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

Expected: only logging/config/startup-chain files and their tests/docs changed; no unrelated reversions.

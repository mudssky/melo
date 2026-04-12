use std::ffi::{OsStr, OsString};
use std::path::PathBuf;
use std::sync::LazyLock;
use std::time::Duration;

use tokio::sync::Mutex;

use crate::core::config::settings::Settings;
use crate::daemon::process::{DaemonLaunchOverrides, daemon_bind_addr, daemon_command};
use crate::daemon::registry::{DaemonRegistration, store_registration_to};

static ENV_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

struct EnvVarGuard {
    key: &'static str,
    original: Option<OsString>,
}

impl EnvVarGuard {
    /// 临时设置一个环境变量，并在 guard 销毁时恢复原值。
    ///
    /// # 参数
    /// - `key`：环境变量名
    /// - `value`：临时值
    ///
    /// # 返回值
    /// - `Self`：用于恢复环境变量的 guard
    fn set(key: &'static str, value: impl AsRef<OsStr>) -> Self {
        let original = std::env::var_os(key);
        // 测试通过全局锁串行化环境变量访问，这里集中包住 Rust 2024 的进程级可变环境操作。
        unsafe {
            std::env::set_var(key, value.as_ref());
        }
        Self { key, original }
    }

    /// 临时移除一个环境变量，并在 guard 销毁时恢复原值。
    ///
    /// # 参数
    /// - `key`：环境变量名
    ///
    /// # 返回值
    /// - `Self`：用于恢复环境变量的 guard
    fn unset(key: &'static str) -> Self {
        let original = std::env::var_os(key);
        // 测试通过全局锁串行化环境变量访问，这里集中包住 Rust 2024 的进程级可变环境操作。
        unsafe {
            std::env::remove_var(key);
        }
        Self { key, original }
    }
}

impl Drop for EnvVarGuard {
    /// 恢复环境变量的原始值，避免测试之间相互污染。
    ///
    /// # 参数
    /// - `self`：当前 guard
    ///
    /// # 返回值
    /// - 无
    fn drop(&mut self) {
        if let Some(original) = &self.original {
            // 测试通过全局锁串行化环境变量访问，这里集中包住 Rust 2024 的进程级可变环境操作。
            unsafe {
                std::env::set_var(self.key, original);
            }
        } else {
            // 测试通过全局锁串行化环境变量访问，这里集中包住 Rust 2024 的进程级可变环境操作。
            unsafe {
                std::env::remove_var(self.key);
            }
        }
    }
}

/// 启动一个只接受连接但永远不返回 HTTP 响应的监听器。
///
/// # 参数
/// - 无
///
/// # 返回值
/// - `(String, tokio::task::JoinHandle<()>)`：监听地址和后台任务句柄
async fn spawn_hanging_http_listener() -> (String, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let handle = tokio::spawn(async move {
        let (_stream, _) = listener.accept().await.unwrap();
        tokio::time::sleep(Duration::from_secs(10)).await;
    });

    (format!("http://{addr}"), handle)
}

#[test]
fn daemon_bind_addr_uses_meolo_base_url_port() {
    let addr = daemon_bind_addr("http://127.0.0.1:38123").unwrap();
    assert_eq!(addr.port(), 38123);
}

#[test]
fn daemon_command_uses_hidden_run_subcommand() {
    let command = daemon_command(PathBuf::from("melo.exe"), &DaemonLaunchOverrides::default());
    let args = command
        .get_args()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect::<Vec<_>>();

    assert_eq!(args, vec!["daemon".to_string(), "run".to_string()]);
}

#[test]
fn daemon_command_propagates_runtime_logging_env() {
    let command = daemon_command(
        PathBuf::from("melo.exe"),
        &DaemonLaunchOverrides {
            daemon_log_level: Some("trace".to_string()),
            command_id: Some("command-1".to_string()),
        },
    );
    let envs = command
        .get_envs()
        .map(|(key, value)| {
            (
                key.to_string_lossy().into_owned(),
                value.map(|item| item.to_string_lossy().into_owned()),
            )
        })
        .collect::<Vec<_>>();

    assert!(envs.iter().any(|(key, value)| {
        key == "MELO_DAEMON_LOG_LEVEL_OVERRIDE" && value.as_deref() == Some("trace")
    }));
    assert!(
        envs.iter()
            .any(|(key, value)| key == "MELO_COMMAND_ID" && value.as_deref() == Some("command-1"))
    );
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

#[tokio::test]
async fn resolve_base_url_clears_registration_when_health_probe_times_out() {
    let _env_guard = ENV_LOCK.lock().await;
    let temp = tempfile::tempdir().unwrap();
    let paths = super::super::registry::DaemonPaths {
        state_file: temp.path().join("daemon.json"),
        log_file: temp.path().join("daemon.log"),
    };
    let (base_url, handle) = spawn_hanging_http_listener().await;
    let _state_file_guard = EnvVarGuard::set("MELO_DAEMON_STATE_FILE", temp.path());
    let _base_url_guard = EnvVarGuard::unset("MELO_BASE_URL");

    store_registration_to(
        &paths.state_file,
        &DaemonRegistration {
            instance_id: "stale-instance".to_string(),
            base_url,
            pid: 999_999,
            started_at: "2026-04-12T00:00:00Z".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            backend: "noop".to_string(),
            host: "127.0.0.1".to_string(),
            port: 65535,
            log_path: paths.log_file.to_string_lossy().to_string(),
        },
    )
    .await
    .unwrap();

    let result = tokio::time::timeout(
        Duration::from_millis(1200),
        crate::daemon::process::resolve_base_url(&Settings::default()),
    )
    .await;

    handle.abort();

    let resolved = result.expect("注册探测应该在超时内返回").unwrap();
    assert_eq!(resolved, "http://127.0.0.1:38123");
    assert!(!paths.state_file.exists());
}

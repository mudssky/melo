use std::ffi::{OsStr, OsString};
use std::sync::LazyLock;
use std::time::Duration;

use tokio::sync::Mutex;

use crate::cli::observe::{ObservedDaemon, observe_read_only_daemon};

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

#[tokio::test]
async fn observe_read_only_daemon_returns_unavailable_when_health_probe_times_out() {
    let _env_guard = ENV_LOCK.lock().await;
    let (base_url, handle) = spawn_hanging_http_listener().await;
    let _base_url_guard = EnvVarGuard::set("MELO_BASE_URL", &base_url);

    let result =
        tokio::time::timeout(Duration::from_millis(1200), observe_read_only_daemon()).await;

    handle.abort();

    let observed = result.expect("health 探测应该在超时内返回").unwrap();
    assert!(matches!(observed, ObservedDaemon::Unavailable { .. }));
}

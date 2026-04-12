use std::net::SocketAddr;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Duration;

use crate::core::config::settings::Settings;
use crate::core::error::{MeloError, MeloResult};

/// 启动 daemon 子进程时附带的运行时覆盖项。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DaemonLaunchOverrides {
    /// 传给 daemon 的临时日志等级。
    pub daemon_log_level: Option<String>,
    /// 当前命令 ID，用于把父命令与子 daemon 日志关联起来。
    pub command_id: Option<String>,
}

/// 从 daemon 基础地址推导监听地址。
///
/// # 参数
/// - `base_url`：daemon 基础地址
///
/// # 返回值
/// - `MeloResult<SocketAddr>`：解析后的监听地址
pub fn daemon_bind_addr(base_url: &str) -> MeloResult<SocketAddr> {
    let url = reqwest::Url::parse(base_url).map_err(|err| MeloError::Message(err.to_string()))?;
    let host = url.host_str().unwrap_or("127.0.0.1");
    let port = url.port_or_known_default().unwrap_or(8080);
    format!("{host}:{port}")
        .parse()
        .map_err(|err: std::net::AddrParseError| MeloError::Message(err.to_string()))
}

/// 构造用于拉起 daemon 子进程的命令。
///
/// # 参数
/// - `current_exe`：当前可执行文件路径
/// - `overrides`：要透传给 daemon 的运行时覆盖项
///
/// # 返回值
/// - `Command`：已配置好的子进程命令
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

/// 后台拉起 daemon 子进程。
///
/// # 参数
/// - 无
///
/// # 返回值
/// - `MeloResult<()>`：启动结果
pub fn spawn_background_daemon() -> MeloResult<()> {
    spawn_background_daemon_with_overrides(&DaemonLaunchOverrides::default())
}

/// 带运行时覆盖项地后台拉起 daemon 子进程。
///
/// # 参数
/// - `overrides`：需要透传给子进程的运行时覆盖项
///
/// # 返回值
/// - `MeloResult<()>`：启动结果
pub fn spawn_background_daemon_with_overrides(overrides: &DaemonLaunchOverrides) -> MeloResult<()> {
    let current_exe = std::env::current_exe().map_err(|err| MeloError::Message(err.to_string()))?;
    daemon_command(current_exe, overrides)
        .spawn()
        .map(|_| ())
        .map_err(|err| MeloError::Message(err.to_string()))
}

/// 解析 daemon 应绑定的下一个可用地址。
///
/// # 参数
/// - `host`：监听主机
/// - `base_port`：首选基础端口
/// - `search_limit`：向后搜索的最大偏移
///
/// # 返回值
/// - `MeloResult<SocketAddr>`：首个可用地址
pub async fn next_bind_addr(
    host: &str,
    base_port: u16,
    search_limit: u16,
) -> MeloResult<SocketAddr> {
    for offset in 0..=search_limit {
        let Some(port) = base_port.checked_add(offset) else {
            break;
        };

        let candidate: SocketAddr = format!("{host}:{port}")
            .parse()
            .map_err(|err: std::net::AddrParseError| MeloError::Message(err.to_string()))?;
        if tokio::net::TcpListener::bind(candidate).await.is_ok() {
            return Ok(candidate);
        }
    }

    Err(MeloError::Message("daemon_port_unavailable".to_string()))
}

/// 通过显式环境变量、注册文件和默认配置解析 daemon 基础地址。
///
/// # 参数
/// - `settings`：当前配置
///
/// # 返回值
/// - `MeloResult<String>`：可用于访问 daemon 的基础地址
pub async fn resolve_base_url(settings: &Settings) -> MeloResult<String> {
    if let Ok(explicit) = std::env::var("MELO_BASE_URL") {
        return Ok(explicit);
    }

    if let Some(registration) = crate::daemon::registry::load_registration().await? {
        let client = crate::cli::client::ApiClient::new_probe(registration.base_url.clone());
        if client.health().await.is_ok() {
            return Ok(registration.base_url);
        }
        crate::daemon::registry::clear_registration().await?;
    }

    Ok(format!(
        "http://{}:{}",
        settings.daemon.host, settings.daemon.base_port
    ))
}

/// 确保 daemon 已经运行，必要时自动拉起并等待健康检查通过。
///
/// # 参数
/// - `settings`：当前配置
///
/// # 返回值
/// - `MeloResult<String>`：实际可访问的 daemon 基础地址
pub async fn ensure_running(settings: &Settings) -> MeloResult<String> {
    let base_url = resolve_base_url(settings).await?;
    let client = crate::cli::client::ApiClient::new_probe(base_url.clone());
    if client.health().await.is_ok() {
        return Ok(base_url);
    }

    spawn_background_daemon()?;

    for _ in 0..20 {
        tokio::time::sleep(Duration::from_millis(150)).await;
        let resolved = resolve_base_url(settings).await?;
        let client = crate::cli::client::ApiClient::new_probe(resolved.clone());
        if client.health().await.is_ok() {
            return Ok(resolved);
        }
    }

    Err(MeloError::Message("daemon failed to start".to_string()))
}

#[cfg(test)]
mod tests;

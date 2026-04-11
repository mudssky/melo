use std::net::SocketAddr;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Duration;

use crate::core::error::{MeloError, MeloResult};

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
/// - `_base_url`：daemon 基础地址
///
/// # 返回值
/// - `Command`：已配置好的子进程命令
pub fn daemon_command(current_exe: PathBuf, _base_url: &str) -> Command {
    let mut command = Command::new(current_exe);
    command.arg("daemon");
    command.stdin(Stdio::null());
    command.stdout(Stdio::null());
    command.stderr(Stdio::null());
    command
}

/// 确保 daemon 已经运行，必要时自动拉起并等待健康检查通过。
///
/// # 参数
/// - `base_url`：daemon 基础地址
///
/// # 返回值
/// - `MeloResult<()>`：确保运行结果
pub async fn ensure_running(base_url: &str) -> MeloResult<()> {
    let client = crate::cli::client::ApiClient::new(base_url.to_string());
    if client.health().await.is_ok() {
        return Ok(());
    }

    let current_exe = std::env::current_exe().map_err(|err| MeloError::Message(err.to_string()))?;
    daemon_command(current_exe, base_url)
        .spawn()
        .map_err(|err| MeloError::Message(err.to_string()))?;

    for _ in 0..20 {
        tokio::time::sleep(Duration::from_millis(150)).await;
        if client.health().await.is_ok() {
            return Ok(());
        }
    }

    Err(MeloError::Message("daemon failed to start".to_string()))
}

#[cfg(test)]
mod tests;

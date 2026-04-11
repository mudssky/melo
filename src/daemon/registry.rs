use std::path::PathBuf;

use crate::core::error::{MeloError, MeloResult};

const STATE_FILE_NAME: &str = "daemon.json";

/// 当前活跃 daemon 的注册信息。
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct DaemonRegistration {
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
}

/// 解析 daemon 注册状态文件路径。
///
/// # 参数
/// - `explicit`：显式覆盖路径，允许传入完整文件路径或目录路径
/// - `local_app_data`：本地应用数据目录
/// - `home_dir`：用户主目录
///
/// # 返回值
/// - `MeloResult<PathBuf>`：最终可写入的状态文件路径
pub fn state_file_path_from_env(
    explicit: Option<PathBuf>,
    local_app_data: Option<PathBuf>,
    home_dir: Option<PathBuf>,
) -> MeloResult<PathBuf> {
    if let Some(path) = explicit {
        return Ok(normalize_state_file_path(path));
    }

    if let Some(root) = local_app_data {
        return Ok(root.join("melo").join(STATE_FILE_NAME));
    }

    let home =
        home_dir.ok_or_else(|| MeloError::Message("daemon_state_path_unavailable".to_string()))?;
    Ok(home
        .join(".local")
        .join("share")
        .join("melo")
        .join(STATE_FILE_NAME))
}

/// 规范化显式传入的状态文件路径。
///
/// # 参数
/// - `path`：外部传入的目录或文件路径
///
/// # 返回值
/// - `PathBuf`：补齐文件名后的状态文件路径
fn normalize_state_file_path(path: PathBuf) -> PathBuf {
    let file_name = path.file_name().and_then(|value| value.to_str());
    if matches!(file_name, Some(STATE_FILE_NAME)) {
        return path;
    }

    if path.extension().is_some() {
        return path;
    }

    path.join(STATE_FILE_NAME)
}

#[cfg(test)]
mod tests;

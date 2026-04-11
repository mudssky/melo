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

/// 根据当前环境变量解析 daemon 注册状态文件路径。
///
/// # 参数
/// - 无
///
/// # 返回值
/// - `MeloResult<PathBuf>`：当前用户作用域下的状态文件路径
pub fn state_file_path() -> MeloResult<PathBuf> {
    let explicit = std::env::var_os("MELO_DAEMON_STATE_FILE").map(PathBuf::from);
    let local_app_data = std::env::var_os("LOCALAPPDATA").map(PathBuf::from);
    let home_dir = std::env::var_os("HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("USERPROFILE").map(PathBuf::from));
    state_file_path_from_env(explicit, local_app_data, home_dir)
}

/// 读取当前 daemon 注册信息。
///
/// # 参数
/// - 无
///
/// # 返回值
/// - `MeloResult<Option<DaemonRegistration>>`：存在时返回注册信息，不存在时返回 `None`
pub async fn load_registration() -> MeloResult<Option<DaemonRegistration>> {
    let path = state_file_path()?;
    let json = match tokio::fs::read_to_string(&path).await {
        Ok(value) => value,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(MeloError::Message(err.to_string())),
    };

    serde_json::from_str(&json)
        .map(Some)
        .map_err(|err| MeloError::Message(err.to_string()))
}

/// 写入当前 daemon 注册信息。
///
/// # 参数
/// - `registration`：待持久化的注册信息
///
/// # 返回值
/// - `MeloResult<()>`：写入结果
pub async fn store_registration(registration: &DaemonRegistration) -> MeloResult<()> {
    let path = state_file_path()?;
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

/// 清除当前 daemon 注册信息。
///
/// # 参数
/// - 无
///
/// # 返回值
/// - `MeloResult<()>`：删除结果，不存在时也视为成功
pub async fn clear_registration() -> MeloResult<()> {
    let path = state_file_path()?;
    match tokio::fs::remove_file(path).await {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(MeloError::Message(err.to_string())),
    }
}

#[cfg(test)]
mod tests;

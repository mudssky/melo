use std::path::{Path, PathBuf};

use time::format_description::well_known::Rfc3339;

use crate::core::error::{MeloError, MeloResult};

const STATE_FILE_NAME: &str = "daemon.json";
const LOG_FILE_NAME: &str = "daemon.log";

/// daemon 运行期文件路径集合。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DaemonPaths {
    /// 注册文件路径。
    pub state_file: PathBuf,
    /// 日志文件路径。
    pub log_file: PathBuf,
}

/// 当前活跃 daemon 的注册信息。
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct DaemonRegistration {
    /// 当前 daemon 实例唯一 ID。
    pub instance_id: String,
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
    /// 固定日志文件路径。
    pub log_path: String,
}

/// 根据环境信息解析 daemon 运行期文件路径。
///
/// # 参数
/// - `explicit`：显式状态文件路径或目录路径
/// - `local_app_data`：Windows 本地应用数据目录
/// - `home_dir`：用户家目录
///
/// # 返回值
/// - `MeloResult<DaemonPaths>`：状态文件与日志文件路径
pub fn runtime_paths_from_env(
    explicit: Option<PathBuf>,
    local_app_data: Option<PathBuf>,
    home_dir: Option<PathBuf>,
) -> MeloResult<DaemonPaths> {
    let runtime_dir = if let Some(path) = explicit {
        normalize_runtime_dir(path)
    } else if let Some(root) = local_app_data {
        root.join("melo")
    } else {
        let home = home_dir
            .ok_or_else(|| MeloError::Message("daemon_state_path_unavailable".to_string()))?;
        home.join(".local").join("share").join("melo")
    };

    Ok(DaemonPaths {
        state_file: runtime_dir.join(STATE_FILE_NAME),
        log_file: runtime_dir.join(LOG_FILE_NAME),
    })
}

/// 根据当前环境变量解析 daemon 运行期文件路径。
///
/// # 参数
/// - 无
///
/// # 返回值
/// - `MeloResult<DaemonPaths>`：当前用户作用域下的运行期文件路径
pub fn runtime_paths() -> MeloResult<DaemonPaths> {
    let explicit = std::env::var_os("MELO_DAEMON_STATE_FILE").map(PathBuf::from);
    let local_app_data = std::env::var_os("LOCALAPPDATA").map(PathBuf::from);
    let home_dir = std::env::var_os("HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("USERPROFILE").map(PathBuf::from));
    runtime_paths_from_env(explicit, local_app_data, home_dir)
}

/// 兼容旧调用方，返回注册文件路径。
///
/// # 参数
/// - `explicit`：显式状态文件路径或目录路径
/// - `local_app_data`：Windows 本地应用数据目录
/// - `home_dir`：用户家目录
///
/// # 返回值
/// - `MeloResult<PathBuf>`：最终注册文件路径
pub fn state_file_path_from_env(
    explicit: Option<PathBuf>,
    local_app_data: Option<PathBuf>,
    home_dir: Option<PathBuf>,
) -> MeloResult<PathBuf> {
    Ok(runtime_paths_from_env(explicit, local_app_data, home_dir)?.state_file)
}

/// 兼容旧调用方，返回当前注册文件路径。
///
/// # 参数
/// - 无
///
/// # 返回值
/// - `MeloResult<PathBuf>`：当前注册文件路径
pub fn state_file_path() -> MeloResult<PathBuf> {
    Ok(runtime_paths()?.state_file)
}

/// 把 Unix 秒级时间戳格式化为 RFC3339 文本。
///
/// # 参数
/// - `seconds`：Unix 秒级时间戳
///
/// # 返回值
/// - `MeloResult<String>`：RFC3339 时间文本
pub fn started_at_text_from_unix_seconds(seconds: u64) -> MeloResult<String> {
    time::OffsetDateTime::from_unix_timestamp(seconds as i64)
        .map_err(|err| MeloError::Message(err.to_string()))?
        .format(&Rfc3339)
        .map_err(|err| MeloError::Message(err.to_string()))
}

/// 生成当前 UTC 时间的 RFC3339 文本。
///
/// # 参数
/// - 无
///
/// # 返回值
/// - `MeloResult<String>`：当前时间文本
pub fn now_started_at_text() -> MeloResult<String> {
    time::OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .map_err(|err| MeloError::Message(err.to_string()))
}

/// 从指定路径读取 daemon 注册信息。
///
/// # 参数
/// - `path`：注册文件路径
///
/// # 返回值
/// - `MeloResult<Option<DaemonRegistration>>`：存在时返回注册信息，不存在时返回 `None`
pub async fn load_registration_from(path: &Path) -> MeloResult<Option<DaemonRegistration>> {
    let json = match tokio::fs::read_to_string(path).await {
        Ok(value) => value,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(MeloError::Message(err.to_string())),
    };

    serde_json::from_str(&json)
        .map(Some)
        .map_err(|err| MeloError::Message(err.to_string()))
}

/// 读取当前 daemon 注册信息。
///
/// # 参数
/// - 无
///
/// # 返回值
/// - `MeloResult<Option<DaemonRegistration>>`：存在时返回注册信息，不存在时返回 `None`
pub async fn load_registration() -> MeloResult<Option<DaemonRegistration>> {
    let paths = runtime_paths()?;
    load_registration_from(&paths.state_file).await
}

/// 把注册信息写入指定路径。
///
/// # 参数
/// - `path`：目标注册文件路径
/// - `registration`：待持久化的注册信息
///
/// # 返回值
/// - `MeloResult<()>`：写入结果
pub async fn store_registration_to(
    path: &Path,
    registration: &DaemonRegistration,
) -> MeloResult<()> {
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

/// 写入当前 daemon 注册信息。
///
/// # 参数
/// - `registration`：待持久化的注册信息
///
/// # 返回值
/// - `MeloResult<()>`：写入结果
pub async fn store_registration(registration: &DaemonRegistration) -> MeloResult<()> {
    let paths = runtime_paths()?;
    store_registration_to(&paths.state_file, registration).await
}

/// 删除指定路径下的注册文件。
///
/// # 参数
/// - `path`：注册文件路径
///
/// # 返回值
/// - `MeloResult<()>`：删除结果，不存在时也视为成功
pub async fn clear_registration_from(path: &Path) -> MeloResult<()> {
    match tokio::fs::remove_file(path).await {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(MeloError::Message(err.to_string())),
    }
}

/// 清除当前 daemon 注册信息。
///
/// # 参数
/// - 无
///
/// # 返回值
/// - `MeloResult<()>`：删除结果，不存在时也视为成功
pub async fn clear_registration() -> MeloResult<()> {
    let paths = runtime_paths()?;
    clear_registration_from(&paths.state_file).await
}

/// 规范化显式传入的运行目录。
///
/// # 参数
/// - `path`：外部传入的目录或文件路径
///
/// # 返回值
/// - `PathBuf`：可放置 `daemon.json` 与 `daemon.log` 的目录
fn normalize_runtime_dir(path: PathBuf) -> PathBuf {
    let file_name = path.file_name().and_then(|value| value.to_str());
    if matches!(file_name, Some(STATE_FILE_NAME) | Some(LOG_FILE_NAME)) {
        return path.parent().unwrap_or(Path::new(".")).to_path_buf();
    }

    if path.extension().is_some() {
        return path.parent().unwrap_or(Path::new(".")).to_path_buf();
    }

    path
}

#[cfg(test)]
mod tests;

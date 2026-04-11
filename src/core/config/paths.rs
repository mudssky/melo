use std::env;
use std::path::{Path, PathBuf};

/// 返回平台默认 Melo 根目录。
///
/// # 参数
/// - 无
///
/// # 返回值
/// - `PathBuf`：平台标准 Melo 根目录
pub fn default_melo_root() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| env::current_dir().expect("current dir unavailable"))
        .join("melo")
}

/// 返回默认配置文件路径。
///
/// # 参数
/// - 无
///
/// # 返回值
/// - `PathBuf`：默认配置文件路径
pub fn default_config_path() -> PathBuf {
    default_melo_root().join("config.toml")
}

/// 返回默认数据库文件路径。
///
/// # 参数
/// - 无
///
/// # 返回值
/// - `PathBuf`：默认数据库路径
pub fn default_database_path() -> PathBuf {
    default_melo_root().join("melo.db")
}

/// 基于配置文件目录解析相对路径。
///
/// # 参数
/// - `config_path`：配置文件绝对路径
/// - `value`：配置中的路径值
///
/// # 返回值
/// - `PathBuf`：解析后的绝对路径
pub fn resolve_from_config_dir(config_path: &Path, value: &Path) -> PathBuf {
    if value.is_absolute() {
        return value.to_path_buf();
    }

    config_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(value)
}

#[cfg(test)]
mod tests;

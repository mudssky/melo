use camino::Utf8PathBuf;
use serde::Deserialize;

use crate::core::error::{MeloError, MeloResult};

/// 数据库相关配置。
#[derive(Debug, Clone, Deserialize)]
pub struct DatabaseSettings {
    /// SQLite 数据库文件路径。
    pub path: Utf8PathBuf,
}

/// Melo 的全局配置。
#[derive(Debug, Clone, Deserialize)]
pub struct Settings {
    /// 数据库配置。
    pub database: DatabaseSettings,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            database: DatabaseSettings {
                path: Utf8PathBuf::from("local/melo.db"),
            },
        }
    }
}

impl Settings {
    /// 从配置文件加载设置，未提供时回退到默认值。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回
    /// - `MeloResult<Self>`：解析后的配置
    pub fn load() -> MeloResult<Self> {
        let path = std::env::var("MELO_CONFIG").unwrap_or_else(|_| "config.toml".to_string());
        let builder = config::Config::builder()
            .add_source(config::File::with_name(&path).required(false))
            .set_default("database.path", "local/melo.db")
            .map_err(|err| MeloError::Message(err.to_string()))?;

        builder
            .build()
            .map_err(|err| MeloError::Message(err.to_string()))?
            .try_deserialize()
            .map_err(|err| MeloError::Message(err.to_string()))
    }

    /// 为测试构造一个只指定数据库路径的配置。
    ///
    /// # 参数
    /// - `path`：测试数据库路径
    ///
    /// # 返回
    /// - `Self`：测试用配置
    pub fn for_test(path: std::path::PathBuf) -> Self {
        Self {
            database: DatabaseSettings {
                path: Utf8PathBuf::from_path_buf(path).expect("测试数据库路径必须是 UTF-8"),
            },
        }
    }
}

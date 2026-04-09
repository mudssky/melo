use std::collections::BTreeMap;

use camino::Utf8PathBuf;
use serde::Deserialize;

use crate::core::error::{MeloError, MeloResult};

/// 数据库相关配置。
#[derive(Debug, Clone, Deserialize)]
pub struct DatabaseSettings {
    /// SQLite 数据库文件路径。
    pub path: Utf8PathBuf,
}

/// Smart playlist 配置项。
#[derive(Debug, Clone, Default, Deserialize)]
pub struct SmartPlaylistDefinition {
    /// 查询字符串。
    pub query: String,
    /// 可选描述。
    pub description: Option<String>,
}

/// 播放列表配置。
#[derive(Debug, Clone, Default, Deserialize)]
pub struct PlaylistSettings {
    /// 智能歌单集合。
    #[serde(default)]
    pub smart: BTreeMap<String, SmartPlaylistDefinition>,
}

/// organize 规则匹配条件。
#[derive(Debug, Clone, Default, Deserialize)]
pub struct OrganizeMatchSettings {
    /// 静态歌单名。
    pub static_playlist: Option<String>,
}

/// organize 单条规则。
#[derive(Debug, Clone, Default, Deserialize)]
pub struct OrganizeRuleSettings {
    /// 规则名称。
    pub name: String,
    /// 优先级。
    pub priority: i32,
    /// 匹配条件。
    #[serde(rename = "match", default)]
    pub match_rule: OrganizeMatchSettings,
    /// 路径模板。
    pub template: String,
}

/// organize 配置。
#[derive(Debug, Clone, Default, Deserialize)]
pub struct OrganizeSettings {
    /// 文件组织根目录。
    pub base_dir: Option<String>,
    /// 冲突策略。
    pub conflict_policy: Option<String>,
    /// 规则列表。
    #[serde(default)]
    pub rules: Vec<OrganizeRuleSettings>,
}

/// 媒体库配置。
#[derive(Debug, Clone, Default, Deserialize)]
pub struct LibrarySettings {
    /// 文件组织配置。
    #[serde(default)]
    pub organize: OrganizeSettings,
}

/// Melo 的全局配置。
#[derive(Debug, Clone, Deserialize)]
pub struct Settings {
    /// 数据库配置。
    pub database: DatabaseSettings,
    /// 媒体库配置。
    #[serde(default)]
    pub library: LibrarySettings,
    /// 播放列表配置。
    #[serde(default)]
    pub playlists: PlaylistSettings,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            database: DatabaseSettings {
                path: Utf8PathBuf::from("local/melo.db"),
            },
            library: LibrarySettings::default(),
            playlists: PlaylistSettings::default(),
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
            library: LibrarySettings::default(),
            playlists: PlaylistSettings::default(),
        }
    }
}

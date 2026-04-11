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

impl Default for DatabaseSettings {
    fn default() -> Self {
        Self {
            path: Utf8PathBuf::from("local/melo.db"),
        }
    }
}

/// 播放器相关配置。
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct PlayerSettings {
    /// 默认音量。
    pub volume: u8,
    /// 是否恢复上一次 daemon 会话。
    pub restore_last_session: bool,
    /// 恢复后是否自动继续播放。
    pub resume_after_restore: bool,
}

impl Default for PlayerSettings {
    fn default() -> Self {
        Self {
            volume: 100,
            restore_last_session: true,
            resume_after_restore: false,
        }
    }
}

/// 直接打开相关配置。
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct OpenSettings {
    /// 裸 `melo` 是否扫描当前目录。
    pub scan_current_dir: bool,
    /// 目录递归扫描深度。
    pub max_depth: usize,
    /// 进入前台前同步预热的曲目数量。
    pub prewarm_limit: usize,
    /// 后台任务并发度。
    pub background_jobs: usize,
}

impl Default for OpenSettings {
    fn default() -> Self {
        Self {
            scan_current_dir: true,
            max_depth: 2,
            prewarm_limit: 20,
            background_jobs: 4,
        }
    }
}

/// Smart playlist 配置项。
#[derive(Debug, Clone, Default, Deserialize)]
pub struct SmartPlaylistDefinition {
    /// 查询字符串。
    pub query: String,
    /// 可选描述。
    pub description: Option<String>,
}

/// 临时歌单可见性配置。
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct EphemeralVisibilitySettings {
    /// 文件路径临时歌单是否可见。
    pub path_file: bool,
    /// 目录路径临时歌单是否可见。
    pub path_dir: bool,
    /// 当前目录临时歌单是否可见。
    pub cwd_dir: bool,
}

impl Default for EphemeralVisibilitySettings {
    fn default() -> Self {
        Self {
            path_file: false,
            path_dir: true,
            cwd_dir: true,
        }
    }
}

/// 临时歌单配置。
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct EphemeralPlaylistSettings {
    /// 默认过期时间，单位为秒。
    pub default_ttl_seconds: u64,
    /// 来源类型对应的可见性策略。
    pub visibility: EphemeralVisibilitySettings,
}

/// 播放列表配置。
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct PlaylistSettings {
    /// 智能歌单集合。
    pub smart: BTreeMap<String, SmartPlaylistDefinition>,
    /// 临时歌单配置。
    pub ephemeral: EphemeralPlaylistSettings,
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
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Settings {
    /// 数据库配置。
    pub database: DatabaseSettings,
    /// 播放器配置。
    #[serde(default)]
    pub player: PlayerSettings,
    /// 直接打开配置。
    #[serde(default)]
    pub open: OpenSettings,
    /// 媒体库配置。
    #[serde(default)]
    pub library: LibrarySettings,
    /// 播放列表配置。
    #[serde(default)]
    pub playlists: PlaylistSettings,
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
        Self::load_from_path(path)
    }

    /// 从指定路径加载设置，未提供时回退到默认值。
    ///
    /// # 参数
    /// - `path`：配置文件路径
    ///
    /// # 返回
    /// - `MeloResult<Self>`：解析后的配置
    pub fn load_from_path(path: impl AsRef<std::path::Path>) -> MeloResult<Self> {
        let builder = config::Config::builder()
            .add_source(config::File::from(path.as_ref()).required(false))
            .set_default("database.path", "local/melo.db")
            .map_err(|err| MeloError::Message(err.to_string()))?
            .set_default("player.volume", 100)
            .map_err(|err| MeloError::Message(err.to_string()))?
            .set_default("player.restore_last_session", true)
            .map_err(|err| MeloError::Message(err.to_string()))?
            .set_default("player.resume_after_restore", false)
            .map_err(|err| MeloError::Message(err.to_string()))?
            .set_default("open.scan_current_dir", true)
            .map_err(|err| MeloError::Message(err.to_string()))?
            .set_default("open.max_depth", 2)
            .map_err(|err| MeloError::Message(err.to_string()))?
            .set_default("open.prewarm_limit", 20)
            .map_err(|err| MeloError::Message(err.to_string()))?
            .set_default("open.background_jobs", 4)
            .map_err(|err| MeloError::Message(err.to_string()))?
            .set_default("playlists.ephemeral.default_ttl_seconds", 0)
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
            ..Self::default()
        }
    }
}

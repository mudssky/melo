use std::collections::BTreeMap;
use std::path::PathBuf;

use camino::Utf8PathBuf;
use serde::Deserialize;

use crate::core::config::paths;
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
            path: Utf8PathBuf::from_path_buf(paths::default_database_path())
                .expect("默认数据库路径必须是 UTF-8"),
        }
    }
}

/// Daemon 文档可见性模式。
#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DaemonDocsMode {
    Disabled,
    #[default]
    Local,
    Network,
}

impl DaemonDocsMode {
    /// 返回 docs 模式的稳定字符串表示。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - `&'static str`：模式文本
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::Local => "local",
            Self::Network => "network",
        }
    }
}

/// Daemon 相关配置。
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct DaemonSettings {
    /// Daemon 绑定主机。
    pub host: String,
    /// 首选基础端口。
    pub base_port: u16,
    /// 高位端口自动避让次数。
    pub port_search_limit: u16,
    /// docs 对外可见性策略。
    pub docs: DaemonDocsMode,
}

impl Default for DaemonSettings {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            base_port: 38123,
            port_search_limit: 32,
            docs: DaemonDocsMode::Local,
        }
    }
}

/// MPV 后端相关配置。
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct MpvSettings {
    /// `mpv` 可执行文件路径。
    pub path: String,
    /// IPC 路径目录或特殊值。
    pub ipc_dir: String,
    /// 追加给 `mpv` 的额外参数。
    pub extra_args: Vec<String>,
}

impl Default for MpvSettings {
    fn default() -> Self {
        Self {
            path: "mpv".to_string(),
            ipc_dir: "auto".to_string(),
            extra_args: Vec::new(),
        }
    }
}

/// 播放器相关配置。
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct PlayerSettings {
    /// 播放后端选择。
    pub backend: String,
    /// 默认音量。
    pub volume: u8,
    /// 是否恢复上一次 daemon 会话。
    pub restore_last_session: bool,
    /// 恢复后是否自动继续播放。
    pub resume_after_restore: bool,
    /// MPV 后端配置。
    pub mpv: MpvSettings,
}

impl Default for PlayerSettings {
    fn default() -> Self {
        Self {
            backend: "auto".to_string(),
            volume: 100,
            restore_last_session: true,
            resume_after_restore: false,
            mpv: MpvSettings::default(),
        }
    }
}

/// TUI 相关配置。
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct TuiSettings {
    /// 是否显示底部帮助提示。
    pub show_footer_hints: bool,
}

impl Default for TuiSettings {
    fn default() -> Self {
        Self {
            show_footer_hints: true,
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
    /// Daemon 配置。
    #[serde(default)]
    pub daemon: DaemonSettings,
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
    /// TUI 配置。
    #[serde(default)]
    pub tui: TuiSettings,
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
        let path = std::env::var("MELO_CONFIG_PATH")
            .or_else(|_| std::env::var("MELO_CONFIG"))
            .map(PathBuf::from)
            .unwrap_or_else(|_| paths::default_config_path());
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
        let config_path = path.as_ref().to_path_buf();
        let builder = config::Config::builder()
            .add_source(config::File::from(config_path.as_path()).required(false))
            .set_default("database.path", "melo.db")
            .map_err(|err| MeloError::Message(err.to_string()))?
            .set_default("daemon.host", "127.0.0.1")
            .map_err(|err| MeloError::Message(err.to_string()))?
            .set_default("daemon.base_port", 38123)
            .map_err(|err| MeloError::Message(err.to_string()))?
            .set_default("daemon.port_search_limit", 32)
            .map_err(|err| MeloError::Message(err.to_string()))?
            .set_default("daemon.docs", DaemonDocsMode::Local.as_str())
            .map_err(|err| MeloError::Message(err.to_string()))?
            .set_default("player.backend", "auto")
            .map_err(|err| MeloError::Message(err.to_string()))?
            .set_default("player.volume", 100)
            .map_err(|err| MeloError::Message(err.to_string()))?
            .set_default("player.restore_last_session", true)
            .map_err(|err| MeloError::Message(err.to_string()))?
            .set_default("player.resume_after_restore", false)
            .map_err(|err| MeloError::Message(err.to_string()))?
            .set_default("player.mpv.path", "mpv")
            .map_err(|err| MeloError::Message(err.to_string()))?
            .set_default("player.mpv.ipc_dir", "auto")
            .map_err(|err| MeloError::Message(err.to_string()))?
            .set_default("player.mpv.extra_args", Vec::<String>::new())
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
            .map_err(|err| MeloError::Message(err.to_string()))?
            .set_default("tui.show_footer_hints", true)
            .map_err(|err| MeloError::Message(err.to_string()))?;

        let mut settings: Self = builder
            .build()
            .map_err(|err| MeloError::Message(err.to_string()))?
            .try_deserialize()
            .map_err(|err| MeloError::Message(err.to_string()))?;

        let resolved_db = std::env::var("MELO_DB_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                paths::resolve_from_config_dir(&config_path, settings.database.path.as_std_path())
            });
        settings.database.path = Utf8PathBuf::from_path_buf(resolved_db)
            .map_err(|_| MeloError::Message("database path must be utf-8".to_string()))?;

        Ok(settings)
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

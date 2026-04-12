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

/// 日志等级配置。
#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum LoggingLevel {
    Error,
    #[default]
    Warning,
    Info,
    Debug,
    Trace,
}

impl LoggingLevel {
    /// 返回日志等级的稳定字符串表示。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - `&'static str`：日志等级文本
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warning => "warning",
            Self::Info => "info",
            Self::Debug => "debug",
            Self::Trace => "trace",
        }
    }

    /// 返回两个日志等级中更详细的那个。
    ///
    /// # 参数
    /// - `other`：另一个待比较的日志等级
    ///
    /// # 返回值
    /// - `LoggingLevel`：更详细的日志等级
    pub fn max(self, other: Self) -> Self {
        if self.rank() >= other.rank() {
            self
        } else {
            other
        }
    }

    /// 返回日志等级的比较权重。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - `u8`：日志等级权重，越大表示越详细
    fn rank(self) -> u8 {
        match self {
            Self::Error => 1,
            Self::Warning => 2,
            Self::Info => 3,
            Self::Debug => 4,
            Self::Trace => 5,
        }
    }
}

impl std::str::FromStr for LoggingLevel {
    type Err = ();

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "error" => Ok(Self::Error),
            "warning" => Ok(Self::Warning),
            "info" => Ok(Self::Info),
            "debug" => Ok(Self::Debug),
            "trace" => Ok(Self::Trace),
            _ => Err(()),
        }
    }
}

/// 日志输出格式。
#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum LoggingFormat {
    #[default]
    Pretty,
    Json,
}

impl LoggingFormat {
    /// 返回日志格式的稳定字符串表示。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - `&'static str`：日志格式文本
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pretty => "pretty",
            Self::Json => "json",
        }
    }
}

/// 单组件日志配置。
#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct LoggingComponentSettings {
    /// 组件级日志等级覆盖。
    pub level: Option<LoggingLevel>,
    /// 是否启用文件日志。
    pub file_enabled: bool,
    /// 文件日志路径。
    pub file_path: Option<String>,
    /// 是否启用终端前缀。
    pub prefix_enabled: Option<bool>,
}

/// Daemon 组件日志配置。
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct DaemonLoggingSettings {
    /// 组件级日志等级覆盖。
    pub level: Option<LoggingLevel>,
    /// 是否启用文件日志。
    pub file_enabled: bool,
    /// 文件日志路径。
    pub file_path: Option<String>,
    /// 是否启用终端前缀。
    pub prefix_enabled: Option<bool>,
    /// 是否允许运行时临时提升级别。
    pub allow_runtime_level_override: bool,
}

impl Default for DaemonLoggingSettings {
    fn default() -> Self {
        Self {
            level: None,
            file_enabled: true,
            file_path: None,
            prefix_enabled: None,
            allow_runtime_level_override: true,
        }
    }
}

/// 日志总配置。
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct LoggingSettings {
    /// 全局默认日志等级。
    pub level: LoggingLevel,
    /// 终端输出格式。
    pub terminal_format: LoggingFormat,
    /// 文件输出格式。
    pub file_format: LoggingFormat,
    /// 是否默认启用前缀。
    pub prefix_enabled: bool,
    /// CLI 前缀文本。
    pub cli_prefix: String,
    /// Daemon 前缀文本。
    pub daemon_prefix: String,
    /// CLI 日志配置。
    pub cli: LoggingComponentSettings,
    /// Daemon 日志配置。
    pub daemon: DaemonLoggingSettings,
}

impl Default for LoggingSettings {
    fn default() -> Self {
        Self {
            level: LoggingLevel::Warning,
            terminal_format: LoggingFormat::Pretty,
            file_format: LoggingFormat::Json,
            prefix_enabled: true,
            cli_prefix: "cli".to_string(),
            daemon_prefix: "daemon".to_string(),
            cli: LoggingComponentSettings::default(),
            daemon: DaemonLoggingSettings::default(),
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
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(untagged)]
pub enum TuiBindingSpec {
    /// 单个按键或组合键。
    Chord(String),
    /// 多步按键序列。
    Sequence(Vec<String>),
}

/// TUI 相关配置。
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct TuiSettings {
    /// 是否显示底部帮助提示。
    pub show_footer_hints: bool,
    /// 是否启用鼠标输入。
    pub mouse_enabled: bool,
    /// TUI 动作到按键绑定的覆盖配置。
    pub keymap: BTreeMap<String, Vec<TuiBindingSpec>>,
}

impl Default for TuiSettings {
    fn default() -> Self {
        Self {
            show_footer_hints: true,
            mouse_enabled: true,
            keymap: BTreeMap::new(),
        }
    }
}

/// 运行时扫描模板配置。
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct RuntimeScanTemplateSettings {
    /// CLI 扫描启动提示模板。
    pub cli_start: Option<String>,
    /// CLI 切入 TUI 的提示模板。
    pub cli_handoff: Option<String>,
    /// TUI 中活动扫描任务提示模板。
    pub tui_active: Option<String>,
    /// TUI 中扫描完成提示模板。
    pub tui_done: Option<String>,
    /// TUI 中扫描失败提示模板。
    pub tui_failed: Option<String>,
}

/// 运行时模板配置。
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct RuntimeTemplateSettings {
    /// 扫描相关的运行时模板集合。
    pub scan: RuntimeScanTemplateSettings,
}

/// 模板总配置。
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct TemplateSettings {
    /// 运行时动态提示模板。
    pub runtime: RuntimeTemplateSettings,
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
    /// 日志配置。
    #[serde(default)]
    pub logging: LoggingSettings,
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
    /// 模板配置。
    #[serde(default)]
    pub templates: TemplateSettings,
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
            .set_default("logging.level", LoggingLevel::Warning.as_str())
            .map_err(|err| MeloError::Message(err.to_string()))?
            .set_default("logging.terminal_format", LoggingFormat::Pretty.as_str())
            .map_err(|err| MeloError::Message(err.to_string()))?
            .set_default("logging.file_format", LoggingFormat::Json.as_str())
            .map_err(|err| MeloError::Message(err.to_string()))?
            .set_default("logging.prefix_enabled", true)
            .map_err(|err| MeloError::Message(err.to_string()))?
            .set_default("logging.cli_prefix", "cli")
            .map_err(|err| MeloError::Message(err.to_string()))?
            .set_default("logging.daemon_prefix", "daemon")
            .map_err(|err| MeloError::Message(err.to_string()))?
            .set_default("logging.cli.file_enabled", false)
            .map_err(|err| MeloError::Message(err.to_string()))?
            .set_default("logging.daemon.file_enabled", true)
            .map_err(|err| MeloError::Message(err.to_string()))?
            .set_default("logging.daemon.allow_runtime_level_override", true)
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
            .map_err(|err| MeloError::Message(err.to_string()))?
            .set_default("tui.mouse_enabled", true)
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

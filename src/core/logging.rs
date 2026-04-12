use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde_json::{Map, Value};
use time::format_description::well_known::Rfc3339;
use tracing::field::{Field, Visit};
use tracing::{Event, Subscriber};
use tracing_subscriber::layer::Context;
use tracing_subscriber::{EnvFilter, Layer, prelude::*};

use crate::core::config::settings::{LoggingFormat, LoggingLevel, Settings};

/// 当前日志所属组件。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogComponent {
    /// 命令行前台进程。
    Cli,
    /// 后台 daemon 进程。
    Daemon,
}

/// CLI 传入的日志覆盖选项。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CliLogOverrides {
    /// 是否启用 verbose 模式。
    pub verbose: bool,
    /// CLI 级别显式覆盖。
    pub log_level: Option<LoggingLevel>,
    /// 是否关闭终端前缀。
    pub no_log_prefix: bool,
    /// Daemon 级别显式覆盖。
    pub daemon_log_level: Option<LoggingLevel>,
}

/// 解析后的组件日志选项。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedLoggingOptions {
    /// 生效日志等级。
    pub level: LoggingLevel,
    /// 是否输出终端前缀。
    pub prefix_enabled: bool,
    /// 当前组件使用的前缀文本。
    pub prefix_text: String,
    /// 是否启用文件输出。
    pub file_enabled: bool,
    /// 生效文件路径。
    pub file_path: Option<String>,
    /// Daemon 运行时覆盖是否被配置阻止。
    pub daemon_runtime_override_blocked: bool,
}

/// 当前命令运行时上下文。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeLogContext {
    /// 当前会话 ID。
    pub session_id: String,
    /// 当前命令 ID。
    pub command_id: String,
}

/// 根据全局配置、组件配置和 CLI 参数解析组件日志选项。
///
/// # 参数
/// - `settings`：全局配置
/// - `component`：当前日志组件
/// - `overrides`：当前命令携带的日志覆盖参数
///
/// # 返回值
/// - `ResolvedLoggingOptions`：最终生效的日志选项
pub fn resolve_logging_options(
    settings: &Settings,
    component: LogComponent,
    overrides: &CliLogOverrides,
) -> ResolvedLoggingOptions {
    match component {
        LogComponent::Cli => {
            let configured_level = settings.logging.cli.level.unwrap_or(settings.logging.level);
            let level = overrides.log_level.unwrap_or_else(|| {
                if overrides.verbose {
                    configured_level.max(LoggingLevel::Info)
                } else {
                    configured_level
                }
            });
            let prefix_enabled = if overrides.no_log_prefix {
                false
            } else {
                settings
                    .logging
                    .cli
                    .prefix_enabled
                    .unwrap_or(settings.logging.prefix_enabled)
            };

            ResolvedLoggingOptions {
                level,
                prefix_enabled,
                prefix_text: settings.logging.cli_prefix.clone(),
                file_enabled: settings.logging.cli.file_enabled,
                file_path: settings.logging.cli.file_path.clone(),
                daemon_runtime_override_blocked: false,
            }
        }
        LogComponent::Daemon => {
            let blocked = overrides.daemon_log_level.is_some()
                && !settings.logging.daemon.allow_runtime_level_override;
            let configured_level = settings
                .logging
                .daemon
                .level
                .unwrap_or(settings.logging.level);
            let level = if blocked {
                configured_level
            } else {
                overrides.daemon_log_level.unwrap_or_else(|| {
                    if overrides.verbose {
                        configured_level.max(LoggingLevel::Info)
                    } else {
                        configured_level
                    }
                })
            };

            ResolvedLoggingOptions {
                level,
                prefix_enabled: settings
                    .logging
                    .daemon
                    .prefix_enabled
                    .unwrap_or(settings.logging.prefix_enabled),
                prefix_text: settings.logging.daemon_prefix.clone(),
                file_enabled: settings.logging.daemon.file_enabled,
                file_path: settings.logging.daemon.file_path.clone(),
                daemon_runtime_override_blocked: blocked,
            }
        }
    }
}

/// 为 tracing 初始化 `EnvFilter`。
///
/// # 参数
/// - `level`：当前组件的生效日志等级
///
/// # 返回值
/// - `EnvFilter`：tracing 过滤器
pub fn env_filter_for(level: LoggingLevel) -> EnvFilter {
    EnvFilter::new(level.as_str())
}

/// 初始化当前进程的 tracing。
///
/// # 参数
/// - `settings`：全局配置
/// - `component`：当前进程所属组件
/// - `overrides`：当前命令的日志覆盖参数
/// - `context`：当前命令运行时上下文
///
/// # 返回值
/// - 无
pub fn init_tracing(
    settings: &Settings,
    component: LogComponent,
    overrides: &CliLogOverrides,
    context: RuntimeLogContext,
) {
    let resolved = resolve_logging_options(settings, component, overrides);
    let terminal_layer = StructuredLogLayer::new_terminal(
        settings.logging.terminal_format,
        resolved.prefix_enabled,
        resolved.prefix_text.clone(),
        component,
        context.clone(),
        shared_writer(Box::new(std::io::stderr())),
    );
    let file_layer = build_file_layer(settings, &resolved, component, &context);
    let subscriber = tracing_subscriber::registry()
        .with(env_filter_for(resolved.level))
        .with(terminal_layer)
        .with(file_layer);

    let _ = tracing::subscriber::set_global_default(subscriber);
}

/// 为单元测试初始化局部 tracing 默认派发器。
///
/// # 参数
/// - `settings`：全局配置
/// - `component`：当前测试模拟的组件
/// - `overrides`：日志覆盖参数
/// - `context`：当前测试的运行时上下文
/// - `terminal_sink`：终端输出缓冲区
///
/// # 返回值
/// - `tracing::dispatcher::DefaultGuard`：局部默认派发器守卫
pub fn init_tracing_for_test(
    settings: &Settings,
    component: LogComponent,
    overrides: &CliLogOverrides,
    context: RuntimeLogContext,
    terminal_sink: Arc<Mutex<Vec<u8>>>,
) -> tracing::dispatcher::DefaultGuard {
    let resolved = resolve_logging_options(settings, component, overrides);
    let terminal_layer = StructuredLogLayer::new_terminal(
        settings.logging.terminal_format,
        resolved.prefix_enabled,
        resolved.prefix_text.clone(),
        component,
        context.clone(),
        shared_writer(Box::new(VecWriter(terminal_sink))),
    );
    let file_layer = build_file_layer(settings, &resolved, component, &context);
    let subscriber = tracing_subscriber::registry()
        .with(env_filter_for(resolved.level))
        .with(terminal_layer)
        .with(file_layer);
    let dispatch = tracing::Dispatch::new(subscriber);
    tracing::dispatcher::set_default(&dispatch)
}

/// 结合配置文件位置解析日志文件路径。
///
/// # 参数
/// - `settings`：全局配置
/// - `configured`：配置中声明的文件路径
///
/// # 返回值
/// - `Option<PathBuf>`：解析后的日志文件路径
fn resolve_log_file_path(settings: &Settings, configured: &Option<String>) -> Option<PathBuf> {
    configured.as_ref().map(|value| {
        let config_path = std::env::var("MELO_CONFIG_PATH")
            .or_else(|_| std::env::var("MELO_CONFIG"))
            .map(PathBuf::from)
            .unwrap_or_else(|_| crate::core::config::paths::default_config_path());
        let _ = settings;
        crate::core::config::paths::resolve_from_config_dir(&config_path, Path::new(value))
    })
}

/// 按组件与配置决定最终文件日志路径。
///
/// # 参数
/// - `settings`：全局配置
/// - `component`：当前日志组件
/// - `resolved`：已解析的日志选项
///
/// # 返回值
/// - `Option<PathBuf>`：最终文件路径
fn effective_log_file_path(
    settings: &Settings,
    component: LogComponent,
    resolved: &ResolvedLoggingOptions,
) -> Option<PathBuf> {
    resolve_log_file_path(settings, &resolved.file_path).or_else(|| {
        if matches!(component, LogComponent::Daemon) && resolved.file_enabled {
            crate::daemon::registry::runtime_paths()
                .ok()
                .map(|paths| paths.log_file)
        } else {
            None
        }
    })
}

/// 构建可选的文件日志层。
///
/// # 参数
/// - `settings`：全局配置
/// - `resolved`：已解析的日志选项
/// - `component`：当前组件
/// - `context`：当前运行时上下文
///
/// # 返回值
/// - `Option<StructuredLogLayer>`：存在文件目标时返回文件日志层
fn build_file_layer(
    settings: &Settings,
    resolved: &ResolvedLoggingOptions,
    component: LogComponent,
    context: &RuntimeLogContext,
) -> Option<StructuredLogLayer> {
    if !resolved.file_enabled {
        return None;
    }

    let path = effective_log_file_path(settings, component, resolved)?;
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .ok()?;

    Some(StructuredLogLayer::new_file(
        settings.logging.file_format,
        component,
        context.clone(),
        shared_writer(Box::new(file)),
    ))
}

/// 返回组件的稳定文本标签。
///
/// # 参数
/// - `component`：日志组件
///
/// # 返回值
/// - `&'static str`：组件标签
fn component_name(component: LogComponent) -> &'static str {
    match component {
        LogComponent::Cli => "cli",
        LogComponent::Daemon => "daemon",
    }
}

/// 返回当前 UTC 时间戳文本。
///
/// # 参数
/// - 无
///
/// # 返回值
/// - `String`：RFC3339 时间文本
fn timestamp_text() -> String {
    time::OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_string())
}

/// 把 writer 包装成线程安全共享对象。
///
/// # 参数
/// - `writer`：原始 writer
///
/// # 返回值
/// - `SharedWriter`：共享 writer
fn shared_writer(writer: Box<dyn Write + Send>) -> SharedWriter {
    Arc::new(Mutex::new(writer))
}

type SharedWriter = Arc<Mutex<Box<dyn Write + Send>>>;

#[derive(Clone)]
struct StructuredLogLayer {
    format: LoggingFormat,
    destination: LogDestination,
    component: LogComponent,
    context: RuntimeLogContext,
    writer: SharedWriter,
}

#[derive(Clone)]
enum LogDestination {
    Terminal {
        prefix_enabled: bool,
        prefix_text: String,
    },
    File,
}

impl StructuredLogLayer {
    /// 构造终端输出层。
    ///
    /// # 参数
    /// - `format`：终端输出格式
    /// - `prefix_enabled`：是否启用前缀
    /// - `prefix_text`：前缀文本
    /// - `component`：当前组件
    /// - `context`：运行时上下文
    /// - `writer`：输出目标
    ///
    /// # 返回值
    /// - `Self`：终端日志层
    fn new_terminal(
        format: LoggingFormat,
        prefix_enabled: bool,
        prefix_text: String,
        component: LogComponent,
        context: RuntimeLogContext,
        writer: SharedWriter,
    ) -> Self {
        Self {
            format,
            destination: LogDestination::Terminal {
                prefix_enabled,
                prefix_text,
            },
            component,
            context,
            writer,
        }
    }

    /// 构造文件输出层。
    ///
    /// # 参数
    /// - `format`：文件输出格式
    /// - `component`：当前组件
    /// - `context`：运行时上下文
    /// - `writer`：输出目标
    ///
    /// # 返回值
    /// - `Self`：文件日志层
    fn new_file(
        format: LoggingFormat,
        component: LogComponent,
        context: RuntimeLogContext,
        writer: SharedWriter,
    ) -> Self {
        Self {
            format,
            destination: LogDestination::File,
            component,
            context,
            writer,
        }
    }

    /// 将事件渲染为字符串行。
    ///
    /// # 参数
    /// - `event`：tracing 事件
    ///
    /// # 返回值
    /// - `String`：最终输出行
    fn render_line(&self, event: &Event<'_>) -> String {
        let metadata = event.metadata();
        let mut visitor = JsonFieldVisitor::default();
        event.record(&mut visitor);

        match (&self.destination, self.format) {
            (
                LogDestination::Terminal {
                    prefix_enabled,
                    prefix_text,
                },
                LoggingFormat::Pretty,
            ) => render_pretty_line(*prefix_enabled, prefix_text, &visitor.fields),
            (LogDestination::File, LoggingFormat::Pretty) => {
                render_pretty_line(false, "", &visitor.fields)
            }
            _ => serde_json::to_string(&build_json_record(
                self.component,
                &self.context,
                metadata.level().as_str(),
                metadata.target(),
                visitor.fields,
            ))
            .unwrap_or_else(|_| "{\"message\":\"logging_serialize_failed\"}".to_string()),
        }
    }
}

impl<S> Layer<S> for StructuredLogLayer
where
    S: Subscriber,
{
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        let line = self.render_line(event);
        if let Ok(mut writer) = self.writer.lock() {
            let _ = writer.write_all(line.as_bytes());
            let _ = writer.write_all(b"\n");
            let _ = writer.flush();
        }
    }
}

#[derive(Default)]
struct JsonFieldVisitor {
    fields: Map<String, Value>,
}

impl Visit for JsonFieldVisitor {
    fn record_bool(&mut self, field: &Field, value: bool) {
        self.fields
            .insert(field.name().to_string(), Value::Bool(value));
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        self.fields
            .insert(field.name().to_string(), Value::Number(value.into()));
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        self.fields
            .insert(field.name().to_string(), Value::Number(value.into()));
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        self.fields
            .insert(field.name().to_string(), Value::String(value.to_string()));
    }

    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        self.fields.insert(
            field.name().to_string(),
            Value::String(format!("{value:?}")),
        );
    }
}

/// 构造统一 JSON 日志记录。
///
/// # 参数
/// - `component`：当前组件
/// - `context`：运行时上下文
/// - `level`：日志等级文本
/// - `target`：tracing target
/// - `fields`：事件字段
///
/// # 返回值
/// - `Value`：结构化 JSON 记录
fn build_json_record(
    component: LogComponent,
    context: &RuntimeLogContext,
    level: &str,
    target: &str,
    fields: Map<String, Value>,
) -> Value {
    serde_json::json!({
        "timestamp": timestamp_text(),
        "level": level,
        "component": component_name(component),
        "target": target,
        "session_id": context.session_id,
        "command_id": context.command_id,
        "fields": fields,
    })
}

/// 渲染 pretty 终端日志行。
///
/// # 参数
/// - `prefix_enabled`：是否启用前缀
/// - `prefix_text`：前缀文本
/// - `fields`：事件字段
///
/// # 返回值
/// - `String`：人类可读日志行
fn render_pretty_line(
    prefix_enabled: bool,
    prefix_text: impl AsRef<str>,
    fields: &Map<String, Value>,
) -> String {
    let mut parts = Vec::new();
    if let Some(message) = fields.get("message").and_then(Value::as_str) {
        parts.push(message.to_string());
    }
    for (key, value) in fields {
        if key == "message" {
            continue;
        }
        parts.push(format!("{key}={}", format_value(value)));
    }

    let body = if parts.is_empty() {
        "event".to_string()
    } else {
        parts.join(" ")
    };

    if prefix_enabled {
        format!("[{}] {}", prefix_text.as_ref(), body)
    } else {
        body
    }
}

/// 把 JSON 值转成便于终端阅读的文本。
///
/// # 参数
/// - `value`：字段值
///
/// # 返回值
/// - `String`：格式化后的文本
fn format_value(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        _ => value.to_string(),
    }
}

/// 当前命令附着的 daemon 日志镜像句柄。
pub struct DaemonLogMirror {
    shutdown: Option<tokio::sync::oneshot::Sender<()>>,
    #[allow(dead_code)]
    join: tokio::task::JoinHandle<()>,
}

impl Drop for DaemonLogMirror {
    fn drop(&mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
    }
}

/// 把 daemon 日志文件增量镜像到当前 CLI 终端。
///
/// # 参数
/// - `path`：daemon 日志文件路径
/// - `prefix_enabled`：是否启用前缀
/// - `prefix_text`：镜像到终端时使用的前缀文本
///
/// # 返回值
/// - `DaemonLogMirror`：可随作用域释放的镜像句柄
pub fn attach_daemon_log_mirror(
    path: PathBuf,
    prefix_enabled: bool,
    prefix_text: String,
) -> DaemonLogMirror {
    let (shutdown_tx, mut shutdown_rx) = tokio::sync::oneshot::channel();
    let join = tokio::spawn(async move {
        let mut seen = std::fs::metadata(&path)
            .map(|metadata| metadata.len() as usize)
            .unwrap_or_default();

        loop {
            tokio::select! {
                _ = &mut shutdown_rx => break,
                _ = tokio::time::sleep(std::time::Duration::from_millis(150)) => {}
            }

            let contents = tokio::fs::read_to_string(&path).await.unwrap_or_default();
            if contents.len() <= seen {
                continue;
            }

            for line in contents[seen..].lines() {
                eprintln!(
                    "{}",
                    render_daemon_log_line(line, prefix_enabled, prefix_text.as_str())
                );
            }
            seen = contents.len();
        }
    });

    DaemonLogMirror {
        shutdown: Some(shutdown_tx),
        join,
    }
}

/// 解析当前配置下 daemon 应写入的日志文件路径。
///
/// # 参数
/// - `settings`：全局配置
///
/// # 返回值
/// - `PathBuf`：daemon 日志文件路径
pub fn daemon_log_path(settings: &Settings) -> PathBuf {
    let resolved =
        resolve_logging_options(settings, LogComponent::Daemon, &CliLogOverrides::default());
    effective_log_file_path(settings, LogComponent::Daemon, &resolved).unwrap_or_else(|| {
        crate::daemon::registry::runtime_paths()
            .map(|paths| paths.log_file)
            .unwrap_or_else(|_| PathBuf::from("daemon.log"))
    })
}

/// 根据当前上下文决定是否提示 daemon 覆盖范围受限。
///
/// # 参数
/// - `settings`：全局配置
/// - `overrides`：当前命令的日志覆盖项
/// - `daemon_already_running`：当前命令是否复用了已有 daemon
///
/// # 返回值
/// - `Option<&'static str>`：需要提示时返回稳定 notice 文本
pub fn daemon_override_notice(
    settings: &Settings,
    overrides: &CliLogOverrides,
    daemon_already_running: bool,
) -> Option<&'static str> {
    overrides.daemon_log_level.as_ref()?;
    if daemon_already_running {
        return Some("daemon_log_level_override_not_applied_to_running_daemon");
    }
    if !settings.logging.daemon.allow_runtime_level_override {
        return Some("daemon_log_level_override_disabled_by_config");
    }
    None
}

/// 把 daemon 文件日志行渲染成当前终端可读文本。
///
/// # 参数
/// - `line`：日志文件中的单行文本
/// - `prefix_enabled`：是否启用前缀
/// - `prefix_text`：前缀文本
///
/// # 返回值
/// - `String`：终端可读的日志行
fn render_daemon_log_line(line: &str, prefix_enabled: bool, prefix_text: &str) -> String {
    if let Ok(value) = serde_json::from_str::<Value>(line)
        && let Some(fields) = value.get("fields").and_then(Value::as_object)
    {
        return render_pretty_line(prefix_enabled, prefix_text, fields);
    }

    if prefix_enabled {
        format!("[{prefix_text}] {line}")
    } else {
        line.to_string()
    }
}

struct VecWriter(Arc<Mutex<Vec<u8>>>);

impl Write for VecWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let mut sink = self
            .0
            .lock()
            .map_err(|_| std::io::Error::other("terminal sink poisoned"))?;
        sink.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests;

use std::path::Path;

use minijinja::Environment;
use serde_json::Value;

use crate::core::config::settings::Settings;

/// 运行时模板键，统一描述 CLI / TUI 在扫描流程中的可覆写文案。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeTemplateKey {
    /// CLI 扫描启动提示。
    CliScanStart,
    /// CLI 切入 TUI 的交接提示。
    CliScanHandoff,
    /// TUI 扫描进行中提示。
    TuiScanActive,
    /// TUI 扫描完成提示。
    TuiScanDone,
    /// TUI 扫描失败提示。
    TuiScanFailed,
}

/// 运行时模板渲染器。
///
/// 设计上始终优先读取用户覆盖模板；如果覆盖模板解析或渲染失败，则自动回退到内置模板，
/// 这样运行时提示不会因为配置错误直接消失。
pub struct RuntimeTemplateRenderer {
    env: Environment<'static>,
}

impl RuntimeTemplateRenderer {
    /// 按模板键和上下文渲染最终运行时文案。
    ///
    /// # 参数
    /// - `settings`：全局配置，用于读取运行时模板覆盖值
    /// - `key`：当前要渲染的模板键
    /// - `context`：模板上下文
    ///
    /// # 返回值
    /// - `String`：最终可展示的运行时文案
    pub fn render(&self, settings: &Settings, key: RuntimeTemplateKey, context: Value) -> String {
        let builtin = self.builtin_template(key);

        self.override_template(settings, key)
            .and_then(|template| self.render_str(template, &context).ok())
            .or_else(|| self.render_str(builtin, &context).ok())
            .unwrap_or_else(|| builtin.to_string())
    }

    /// 读取某个模板键对应的用户覆盖模板。
    ///
    /// # 参数
    /// - `settings`：全局配置
    /// - `key`：模板键
    ///
    /// # 返回值
    /// - `Option<&str>`：用户配置的模板文本；未覆盖时返回 `None`
    fn override_template<'a>(
        &self,
        settings: &'a Settings,
        key: RuntimeTemplateKey,
    ) -> Option<&'a str> {
        match key {
            RuntimeTemplateKey::CliScanStart => {
                settings.templates.runtime.scan.cli_start.as_deref()
            }
            RuntimeTemplateKey::CliScanHandoff => {
                settings.templates.runtime.scan.cli_handoff.as_deref()
            }
            RuntimeTemplateKey::TuiScanActive => {
                settings.templates.runtime.scan.tui_active.as_deref()
            }
            RuntimeTemplateKey::TuiScanDone => settings.templates.runtime.scan.tui_done.as_deref(),
            RuntimeTemplateKey::TuiScanFailed => {
                settings.templates.runtime.scan.tui_failed.as_deref()
            }
        }
    }

    /// 返回模板键对应的内置模板。
    ///
    /// # 参数
    /// - `key`：模板键
    ///
    /// # 返回值
    /// - `&'static str`：内置模板文本
    fn builtin_template(&self, key: RuntimeTemplateKey) -> &'static str {
        match key {
            RuntimeTemplateKey::CliScanStart => "Scanning {{ source_label }}...",
            RuntimeTemplateKey::CliScanHandoff => "Launching TUI, background scan continues...",
            RuntimeTemplateKey::TuiScanActive => {
                "Scanning {{ source_label }}... {{ indexed_count }} / {{ discovered_count }} · {{ current_item_name }}"
            }
            RuntimeTemplateKey::TuiScanDone => "Scan complete: {{ queued_count }} tracks indexed",
            RuntimeTemplateKey::TuiScanFailed => "Scan failed: {{ error_message }}",
        }
    }

    /// 使用 MiniJinja 渲染字符串模板。
    ///
    /// # 参数
    /// - `template`：模板文本
    /// - `context`：模板上下文
    ///
    /// # 返回值
    /// - `Result<String, minijinja::Error>`：渲染结果
    fn render_str(&self, template: &str, context: &Value) -> Result<String, minijinja::Error> {
        self.env.render_str(template, context)
    }
}

impl Default for RuntimeTemplateRenderer {
    /// 创建带内置过滤器的运行时模板渲染器。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - `Self`：可直接用于渲染运行时模板的渲染器
    fn default() -> Self {
        let mut env = Environment::new();
        env.add_filter("basename", basename_filter);
        env.add_filter("truncate", truncate_filter);
        Self { env }
    }
}

/// 提取路径的基名，方便把长路径压缩成更适合实时提示的标签。
///
/// # 参数
/// - `value`：原始路径文本
///
/// # 返回值
/// - `String`：路径基名；无法解析时回退原始值
fn basename_filter(value: String) -> String {
    Path::new(&value)
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .map(ToString::to_string)
        .unwrap_or(value)
}

/// 按字符数截断字符串，并在需要时追加省略号。
///
/// # 参数
/// - `value`：原始文本
/// - `width`：最大字符宽度
///
/// # 返回值
/// - `String`：截断后的文本
fn truncate_filter(value: String, width: usize) -> String {
    if width == 0 {
        return String::new();
    }

    let chars: Vec<char> = value.chars().collect();
    if chars.len() <= width {
        return value;
    }

    if width == 1 {
        return "…".to_string();
    }

    let mut truncated = chars.into_iter().take(width - 1).collect::<String>();
    truncated.push('…');
    truncated
}

#[cfg(test)]
mod tests;

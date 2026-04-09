use crate::core::config::settings::{OrganizeRuleSettings, Settings};
use crate::domain::library::repository::OrganizeCandidate;

/// organize 预览结果。
#[derive(Debug, Clone)]
pub struct OrganizePreviewRow {
    /// 歌曲 ID。
    pub song_id: i64,
    /// 命中的规则名。
    pub rule_name: String,
    /// 原始路径。
    pub source_path: String,
    /// 目标路径。
    pub target_path: String,
}

/// 选择首条命中的 organize 规则。
///
/// # 参数
/// - `rules`：规则列表
/// - `song`：候选歌曲
///
/// # 返回
/// - `Option<&OrganizeRuleSettings>`：命中的规则
pub fn choose_rule<'a>(
    rules: &'a [OrganizeRuleSettings],
    song: &OrganizeCandidate,
) -> Option<&'a OrganizeRuleSettings> {
    let mut sorted = rules.iter().collect::<Vec<_>>();
    sorted.sort_by_key(|rule| std::cmp::Reverse(rule.priority));
    sorted.into_iter().find(|rule| {
        if let Some(static_playlist) = &rule.match_rule.static_playlist {
            song.static_playlists
                .iter()
                .any(|name| name == static_playlist)
        } else {
            true
        }
    })
}

/// 计算与歌曲同名的歌词 sidecar 目标。
///
/// # 参数
/// - `song_source`：原始歌曲路径
/// - `song_target`：目标歌曲路径
///
/// # 返回
/// - `Vec<(PathBuf, PathBuf)>`：需要一起移动的 sidecar 源路径和目标路径
pub fn sidecar_targets(
    song_source: &std::path::Path,
    song_target: &std::path::Path,
) -> Vec<(std::path::PathBuf, std::path::PathBuf)> {
    let mut moves = Vec::new();
    for ext in ["lrc", "txt"] {
        let source = song_source.with_extension(ext);
        if source.exists() {
            moves.push((source, song_target.with_extension(ext)));
        }
    }
    moves
}

/// 根据规则渲染目标文件路径。
///
/// # 参数
/// - `settings`：全局配置
/// - `rule`：命中的规则
/// - `song`：候选歌曲
///
/// # 返回
/// - `Result<String, Error>`：渲染后的绝对路径
pub fn render_target_path(
    settings: &Settings,
    rule: &OrganizeRuleSettings,
    song: &OrganizeCandidate,
) -> Result<String, minijinja::Error> {
    let mut env = minijinja::Environment::new();
    env.add_filter("sanitize", |value: String| {
        value
            .chars()
            .map(|ch| match ch {
                '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
                _ => ch,
            })
            .collect::<String>()
            .trim()
            .to_string()
    });

    let template = env.template_from_str(&rule.template)?;
    let relative = template.render(minijinja::context! {
        title => song.title.clone(),
        artist => song.artist.clone().unwrap_or_default(),
    })?;

    let extension = std::path::Path::new(&song.source_path)
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default();
    let base_dir = settings
        .library
        .organize
        .base_dir
        .clone()
        .unwrap_or_else(|| ".".to_string());
    Ok(std::path::Path::new(&base_dir)
        .join(format!("{relative}.{extension}"))
        .to_string_lossy()
        .into_owned())
}

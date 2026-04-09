use crate::domain::library::metadata::{LyricsSourceKind, SongMetadata};

/// 解析完成后的歌词选择结果。
#[derive(Debug, Clone)]
pub struct ResolvedLyrics {
    /// 最终歌词文本。
    pub text: String,
    /// 来源类型。
    pub source_kind: LyricsSourceKind,
    /// 来源路径。
    pub source_path: Option<String>,
    /// 歌词格式。
    pub format: String,
}

/// 设计稿约定的默认歌词优先级。
const DEFAULT_LYRICS_PRIORITY: [&str; 3] = ["sidecar_lrc", "sidecar_txt", "embedded"];

/// 查找与音频文件同名的歌词 sidecar。
///
/// # 参数
/// - `path`：音频文件路径
///
/// # 返回值
/// - `Option<(String, String, String)>`：依次为歌词路径、规范化歌词文本、歌词格式
pub fn find_sidecar_lyrics(path: &std::path::Path) -> Option<(String, String, String)> {
    for ext in ["lrc", "txt"] {
        if let Some(candidate) = find_sidecar_lyrics_with_extension(path, ext) {
            return Some(candidate);
        }
    }

    None
}

/// 根据扩展名查找歌词 sidecar。
///
/// # 参数
/// - `path`：音频文件路径
/// - `extension`：歌词扩展名
///
/// # 返回值
/// - `Option<(String, String, String)>`：依次为歌词路径、规范化歌词文本、歌词格式
pub fn find_sidecar_lyrics_with_extension(
    path: &std::path::Path,
    extension: &str,
) -> Option<(String, String, String)> {
    let stem = path.file_stem()?.to_string_lossy();
    let candidate = path.with_file_name(format!("{stem}.{extension}"));
    if !candidate.exists() {
        return None;
    }

    let raw = std::fs::read_to_string(&candidate).ok()?;
    let lyrics = if extension == "lrc" {
        raw.lines()
            .map(|line| line.rsplit(']').next().unwrap_or(line).trim())
            .filter(|line| !line.is_empty())
            .collect::<Vec<_>>()
            .join("\n")
    } else {
        raw.trim().to_string()
    };

    Some((
        candidate.to_string_lossy().into_owned(),
        lyrics,
        extension.to_string(),
    ))
}

/// 按设计稿默认优先级解析歌词来源。
///
/// # 参数
/// - `path`：音频文件路径
/// - `metadata`：读取器返回的原始元数据
///
/// # 返回值
/// - `Option<ResolvedLyrics>`：最终选择的歌词结果
pub fn resolve_lyrics(path: &std::path::Path, metadata: &SongMetadata) -> Option<ResolvedLyrics> {
    for source in DEFAULT_LYRICS_PRIORITY {
        match source {
            "sidecar_lrc" => {
                if let Some((source_path, text, format)) =
                    find_sidecar_lyrics_with_extension(path, "lrc")
                {
                    return Some(ResolvedLyrics {
                        text,
                        source_kind: LyricsSourceKind::Sidecar,
                        source_path: Some(source_path),
                        format,
                    });
                }
            }
            "sidecar_txt" => {
                if let Some((source_path, text, format)) =
                    find_sidecar_lyrics_with_extension(path, "txt")
                {
                    return Some(ResolvedLyrics {
                        text,
                        source_kind: LyricsSourceKind::Sidecar,
                        source_path: Some(source_path),
                        format,
                    });
                }
            }
            "embedded" => {
                if let Some(text) = metadata.lyrics.clone() {
                    return Some(ResolvedLyrics {
                        text,
                        source_kind: metadata.lyrics_source_kind.clone(),
                        source_path: None,
                        format: metadata
                            .lyrics_format
                            .clone()
                            .unwrap_or_else(|| "plain".to_string()),
                    });
                }
            }
            _ => {}
        }
    }

    None
}

/// 在音频文件所在目录查找封面 sidecar。
///
/// # 参数
/// - `path`：音频文件路径
///
/// # 返回值
/// - `Option<PathBuf>`：找到的封面路径
pub fn find_cover(path: &std::path::Path) -> Option<std::path::PathBuf> {
    let dir = path.parent()?;
    for name in ["cover", "folder", "front", "album", "art"] {
        for ext in ["jpg", "jpeg", "png", "webp"] {
            let candidate = dir.join(format!("{name}.{ext}"));
            if candidate.exists() {
                return Some(candidate);
            }
        }
    }

    None
}

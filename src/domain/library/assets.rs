/// 查找与音频文件同名的歌词 sidecar。
///
/// # 参数
/// - `path`：音频文件路径
///
/// # 返回
/// - `Option<(String, String, String)>`：依次为歌词路径、规范化歌词文本、歌词格式
pub fn find_sidecar_lyrics(path: &std::path::Path) -> Option<(String, String, String)> {
    let stem = path.file_stem()?.to_string_lossy();
    for ext in ["lrc", "txt"] {
        let candidate = path.with_file_name(format!("{stem}.{ext}"));
        if candidate.exists() {
            let raw = std::fs::read_to_string(&candidate).ok()?;
            let lyrics = if ext == "lrc" {
                raw.lines()
                    .map(|line| line.rsplit(']').next().unwrap_or(line).trim())
                    .filter(|line| !line.is_empty())
                    .collect::<Vec<_>>()
                    .join("\n")
            } else {
                raw.trim().to_string()
            };

            return Some((
                candidate.to_string_lossy().into_owned(),
                lyrics,
                ext.to_string(),
            ));
        }
    }

    None
}

/// 在音频文件所在目录查找封面 sidecar。
///
/// # 参数
/// - `path`：音频文件路径
///
/// # 返回
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

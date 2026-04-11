use std::path::Path;

/// 判断给定路径是否是支持的音频文件。
///
/// # 参数
/// - `path`：待判断路径
///
/// # 返回值
/// - `bool`：是否支持
pub fn is_supported_audio_path(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.to_ascii_lowercase())
            .as_deref(),
        Some("flac" | "mp3" | "ogg" | "wav" | "m4a" | "aac")
    )
}

#[cfg(test)]
mod tests;

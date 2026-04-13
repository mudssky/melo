use crate::core::model::track_content::LyricLine;

/// 将原始歌词文本解析为可按时间轴消费的歌词行。
///
/// # 参数
/// - `raw`：原始歌词文本，支持简单 LRC 标签
///
/// # 返回值
/// - `Vec<LyricLine>`：按时间顺序排列的歌词行列表
pub fn parse_lyrics_timeline(raw: &str) -> Vec<LyricLine> {
    let mut parsed = Vec::new();

    for (plain_index, raw_line) in raw.lines().enumerate() {
        let mut matched = false;
        if let Some(rest) = raw_line.strip_prefix('[')
            && let Some((tag, text)) = rest.split_once(']')
            && let Some((minutes_text, seconds_text)) = tag.split_once(':')
            && let (Ok(minutes), Ok(seconds)) =
                (minutes_text.parse::<f64>(), seconds_text.parse::<f64>())
        {
            parsed.push(LyricLine {
                timestamp_seconds: minutes * 60.0 + seconds,
                text: text.to_string(),
            });
            matched = true;
        }

        if !matched {
            parsed.push(LyricLine {
                timestamp_seconds: plain_index as f64,
                text: raw_line.to_string(),
            });
        }
    }

    parsed.sort_by(|left, right| {
        left.timestamp_seconds
            .partial_cmp(&right.timestamp_seconds)
            .unwrap()
    });
    parsed
}

#[cfg(test)]
mod tests;

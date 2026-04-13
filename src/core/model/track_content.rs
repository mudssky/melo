use serde::{Deserialize, Serialize};

/// 当前歌曲可供终端展示的封面摘要。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtworkSummary {
    /// 封面来源类型。
    pub source_kind: String,
    /// 封面来源路径。
    pub source_path: Option<String>,
    /// 终端环境下的摘要文案。
    pub terminal_summary: String,
}

/// 解析后的单行歌词时间轴。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LyricLine {
    /// 该行歌词对应的秒数。
    pub timestamp_seconds: f64,
    /// 该行歌词文本。
    pub text: String,
}

/// 当前歌曲的低频内容快照。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrackContentSnapshot {
    /// 当前歌曲 ID。
    pub song_id: i64,
    /// 当前歌曲标题。
    pub title: String,
    /// 当前歌曲时长秒数。
    pub duration_seconds: Option<f64>,
    /// 当前歌曲封面摘要。
    pub artwork: Option<ArtworkSummary>,
    /// 当前歌曲解析后的歌词时间轴。
    pub lyrics: Vec<LyricLine>,
    /// 当前内容版本令牌。
    pub refresh_token: String,
}

impl TrackContentSnapshot {
    /// 根据当前播放位置返回应该高亮的歌词行索引。
    ///
    /// # 参数
    /// - `position_seconds`：当前播放秒数
    ///
    /// # 返回值
    /// - `Option<usize>`：命中时返回歌词行索引
    pub fn current_lyric_index(&self, position_seconds: f64) -> Option<usize> {
        self.lyrics
            .iter()
            .enumerate()
            .rev()
            .find(|(_, line)| line.timestamp_seconds <= position_seconds)
            .map(|(index, _)| index)
    }
}

#[cfg(test)]
mod tests;

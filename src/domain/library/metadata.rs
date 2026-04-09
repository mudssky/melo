use std::path::Path;

use crate::core::error::MeloResult;

/// 歌词来源类型。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LyricsSourceKind {
    None,
    Embedded,
    Sidecar,
}

impl LyricsSourceKind {
    /// 返回可写入数据库的字符串值。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - `&'static str`：数据库存储值
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Embedded => "embedded",
            Self::Sidecar => "sidecar",
        }
    }
}

/// 内嵌封面数据。
#[derive(Debug, Clone)]
pub struct EmbeddedArtwork {
    /// 图片 MIME 类型。
    pub mime: Option<String>,
    /// 图片二进制数据。
    pub bytes: Vec<u8>,
}

/// 扫描阶段读取到的歌曲元数据。
#[derive(Debug, Clone)]
pub struct SongMetadata {
    /// 标题。
    pub title: String,
    /// 艺术家。
    pub artist: Option<String>,
    /// 专辑名。
    pub album: Option<String>,
    /// 曲目序号。
    pub track_no: Option<u32>,
    /// 碟片序号。
    pub disc_no: Option<u32>,
    /// 时长（秒）。
    pub duration_seconds: Option<f64>,
    /// 流派。
    pub genre: Option<String>,
    /// 歌词文本。
    pub lyrics: Option<String>,
    /// 歌词来源。
    pub lyrics_source_kind: LyricsSourceKind,
    /// 歌词格式。
    pub lyrics_format: Option<String>,
    /// 内嵌封面。
    pub embedded_artwork: Option<EmbeddedArtwork>,
    /// 音频格式。
    pub format: Option<String>,
    /// 比特率。
    pub bitrate: Option<u32>,
    /// 采样率。
    pub sample_rate: Option<u32>,
    /// 位深。
    pub bit_depth: Option<u32>,
    /// 声道数。
    pub channels: Option<u32>,
}

/// 元数据读取器接口，便于切换为真实实现或测试替身。
pub trait MetadataReader: Send + Sync {
    /// 从音频文件读取元数据。
    ///
    /// # 参数
    /// - `path`：音频文件路径
    ///
    /// # 返回值
    /// - `MeloResult<SongMetadata>`：读取到的元数据
    fn read(&self, path: &Path) -> MeloResult<SongMetadata>;
}

/// 空实现读取器，仅用于不需要真实扫描的测试场景。
pub struct NullMetadataReader;

impl MetadataReader for NullMetadataReader {
    /// 返回一个明确的占位实现错误。
    ///
    /// # 参数
    /// - `_path`：音频文件路径
    ///
    /// # 返回值
    /// - `MeloResult<SongMetadata>`：占位实现始终返回错误
    fn read(&self, _path: &Path) -> MeloResult<SongMetadata> {
        Err(crate::core::error::MeloError::Message(
            "NullMetadataReader 不能用于真实扫描".to_string(),
        ))
    }
}

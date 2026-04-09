use lofty::prelude::*;
use lofty::probe::Probe;
use lofty::tag::Tag;

use crate::core::error::{MeloError, MeloResult};
use crate::domain::library::metadata::{
    EmbeddedArtwork, LyricsSourceKind, MetadataReader, SongMetadata,
};

/// 基于 `Lofty` 的真实元数据读取器。
pub struct LoftyMetadataReader;

impl LoftyMetadataReader {
    /// 从标签中提取第一张内嵌封面。
    ///
    /// # 参数
    /// - `tag`：Lofty 标签对象
    ///
    /// # 返回值
    /// - `Option<EmbeddedArtwork>`：提取到的封面数据
    fn embedded_artwork(tag: &Tag) -> Option<EmbeddedArtwork> {
        let picture = tag.pictures().first()?;
        Some(EmbeddedArtwork {
            mime: picture
                .mime_type()
                .map(|mime_type: &lofty::picture::MimeType| mime_type.as_str().to_string()),
            bytes: picture.data().to_vec(),
        })
    }

    /// 根据路径推断音频格式字符串。
    ///
    /// # 参数
    /// - `path`：音频文件路径
    ///
    /// # 返回值
    /// - `Option<String>`：小写格式字符串
    fn format_from_path(path: &std::path::Path) -> Option<String> {
        path.extension()
            .and_then(|extension| extension.to_str())
            .map(|extension| extension.to_lowercase())
    }
}

impl MetadataReader for LoftyMetadataReader {
    /// 从音频文件读取真实元数据。
    ///
    /// # 参数
    /// - `path`：音频文件路径
    ///
    /// # 返回值
    /// - `MeloResult<SongMetadata>`：解析后的歌曲元数据
    fn read(&self, path: &std::path::Path) -> MeloResult<SongMetadata> {
        let tagged_file = Probe::open(path)
            .map_err(|err| MeloError::Message(err.to_string()))?
            .read()
            .map_err(|err| MeloError::Message(err.to_string()))?;

        let tag = tagged_file
            .primary_tag()
            .or_else(|| tagged_file.first_tag());
        let lyrics = tag
            .and_then(|tag| tag.get_string(ItemKey::UnsyncLyrics))
            .or_else(|| tag.and_then(|tag| tag.get_string(ItemKey::Lyrics)))
            .map(|value| value.to_string());
        let properties = tagged_file.properties();
        let title = tag
            .and_then(|tag| tag.title().map(|value| value.to_string()))
            .or_else(|| {
                path.file_stem()
                    .and_then(|stem| stem.to_str())
                    .map(|stem| stem.to_string())
            })
            .ok_or_else(|| MeloError::Message("无法为音频文件解析标题".to_string()))?;

        Ok(SongMetadata {
            title,
            artist: tag.and_then(|tag| tag.artist().map(|value| value.to_string())),
            album: tag.and_then(|tag| tag.album().map(|value| value.to_string())),
            track_no: tag.and_then(|tag| tag.track()),
            disc_no: tag.and_then(|tag| tag.disk()),
            duration_seconds: {
                let duration = properties.duration();
                let seconds = duration.as_secs_f64();
                if seconds > 0.0 { Some(seconds) } else { None }
            },
            genre: tag.and_then(|tag| tag.genre().map(|value| value.to_string())),
            lyrics: lyrics.clone(),
            lyrics_source_kind: if lyrics.is_some() {
                LyricsSourceKind::Embedded
            } else {
                LyricsSourceKind::None
            },
            lyrics_format: lyrics.map(|_| "plain".to_string()),
            embedded_artwork: tag.and_then(Self::embedded_artwork),
            format: Self::format_from_path(path),
            bitrate: properties.audio_bitrate(),
            sample_rate: properties.sample_rate(),
            bit_depth: properties.bit_depth().map(u32::from),
            channels: properties.channels().map(u32::from),
        })
    }
}

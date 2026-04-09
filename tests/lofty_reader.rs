use std::path::{Path, PathBuf};

use lofty::config::WriteOptions;
use lofty::picture::{MimeType, Picture, PictureType};
use lofty::prelude::*;
use lofty::probe::Probe;
use melo::domain::library::lofty_reader::LoftyMetadataReader;
use melo::domain::library::metadata::{LyricsSourceKind, MetadataReader};
use tempfile::tempdir;

/// 将仓库内的最小 MP3 fixture 复制到临时目录并写入可预测的标签。
///
/// # 参数
/// - `target`：目标文件路径
///
/// # 返回值
/// - `Result<(), Box<dyn std::error::Error>>`：写入结果
fn prepare_tagged_fixture(target: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/full_test.mp3");
    std::fs::copy(source, target)?;

    let mut tagged_file = Probe::open(target)?.read()?;
    let tag = if let Some(tag) = tagged_file.primary_tag_mut() {
        tag
    } else {
        tagged_file
            .first_tag_mut()
            .expect("fixture 必须包含可写标签")
    };

    tag.set_title("Blue Bird".to_string());
    tag.set_artist("Ikimono-gakari".to_string());
    tag.set_album("Blue Bird".to_string());
    tag.set_genre("J-Pop".to_string());
    tag.set_track(1);
    tag.set_disk(1);
    tag.insert_text(ItemKey::UnsyncLyrics, "fly high".to_string());
    tag.push_picture(
        Picture::unchecked(vec![0xFF, 0xD8, 0xFF, 0xD9])
            .pic_type(PictureType::CoverFront)
            .mime_type(MimeType::Jpeg)
            .description("Front Cover")
            .build(),
    );
    tag.save_to_path(target, WriteOptions::default())?;
    Ok(())
}

#[test]
fn lofty_reader_extracts_title_lyrics_format_and_embedded_artwork() {
    let temp = tempdir().unwrap();
    let song_path = temp.path().join("blue-bird.mp3");
    prepare_tagged_fixture(&song_path).unwrap();

    let reader = LoftyMetadataReader;
    let metadata = reader.read(&song_path).unwrap();

    assert_eq!(metadata.title, "Blue Bird");
    assert_eq!(metadata.artist.as_deref(), Some("Ikimono-gakari"));
    assert_eq!(metadata.album.as_deref(), Some("Blue Bird"));
    assert_eq!(metadata.track_no, Some(1));
    assert_eq!(metadata.disc_no, Some(1));
    assert_eq!(metadata.genre.as_deref(), Some("J-Pop"));
    assert_eq!(metadata.lyrics.as_deref(), Some("fly high"));
    assert_eq!(metadata.lyrics_source_kind, LyricsSourceKind::Embedded);
    assert_eq!(metadata.lyrics_format.as_deref(), Some("plain"));
    assert_eq!(metadata.format.as_deref(), Some("mp3"));
    assert_eq!(
        metadata
            .embedded_artwork
            .as_ref()
            .and_then(|artwork| artwork.mime.as_deref()),
        Some("image/jpeg")
    );
    assert!(metadata.duration_seconds.unwrap_or_default() > 0.0);
    assert!(metadata.channels.unwrap_or_default() > 0);
}

use std::path::{Path, PathBuf};
use std::sync::Arc;

use lofty::config::WriteOptions;
use lofty::picture::{MimeType, Picture, PictureType};
use lofty::prelude::*;
use lofty::probe::Probe;
use melo::core::config::settings::Settings;
use melo::core::db::bootstrap::DatabaseBootstrap;
use melo::domain::library::lofty_reader::LoftyMetadataReader;
use melo::domain::library::service::LibraryService;
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
    tag.insert_text(ItemKey::UnsyncLyrics, "embedded words".to_string());
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

#[tokio::test]
async fn scan_prefers_sidecar_lyrics_and_cover_over_embedded_metadata() {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("melo.db");
    let music_dir = temp.path().join("music");
    std::fs::create_dir_all(&music_dir).unwrap();

    let song_path = music_dir.join("blue-bird.mp3");
    let lrc_path = music_dir.join("blue-bird.lrc");
    let cover_path = music_dir.join("cover.jpg");

    prepare_tagged_fixture(&song_path).unwrap();
    std::fs::write(&lrc_path, "[00:01.00]sidecar words").unwrap();
    std::fs::write(&cover_path, b"jpeg").unwrap();

    let settings = Settings::for_test(db_path);
    DatabaseBootstrap::new(&settings).init().await.unwrap();

    let service = LibraryService::new(settings, Arc::new(LoftyMetadataReader));
    service.scan_paths(&[music_dir]).await.unwrap();

    let songs = service.list_songs().await.unwrap();
    assert_eq!(songs.len(), 1);
    assert_eq!(songs[0].title, "Blue Bird");
    assert_eq!(songs[0].lyrics.as_deref(), Some("sidecar words"));
    assert_eq!(songs[0].lyrics_source_kind, "sidecar");

    let artwork = service
        .artwork_for_song(songs[0].id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(artwork.source_kind, "sidecar");
    assert!(artwork.source_path.unwrap().ends_with("cover.jpg"));

    let track_content = service.track_content(songs[0].id).await.unwrap();
    assert_eq!(
        track_content
            .artwork
            .as_ref()
            .map(|artwork| artwork.source_kind.as_str()),
        Some("sidecar")
    );
    assert!(
        track_content
            .artwork
            .as_ref()
            .and_then(|artwork| artwork.source_path.as_deref())
            .is_some_and(|path| path.ends_with("cover.jpg"))
    );
    assert!(
        track_content
            .artwork
            .as_ref()
            .is_some_and(|artwork| artwork.terminal_summary.contains("cover.jpg"))
    );
}

#[tokio::test]
async fn track_content_reports_embedded_artwork_without_sidecar_cover() {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("melo.db");
    let music_dir = temp.path().join("music");
    std::fs::create_dir_all(&music_dir).unwrap();

    let song_path = music_dir.join("blue-bird.mp3");
    prepare_tagged_fixture(&song_path).unwrap();

    let settings = Settings::for_test(db_path);
    DatabaseBootstrap::new(&settings).init().await.unwrap();

    let service = LibraryService::new(settings, Arc::new(LoftyMetadataReader));
    service.scan_paths(&[music_dir]).await.unwrap();

    let songs = service.list_songs().await.unwrap();
    let track_content = service.track_content(songs[0].id).await.unwrap();

    assert_eq!(
        track_content
            .artwork
            .as_ref()
            .map(|artwork| artwork.source_kind.as_str()),
        Some("embedded")
    );
    assert_eq!(
        track_content
            .artwork
            .as_ref()
            .map(|artwork| artwork.terminal_summary.as_str()),
        Some("封面来自音频元数据")
    );
}

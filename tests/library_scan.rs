use std::sync::Arc;

use melo::core::config::settings::Settings;
use melo::core::db::bootstrap::DatabaseBootstrap;
use melo::domain::library::metadata::{
    EmbeddedArtwork, LyricsSourceKind, MetadataReader, SongMetadata,
};
use melo::domain::library::service::LibraryService;
use tempfile::tempdir;

struct FakeReader;

impl MetadataReader for FakeReader {
    fn read(&self, _path: &std::path::Path) -> melo::core::error::MeloResult<SongMetadata> {
        Ok(SongMetadata {
            title: "Blue Bird".into(),
            artist: Some("Ikimono-gakari".into()),
            album: Some("Blue Bird".into()),
            track_no: Some(1),
            disc_no: Some(1),
            duration_seconds: Some(212.0),
            genre: Some("J-Pop".into()),
            lyrics: Some("fly high".into()),
            lyrics_source_kind: LyricsSourceKind::Embedded,
            lyrics_format: Some("plain".into()),
            embedded_artwork: Some(EmbeddedArtwork {
                mime: Some("image/jpeg".into()),
                bytes: vec![1, 2, 3, 4],
            }),
            format: Some("flac".into()),
            bitrate: Some(900_000),
            sample_rate: Some(48_000),
            bit_depth: Some(24),
            channels: Some(2),
        })
    }
}

#[tokio::test]
async fn scan_inserts_song_and_sidecar_lyrics() {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("melo.db");
    let music_dir = temp.path().join("music");
    std::fs::create_dir_all(&music_dir).unwrap();

    let song_path = music_dir.join("blue-bird.flac");
    let lrc_path = music_dir.join("blue-bird.lrc");
    let cover_path = music_dir.join("cover.jpg");

    std::fs::write(&song_path, b"fake-audio").unwrap();
    std::fs::write(&lrc_path, "[00:01.00]fly high").unwrap();
    std::fs::write(&cover_path, b"jpeg").unwrap();

    let settings = Settings::for_test(db_path);
    DatabaseBootstrap::new(&settings).init().await.unwrap();

    let service = LibraryService::new(settings, Arc::new(FakeReader));
    service.scan_paths(&[music_dir]).await.unwrap();

    let songs = service.list_songs().await.unwrap();
    assert_eq!(songs.len(), 1);
    assert_eq!(songs[0].title, "Blue Bird");
    assert_eq!(songs[0].lyrics.as_deref(), Some("fly high"));
    assert_eq!(songs[0].lyrics_source_kind, "sidecar");

    let artwork = service
        .artwork_for_song(songs[0].id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(artwork.source_kind, "sidecar");
    assert!(artwork.source_path.unwrap().ends_with("cover.jpg"));
}

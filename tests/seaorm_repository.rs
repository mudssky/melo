use melo::core::config::settings::Settings;
use melo::core::db::bootstrap::DatabaseBootstrap;
use melo::domain::library::metadata::{LyricsSourceKind, SongMetadata};
use melo::domain::library::repository::LibraryRepository;
use melo::domain::playlist::repository::PlaylistRepository;
use tempfile::tempdir;

#[tokio::test]
async fn library_repository_updates_existing_song_and_playlist_entries() {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("melo.db");
    let music_dir = temp.path().join("music");
    std::fs::create_dir_all(&music_dir).unwrap();

    let song_path = music_dir.join("blue-bird.flac");
    let cover_path = music_dir.join("cover.jpg");
    std::fs::write(&song_path, b"fake-audio-v1").unwrap();
    std::fs::write(&cover_path, b"jpeg").unwrap();

    let settings = Settings::for_test(db_path);
    DatabaseBootstrap::new(&settings).init().await.unwrap();

    let library_repository = LibraryRepository::new(settings.clone());
    let playlist_repository = PlaylistRepository::new(settings);

    let first_metadata = SongMetadata {
        title: "Blue Bird".to_string(),
        artist: Some("Ikimono-gakari".to_string()),
        album: Some("Blue Bird".to_string()),
        track_no: Some(1),
        disc_no: Some(1),
        duration_seconds: Some(212.0),
        genre: Some("J-Pop".to_string()),
        lyrics: Some("fly high".to_string()),
        lyrics_source_kind: LyricsSourceKind::Sidecar,
        lyrics_format: Some("lrc".to_string()),
        embedded_artwork: None,
        format: Some("flac".to_string()),
        bitrate: Some(900_000),
        sample_rate: Some(48_000),
        bit_depth: Some(24),
        channels: Some(2),
    };
    let first_song_id = library_repository
        .upsert_song(
            &song_path,
            &first_metadata,
            Some(song_path.with_extension("lrc").to_string_lossy().as_ref()),
            Some(&cover_path),
        )
        .await
        .unwrap();

    std::fs::write(&song_path, b"fake-audio-v2").unwrap();
    let second_metadata = SongMetadata {
        title: "Blue Bird (Live)".to_string(),
        lyrics: Some("fly higher".to_string()),
        ..first_metadata
    };
    let second_song_id = library_repository
        .upsert_song(
            &song_path,
            &second_metadata,
            Some(song_path.with_extension("txt").to_string_lossy().as_ref()),
            None,
        )
        .await
        .unwrap();

    assert_eq!(first_song_id, second_song_id);

    let songs = library_repository.list_songs().await.unwrap();
    assert_eq!(songs.len(), 1);
    assert_eq!(songs[0].title, "Blue Bird (Live)");
    assert_eq!(songs[0].lyrics.as_deref(), Some("fly higher"));
    assert_eq!(songs[0].lyrics_source_kind, "sidecar");

    playlist_repository
        .create_static("Favorites", Some("best songs"))
        .await
        .unwrap();
    playlist_repository
        .add_songs("Favorites", &[first_song_id])
        .await
        .unwrap();

    let playlists = playlist_repository.list_static().await.unwrap();
    assert_eq!(playlists.len(), 1);
    assert_eq!(playlists[0].name, "Favorites");
    assert_eq!(playlists[0].count, 1);

    let preview = playlist_repository
        .preview_static("Favorites")
        .await
        .unwrap();
    assert_eq!(preview.len(), 1);
    assert_eq!(preview[0].id, first_song_id);
    assert_eq!(preview[0].title, "Blue Bird (Live)");
}

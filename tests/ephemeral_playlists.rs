use std::collections::BTreeSet;

use rusqlite::params;

fn seed_many_songs(harness: &melo::test_support::TestHarness, total: usize) {
    let mut conn = rusqlite::Connection::open(harness.settings.database.path.as_std_path())
        .expect("必须能连接测试数据库");
    let tx = conn.transaction().expect("必须能开启事务");
    tx.execute(
        "INSERT INTO artists (name, sort_name, search_name, created_at, updated_at)
         VALUES (?1, ?1, lower(?1), datetime('now'), datetime('now'))",
        ["Batch Artist"],
    )
    .expect("必须能插入 artist");
    let artist_id = tx.last_insert_rowid();
    tx.execute(
        "INSERT INTO albums (title, album_artist_id, year, source_dir, created_at, updated_at)
         VALUES (?1, ?2, ?3, NULL, datetime('now'), datetime('now'))",
        params!["Batch Album", artist_id, 2026],
    )
    .expect("必须能插入 album");
    let album_id = tx.last_insert_rowid();

    for index in 0..total {
        let title = format!("Song {index:04}");
        tx.execute(
            "INSERT INTO songs (
                path,
                title,
                artist_id,
                album_id,
                track_no,
                disc_no,
                duration_seconds,
                genre,
                lyrics,
                lyrics_source_kind,
                lyrics_source_path,
                lyrics_format,
                lyrics_updated_at,
                format,
                bitrate,
                sample_rate,
                bit_depth,
                channels,
                file_size,
                file_mtime,
                added_at,
                scanned_at,
                updated_at
            ) VALUES (
                ?1,
                ?2,
                ?3,
                ?4,
                1,
                1,
                180.0,
                NULL,
                NULL,
                'none',
                NULL,
                NULL,
                NULL,
                'flac',
                NULL,
                NULL,
                NULL,
                NULL,
                0,
                0,
                datetime('now'),
                datetime('now'),
                datetime('now')
            )",
            params![format!("D:/Music/{title}.flac"), title, artist_id, album_id],
        )
        .expect("必须能插入 song");
    }

    tx.commit().expect("必须能提交事务");
}

#[tokio::test]
async fn upsert_ephemeral_reuses_same_source_key() {
    let harness = melo::test_support::TestHarness::new().await;
    harness
        .seed_song("Blue Bird", "Ikimono-gakari", "Blue Bird", 2008)
        .await;
    harness
        .seed_song("Brave Shine", "Aimer", "Brave Shine", 2015)
        .await;
    let playlist_service = harness.playlist_service();

    let first = playlist_service
        .upsert_ephemeral(
            "D:/Music/Anime",
            "path_dir",
            "D:/Music/Anime",
            true,
            None,
            &[1, 2],
        )
        .await
        .unwrap();
    let second = playlist_service
        .upsert_ephemeral(
            "D:/Music/Anime",
            "path_dir",
            "D:/Music/Anime",
            true,
            None,
            &[1, 2],
        )
        .await
        .unwrap();

    assert_eq!(first.id, second.id);
}

#[tokio::test]
async fn promote_ephemeral_turns_it_into_static_playlist() {
    let harness = melo::test_support::TestHarness::new().await;
    harness
        .seed_song("Blue Bird", "Ikimono-gakari", "Blue Bird", 2008)
        .await;
    let playlist_service = harness.playlist_service();

    playlist_service
        .upsert_ephemeral(
            "blue-bird.mp3",
            "path_file",
            "D:/Music/blue-bird.mp3",
            false,
            None,
            &[1],
        )
        .await
        .unwrap();

    playlist_service
        .promote_ephemeral("D:/Music/blue-bird.mp3", "Single Favorites")
        .await
        .unwrap();

    let playlists = playlist_service.list_all().await.unwrap();
    assert!(
        playlists
            .iter()
            .any(|playlist| playlist.name == "Single Favorites" && playlist.kind == "static")
    );
}

#[tokio::test]
async fn cleanup_expired_removes_only_elapsed_ephemeral_playlists() {
    let harness = melo::test_support::TestHarness::new().await;
    harness
        .seed_song("Blue Bird", "Ikimono-gakari", "Blue Bird", 2008)
        .await;
    let playlist_service = harness.playlist_service();

    playlist_service
        .upsert_ephemeral(
            "Expired Dir",
            "cwd_dir",
            "D:/Music/Expired",
            true,
            Some("2000-01-01T00:00:00Z"),
            &[1],
        )
        .await
        .unwrap();

    let deleted = playlist_service
        .cleanup_expired("2026-04-11T00:00:00Z")
        .await
        .unwrap();
    assert_eq!(deleted, 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn upsert_ephemeral_does_not_expose_partially_rebuilt_playlist() {
    let harness = melo::test_support::TestHarness::new().await;
    let playlist_service = harness.playlist_service();
    let playlist_name = "D:/Music/Anime";
    let initial_len = 600usize;
    let final_len = 2400usize;

    seed_many_songs(&harness, final_len);

    let initial_song_ids = (1..=initial_len as i64).collect::<Vec<_>>();
    playlist_service
        .upsert_ephemeral(
            playlist_name,
            "path_dir",
            playlist_name,
            true,
            None,
            &initial_song_ids,
        )
        .await
        .unwrap();

    let writer_service = playlist_service.clone();
    let final_song_ids = (1..=final_len as i64).collect::<Vec<_>>();
    let writer = tokio::spawn(async move {
        writer_service
            .upsert_ephemeral(
                playlist_name,
                "path_dir",
                playlist_name,
                true,
                None,
                &final_song_ids,
            )
            .await
            .unwrap();
    });

    let mut observed_lengths = BTreeSet::new();
    while !writer.is_finished() {
        let preview_len = playlist_service.preview(playlist_name).await.unwrap().len();
        observed_lengths.insert(preview_len);
        if preview_len != initial_len && preview_len != final_len {
            break;
        }
        tokio::task::yield_now().await;
    }

    writer.await.unwrap();
    observed_lengths.insert(playlist_service.preview(playlist_name).await.unwrap().len());

    assert!(
        observed_lengths
            .iter()
            .all(|len| *len == initial_len || *len == final_len),
        "临时歌单刷新期间暴露了半更新状态: {observed_lengths:?}"
    );
}

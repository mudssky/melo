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

#[tokio::test]
async fn playlist_service_combines_static_and_smart_playlists() {
    let harness = melo::test_support::TestHarness::new().await;
    let playlist_service = harness.playlist_service();

    harness
        .seed_song("Brave Shine", "Aimer", "Brave Shine", 2015)
        .await;
    harness
        .seed_song("I beg you", "Aimer", "Penny Rain", 2019)
        .await;

    playlist_service
        .create_static("Favorites", None)
        .await
        .unwrap();
    playlist_service.add_songs("Favorites", &[1]).await.unwrap();

    harness
        .write_config(
            r#"
        [playlists.smart.aimer]
        query = 'artist:"Aimer"'
        "#,
        )
        .await;

    let playlists = playlist_service.list_all().await.unwrap();

    assert_eq!(
        playlists
            .iter()
            .find(|playlist| playlist.name == "Favorites")
            .unwrap()
            .kind,
        "static"
    );
    assert_eq!(
        playlists
            .iter()
            .find(|playlist| playlist.name == "aimer")
            .unwrap()
            .kind,
        "smart"
    );

    let preview = playlist_service.preview("aimer").await.unwrap();
    assert_eq!(preview.len(), 2);
}

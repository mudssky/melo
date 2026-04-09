#[tokio::test]
async fn organize_preview_uses_first_matching_rule_and_moves_sidecar_lyrics() {
    let harness = melo::test_support::TestHarness::new().await;

    let song_path = harness.write_song_file("incoming/blue-bird.flac").await;
    harness
        .write_file("incoming/blue-bird.lrc", "[00:01.00]fly high")
        .await;
    harness
        .seed_scanned_song_with_sidecar("Blue Bird", "Ikimono-gakari", "AnimeSongs", &song_path)
        .await;

    harness
        .write_config(
            r#"
        [library.organize]
        base_dir = "D:/Library"
        conflict_policy = "first_match"

        [[library.organize.rules]]
        name = "anime"
        priority = 100
        match = { static_playlist = "AnimeSongs" }
        template = "AnimeSongs/{{ title|sanitize }} - {{ artist|sanitize }}"
        "#,
        )
        .await;

    let preview = harness
        .library_service()
        .preview_organize(None)
        .await
        .unwrap();
    assert_eq!(preview[0].rule_name, "anime");
    assert!(
        preview[0]
            .target_path
            .ends_with("AnimeSongs/Blue Bird - Ikimono-gakari.flac")
    );

    harness
        .library_service()
        .apply_organize(None)
        .await
        .unwrap();
    assert!(
        harness
            .file_exists("AnimeSongs/Blue Bird - Ikimono-gakari.flac")
            .await
    );
    assert!(
        harness
            .file_exists("AnimeSongs/Blue Bird - Ikimono-gakari.lrc")
            .await
    );
}

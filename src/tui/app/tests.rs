use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

fn browser_snapshot() -> crate::core::model::tui::PlaylistBrowserSnapshot {
    crate::core::model::tui::PlaylistBrowserSnapshot {
        default_view: crate::core::model::tui::TuiViewKind::Playlist,
        default_selected_playlist: Some("Favorites".to_string()),
        current_playing_playlist: Some(crate::core::model::tui::PlaylistListItem {
            name: "Favorites".to_string(),
            kind: "static".to_string(),
            count: 2,
            is_current_playing_source: true,
            is_ephemeral: false,
        }),
        visible_playlists: vec![
            crate::core::model::tui::PlaylistListItem {
                name: "Favorites".to_string(),
                kind: "static".to_string(),
                count: 2,
                is_current_playing_source: true,
                is_ephemeral: false,
            },
            crate::core::model::tui::PlaylistListItem {
                name: "Aimer".to_string(),
                kind: "smart".to_string(),
                count: 4,
                is_current_playing_source: false,
                is_ephemeral: false,
            },
        ],
    }
}

#[test]
fn app_uses_default_selected_playlist_only_for_initial_selection() {
    let mut app = crate::tui::app::App::new_for_test();
    app.apply_tui_snapshot(crate::core::model::tui::TuiSnapshot {
        player: crate::core::model::player::PlayerSnapshot::default(),
        active_task: None,
        playlist_browser: browser_snapshot(),
        current_track: crate::core::model::tui::CurrentTrackSnapshot::default(),
    });
    assert_eq!(app.selected_playlist_name(), Some("Favorites"));

    app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    assert_eq!(app.selected_playlist_name(), Some("Aimer"));

    app.apply_tui_snapshot(crate::core::model::tui::TuiSnapshot {
        player: crate::core::model::player::PlayerSnapshot::default(),
        active_task: None,
        playlist_browser: browser_snapshot(),
        current_track: crate::core::model::tui::CurrentTrackSnapshot::default(),
    });
    assert_eq!(app.selected_playlist_name(), Some("Aimer"));
}

#[test]
fn tab_switches_focus_between_playlist_list_and_preview() {
    let mut app = crate::tui::app::App::new_for_test();
    assert_eq!(app.focus, crate::tui::app::FocusArea::PlaylistList);

    let action = app.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));

    assert_eq!(action, None);
    assert_eq!(app.focus, crate::tui::app::FocusArea::PlaylistPreview);
}

#[test]
fn enter_on_playlist_list_requests_play_from_start() {
    let mut app = crate::tui::app::App::new_for_test();
    app.apply_tui_snapshot(crate::core::model::tui::TuiSnapshot {
        player: crate::core::model::player::PlayerSnapshot::default(),
        active_task: None,
        playlist_browser: browser_snapshot(),
        current_track: crate::core::model::tui::CurrentTrackSnapshot::default(),
    });

    let action = app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    assert_eq!(
        action,
        Some(crate::tui::event::Action::PlaySelectedPlaylistFromStart)
    );
}

#[test]
fn selecting_playlist_index_updates_highlight_without_playing() {
    let mut app = crate::tui::app::App::new_for_test();
    app.apply_tui_snapshot(crate::core::model::tui::TuiSnapshot {
        player: crate::core::model::player::PlayerSnapshot::default(),
        active_task: None,
        playlist_browser: browser_snapshot(),
        current_track: crate::core::model::tui::CurrentTrackSnapshot::default(),
    });

    let effect = app.select_playlist_index(1);

    assert_eq!(
        effect,
        Some(crate::tui::event::Intent::Action(
            crate::tui::event::ActionId::LoadPreview
        ))
    );
    assert_eq!(app.selected_playlist_name(), Some("Aimer"));
}

#[test]
fn app_sets_preview_row_current_track_by_song_id() {
    let mut app = crate::tui::app::App::new_for_test();
    app.apply_tui_snapshot(crate::core::model::tui::TuiSnapshot {
        player: crate::core::model::player::PlayerSnapshot {
            current_song: Some(crate::core::model::player::NowPlayingSong {
                song_id: 2,
                title: "Aimer".into(),
                duration_seconds: Some(180.0),
            }),
            queue_index: Some(1),
            ..crate::core::model::player::PlayerSnapshot::default()
        },
        active_task: None,
        playlist_browser: browser_snapshot(),
        current_track: crate::core::model::tui::CurrentTrackSnapshot {
            song_id: Some(2),
            title: Some("Aimer".into()),
            lyrics: Some("[00:00.00]hello".into()),
            lyrics_source_kind: Some("sidecar".into()),
            artwork: None,
        },
    });
    app.set_playlist_preview(&crate::api::playlist::PlaylistPreviewResponse {
        name: "Favorites".into(),
        songs: vec![
            crate::api::playlist::PlaylistPreviewSong {
                id: 1,
                title: "One".into(),
            },
            crate::api::playlist::PlaylistPreviewSong {
                id: 2,
                title: "Aimer".into(),
            },
        ],
    });

    let rows = crate::tui::ui::playlist::preview_row_models(&app);
    assert!(!rows[0].is_current_track);
    assert!(rows[1].is_current_track);
}

#[test]
fn esc_returns_focus_to_playlist_list() {
    let mut app = crate::tui::app::App::new_for_test();
    app.focus = crate::tui::app::FocusArea::PlaylistPreview;

    let intent = app.handle_action(crate::tui::event::ActionId::FocusPrev);

    assert_eq!(intent, None);
    assert_eq!(app.focus, crate::tui::app::FocusArea::PlaylistList);
}

#[test]
fn lyric_follow_state_transitions_from_manual_browse_back_to_follow_current() {
    use std::time::{Duration, Instant};

    let now = Instant::now();
    let mut state = crate::tui::lyrics::LyricFollowState::new(Duration::from_secs(3));
    state.on_manual_scroll(now);
    assert!(matches!(
        state.mode(),
        crate::tui::lyrics::LyricFollowMode::ManualBrowse
    ));

    state.tick(now + Duration::from_millis(100));
    assert!(matches!(
        state.mode(),
        crate::tui::lyrics::LyricFollowMode::ResumePending
    ));

    state.tick(now + Duration::from_secs(3));
    assert!(matches!(
        state.mode(),
        crate::tui::lyrics::LyricFollowMode::FollowCurrent
    ));
}

#[test]
fn app_uses_local_clock_for_current_lyric_index() {
    let mut app = crate::tui::app::App::new_for_test();
    app.current_track_song_id = Some(7);
    app.cache_track_content(crate::core::model::track_content::TrackContentSnapshot {
        song_id: 7,
        title: "Blue Bird".into(),
        duration_seconds: Some(212.0),
        artwork: None,
        lyrics: vec![
            crate::core::model::track_content::LyricLine {
                timestamp_seconds: 0.0,
                text: "a".into(),
            },
            crate::core::model::track_content::LyricLine {
                timestamp_seconds: 5.0,
                text: "b".into(),
            },
        ],
        refresh_token: "7-v1".into(),
    });

    let now = std::time::Instant::now();
    app.apply_runtime_snapshot_at(
        crate::core::model::playback_runtime::PlaybackRuntimeSnapshot {
            generation: 2,
            playback_state: "playing".into(),
            current_source_ref: Some("Favorites".into()),
            current_song_id: Some(7),
            current_index: Some(0),
            position_seconds: Some(4.4),
            duration_seconds: Some(212.0),
            playback_mode: crate::core::model::playback_mode::PlaybackMode::Ordered,
            volume_percent: 100,
            muted: false,
            last_error_code: None,
        },
        now,
    );

    assert_eq!(
        app.current_lyric_index_at(now + std::time::Duration::from_secs(1)),
        Some(1)
    );
}

#[test]
fn selecting_preview_index_updates_track_viewport_follow_position() {
    let mut app = crate::tui::app::App::new_for_test();
    app.load_fake_track_list_for_test(12);
    app.track_viewport.visible_height = 4;

    app.select_preview_index(6);

    assert_eq!(app.track_viewport.scroll_top, 3);
}

#[test]
fn applying_runtime_snapshot_follows_current_lyric_into_viewport() {
    let mut app = crate::tui::app::App::new_for_test();
    app.current_track_song_id = Some(7);
    app.lyric_viewport.visible_height = 2;
    app.cache_track_content(crate::core::model::track_content::TrackContentSnapshot {
        song_id: 7,
        title: "Blue Bird".into(),
        duration_seconds: Some(212.0),
        artwork: None,
        lyrics: vec![
            crate::core::model::track_content::LyricLine {
                timestamp_seconds: 0.0,
                text: "a".into(),
            },
            crate::core::model::track_content::LyricLine {
                timestamp_seconds: 1.0,
                text: "b".into(),
            },
            crate::core::model::track_content::LyricLine {
                timestamp_seconds: 2.0,
                text: "c".into(),
            },
            crate::core::model::track_content::LyricLine {
                timestamp_seconds: 3.0,
                text: "d".into(),
            },
        ],
        refresh_token: "7-v1".into(),
    });

    app.apply_runtime_snapshot_at(
        crate::core::model::playback_runtime::PlaybackRuntimeSnapshot {
            generation: 2,
            playback_state: "playing".into(),
            current_source_ref: Some("Favorites".into()),
            current_song_id: Some(7),
            current_index: Some(0),
            position_seconds: Some(3.2),
            duration_seconds: Some(212.0),
            playback_mode: crate::core::model::playback_mode::PlaybackMode::Ordered,
            volume_percent: 100,
            muted: false,
            last_error_code: None,
        },
        std::time::Instant::now(),
    );

    assert_eq!(app.lyric_viewport.scroll_top, 2);
}

#[test]
fn scrolling_lyrics_pauses_follow_and_moves_viewport() {
    let mut app = crate::tui::app::App::new_for_test();
    app.current_track_song_id = Some(7);
    app.lyric_viewport.visible_height = 2;
    app.cache_track_content(crate::core::model::track_content::TrackContentSnapshot {
        song_id: 7,
        title: "Blue Bird".into(),
        duration_seconds: Some(212.0),
        artwork: None,
        lyrics: vec![
            crate::core::model::track_content::LyricLine {
                timestamp_seconds: 0.0,
                text: "a".into(),
            },
            crate::core::model::track_content::LyricLine {
                timestamp_seconds: 1.0,
                text: "b".into(),
            },
            crate::core::model::track_content::LyricLine {
                timestamp_seconds: 2.0,
                text: "c".into(),
            },
            crate::core::model::track_content::LyricLine {
                timestamp_seconds: 3.0,
                text: "d".into(),
            },
        ],
        refresh_token: "7-v1".into(),
    });
    app.apply_runtime_snapshot_at(
        crate::core::model::playback_runtime::PlaybackRuntimeSnapshot {
            generation: 2,
            playback_state: "playing".into(),
            current_source_ref: Some("Favorites".into()),
            current_song_id: Some(7),
            current_index: Some(0),
            position_seconds: Some(3.2),
            duration_seconds: Some(212.0),
            playback_mode: crate::core::model::playback_mode::PlaybackMode::Ordered,
            volume_percent: 100,
            muted: false,
            last_error_code: None,
        },
        std::time::Instant::now(),
    );

    let now = std::time::Instant::now();
    app.scroll_lyrics(-1, now);

    assert_eq!(app.lyric_viewport.scroll_top, 1);
    assert!(matches!(
        app.lyric_follow_state.mode(),
        crate::tui::lyrics::LyricFollowMode::ManualBrowse
    ));
}

#[test]
fn ticking_lyrics_restores_follow_and_recenters_after_delay() {
    use std::time::Duration;

    let mut app = crate::tui::app::App::new_for_test();
    app.current_track_song_id = Some(7);
    app.lyric_viewport.visible_height = 2;
    app.cache_track_content(crate::core::model::track_content::TrackContentSnapshot {
        song_id: 7,
        title: "Blue Bird".into(),
        duration_seconds: Some(212.0),
        artwork: None,
        lyrics: vec![
            crate::core::model::track_content::LyricLine {
                timestamp_seconds: 0.0,
                text: "a".into(),
            },
            crate::core::model::track_content::LyricLine {
                timestamp_seconds: 1.0,
                text: "b".into(),
            },
            crate::core::model::track_content::LyricLine {
                timestamp_seconds: 2.0,
                text: "c".into(),
            },
            crate::core::model::track_content::LyricLine {
                timestamp_seconds: 3.0,
                text: "d".into(),
            },
        ],
        refresh_token: "7-v1".into(),
    });
    let base = std::time::Instant::now();
    app.apply_runtime_snapshot_at(
        crate::core::model::playback_runtime::PlaybackRuntimeSnapshot {
            generation: 2,
            playback_state: "playing".into(),
            current_source_ref: Some("Favorites".into()),
            current_song_id: Some(7),
            current_index: Some(0),
            position_seconds: Some(3.2),
            duration_seconds: Some(212.0),
            playback_mode: crate::core::model::playback_mode::PlaybackMode::Ordered,
            volume_percent: 100,
            muted: false,
            last_error_code: None,
        },
        base,
    );
    app.scroll_lyrics(-1, base);

    app.tick_lyrics(base + Duration::from_secs(3));

    assert!(matches!(
        app.lyric_follow_state.mode(),
        crate::tui::lyrics::LyricFollowMode::FollowCurrent
    ));
    assert_eq!(app.lyric_viewport.scroll_top, 2);
}

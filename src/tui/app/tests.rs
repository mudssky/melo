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

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[tokio::test]
async fn tui_space_key_maps_to_toggle_command() {
    let mut app = melo::tui::app::App::new_for_test();
    let action = app.handle_key(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE));
    assert_eq!(action, Some(melo::tui::event::Action::TogglePlayback));
}

#[tokio::test]
async fn tui_updates_player_snapshot_from_ws_messages() {
    let mut app = melo::tui::app::App::new_for_test();
    app.apply_snapshot(melo::core::model::player::PlayerSnapshot {
        backend_name: "noop".into(),
        playback_state: "playing".into(),
        queue_preview: vec!["Blue Bird".into()],
        current_song: Some(melo::core::model::player::NowPlayingSong {
            song_id: 1,
            title: "Blue Bird".into(),
            duration_seconds: Some(212.0),
        }),
        queue_len: 1,
        queue_index: Some(0),
        has_next: false,
        has_prev: false,
        last_error: None,
        version: 1,
        position_seconds: None,
        position_fraction: None,
        volume_percent: 100,
        muted: false,
        repeat_mode: "off".into(),
        shuffle_enabled: false,
    });

    assert_eq!(app.player.playback_state, "playing");
    assert_eq!(app.player.current_song.unwrap().title, "Blue Bird");
}

#[tokio::test]
async fn tui_applies_navigation_flags_and_last_error_from_snapshot() {
    let mut app = melo::tui::app::App::new_for_test();
    app.apply_snapshot(melo::core::model::player::PlayerSnapshot {
        backend_name: "noop".into(),
        playback_state: "error".into(),
        queue_preview: vec!["One".into(), "Two".into()],
        current_song: None,
        queue_len: 2,
        queue_index: Some(1),
        has_next: false,
        has_prev: true,
        last_error: Some(melo::core::model::player::PlayerErrorInfo {
            code: "queue_no_next".into(),
            message: "queue has no next item".into(),
        }),
        version: 4,
        position_seconds: None,
        position_fraction: None,
        volume_percent: 100,
        muted: false,
        repeat_mode: "off".into(),
        shuffle_enabled: false,
    });

    assert_eq!(app.player.playback_state, "error");
    assert!(app.player.has_prev);
    assert_eq!(
        app.player.last_error.as_ref().unwrap().code,
        "queue_no_next"
    );
    assert_eq!(app.player.version, 4);
    assert_eq!(
        app.footer_status(),
        "ERR queue_no_next: queue has no next item"
    );
}

#[test]
fn tui_song_rows_measure_cjk_width_correctly() {
    let rendered = melo::tui::ui::content::render_song_title("夜に駆ける", 5);

    assert_eq!(rendered, "夜に…");
    assert_eq!(unicode_width::UnicodeWidthStr::width(rendered.as_str()), 5);
}

#[test]
fn playback_label_renders_progress_window() {
    let label =
        melo::tui::ui::playbar::playback_label(&melo::core::model::player::PlayerSnapshot {
            backend_name: "noop".into(),
            playback_state: "playing".into(),
            queue_preview: vec!["Blue Bird".into()],
            current_song: Some(melo::core::model::player::NowPlayingSong {
                song_id: 1,
                title: "Blue Bird".into(),
                duration_seconds: Some(212.0),
            }),
            queue_len: 1,
            queue_index: Some(0),
            has_next: false,
            has_prev: false,
            last_error: None,
            version: 3,
            position_seconds: Some(72.0),
            position_fraction: Some(72.0 / 212.0),
            volume_percent: 100,
            muted: false,
            repeat_mode: "off".into(),
            shuffle_enabled: false,
        });

    assert!(label.contains("01:12"));
    assert!(label.contains("03:32"));
    assert!(label.contains("Blue Bird"));
}

#[test]
fn footer_status_includes_volume_and_repeat_mode() {
    let mut app = melo::tui::app::App::new_for_test();
    app.apply_snapshot(melo::core::model::player::PlayerSnapshot {
        backend_name: "noop".into(),
        playback_state: "playing".into(),
        queue_preview: vec!["Blue Bird".into(), "Always Online".into()],
        current_song: None,
        queue_len: 2,
        queue_index: Some(0),
        has_next: true,
        has_prev: false,
        last_error: None,
        version: 6,
        position_seconds: Some(10.0),
        position_fraction: Some(0.1),
        volume_percent: 55,
        muted: false,
        repeat_mode: "all".into(),
        shuffle_enabled: true,
    });

    let footer = app.footer_status();
    assert!(footer.contains("vol=55"));
    assert!(footer.contains("repeat=all"));
    assert!(footer.contains("shuffle=true"));
}

#[test]
fn footer_status_still_includes_repeat_and_shuffle_after_playlist_snapshot_applies() {
    let mut app = melo::tui::app::App::new_for_test();
    app.apply_tui_snapshot(melo::core::model::tui::TuiSnapshot {
        player: melo::core::model::player::PlayerSnapshot {
            repeat_mode: "all".into(),
            shuffle_enabled: true,
            ..melo::core::model::player::PlayerSnapshot::default()
        },
        active_task: None,
        playlist_browser: melo::core::model::tui::PlaylistBrowserSnapshot::default(),
        current_track: melo::core::model::tui::CurrentTrackSnapshot::default(),
    });

    let footer = app.footer_status();
    assert!(footer.contains("repeat=all"));
    assert!(footer.contains("shuffle=true"));
}

#[test]
fn question_mark_toggles_help_popup() {
    let mut app = melo::tui::app::App::new_for_test();
    assert!(!app.show_help);

    let action = app.handle_key(KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE));

    assert_eq!(action, Some(melo::tui::event::Action::OpenHelp));
    assert!(app.show_help);
}

#[test]
fn footer_hints_can_be_hidden() {
    let mut app = melo::tui::app::App::new_for_test();
    app.footer_hints_enabled = false;

    let footer = app.footer_status();
    assert!(!footer.contains("? Help"));
}

#[test]
fn queue_panel_renders_loaded_titles() {
    let mut app = melo::tui::app::App::new_for_test();
    app.queue_titles = vec!["Blue Bird".to_string(), "Always Online".to_string()];

    let content = app.render_queue_lines();
    assert!(content.iter().any(|line| line.contains("Blue Bird")));
    assert!(content.iter().any(|line| line.contains("Always Online")));
}

#[test]
fn active_task_bar_renders_current_item_name() {
    let mut app = melo::tui::app::App::new_for_test();
    app.apply_tui_snapshot(melo::core::model::tui::TuiSnapshot {
        player: melo::core::model::player::PlayerSnapshot::default(),
        active_task: Some(melo::core::model::runtime_task::RuntimeTaskSnapshot {
            task_id: "scan-1".into(),
            kind: melo::core::model::runtime_task::RuntimeTaskKind::LibraryScan,
            phase: melo::core::model::runtime_task::RuntimeTaskPhase::Indexing,
            source_label: "D:/Music/Aimer".into(),
            discovered_count: 240,
            indexed_count: 12,
            queued_count: 12,
            current_item_name: Some("Ref:rain.flac".into()),
            last_error: None,
        }),
        playlist_browser: melo::core::model::tui::PlaylistBrowserSnapshot::default(),
        current_track: melo::core::model::tui::CurrentTrackSnapshot::default(),
    });

    let text = app.task_bar_text(
        &melo::core::runtime_templates::RuntimeTemplateRenderer::default(),
        &melo::core::config::settings::Settings::default(),
        120,
    );

    assert!(text.is_some());
    assert!(text.unwrap().contains("Ref:rain.flac"));
}

#[test]
fn active_task_bar_truncates_long_text_to_available_width() {
    let mut app = melo::tui::app::App::new_for_test();
    app.apply_tui_snapshot(melo::core::model::tui::TuiSnapshot {
        player: melo::core::model::player::PlayerSnapshot::default(),
        active_task: Some(melo::core::model::runtime_task::RuntimeTaskSnapshot {
            task_id: "scan-1".into(),
            kind: melo::core::model::runtime_task::RuntimeTaskKind::LibraryScan,
            phase: melo::core::model::runtime_task::RuntimeTaskPhase::Indexing,
            source_label: "D:/Very/Long/Source/Path/That/Should/Be/Trimmed".into(),
            discovered_count: 240,
            indexed_count: 12,
            queued_count: 12,
            current_item_name: Some(
                "A very very long filename that should not overflow.flac".into(),
            ),
            last_error: None,
        }),
        playlist_browser: melo::core::model::tui::PlaylistBrowserSnapshot::default(),
        current_track: melo::core::model::tui::CurrentTrackSnapshot::default(),
    });

    let text = app
        .task_bar_text(
            &melo::core::runtime_templates::RuntimeTemplateRenderer::default(),
            &melo::core::config::settings::Settings::default(),
            40,
        )
        .unwrap();

    assert!(unicode_width::UnicodeWidthStr::width(text.as_str()) <= 40);
}

fn browser_snapshot() -> melo::core::model::tui::PlaylistBrowserSnapshot {
    melo::core::model::tui::PlaylistBrowserSnapshot {
        default_view: melo::core::model::tui::TuiViewKind::Playlist,
        default_selected_playlist: Some("Favorites".to_string()),
        current_playing_playlist: Some(melo::core::model::tui::PlaylistListItem {
            name: "Favorites".to_string(),
            kind: "static".to_string(),
            count: 2,
            is_current_playing_source: true,
            is_ephemeral: false,
        }),
        visible_playlists: vec![
            melo::core::model::tui::PlaylistListItem {
                name: "Favorites".to_string(),
                kind: "static".to_string(),
                count: 2,
                is_current_playing_source: true,
                is_ephemeral: false,
            },
            melo::core::model::tui::PlaylistListItem {
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
fn render_playlist_rows_marks_selected_and_current_source_separately() {
    let mut app = melo::tui::app::App::new_for_test();
    app.apply_tui_snapshot(melo::core::model::tui::TuiSnapshot {
        player: melo::core::model::player::PlayerSnapshot::default(),
        active_task: None,
        playlist_browser: browser_snapshot(),
        current_track: melo::core::model::tui::CurrentTrackSnapshot::default(),
    });
    app.select_playlist_index(1);

    let rows = melo::tui::ui::playlist::playlist_row_models(&app);
    assert!(rows[0].is_current_source);
    assert!(!rows[0].is_selected);
    assert!(rows[1].is_selected);
}

#[test]
fn detail_lines_show_lyrics_or_cover_fallback() {
    let mut app = melo::tui::app::App::new_for_test();
    app.current_track_lyrics = Some("[00:00.00]hello".to_string());
    app.current_track_cover_summary = Some("Cover unsupported in this terminal".to_string());

    let lines = melo::tui::ui::details::render_detail_lines(&app);
    assert!(lines.iter().any(|line| line.contains("hello")));
    assert!(lines.iter().any(|line| line.contains("unsupported")));
}

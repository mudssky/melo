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
        playback_state: "playing".into(),
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
    });

    assert_eq!(app.player.playback_state, "playing");
    assert_eq!(app.player.current_song.unwrap().title, "Blue Bird");
}

#[tokio::test]
async fn tui_applies_navigation_flags_and_last_error_from_snapshot() {
    let mut app = melo::tui::app::App::new_for_test();
    app.apply_snapshot(melo::core::model::player::PlayerSnapshot {
        playback_state: "error".into(),
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

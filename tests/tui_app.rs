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
    });

    assert_eq!(app.player.playback_state, "playing");
    assert_eq!(app.player.current_song.unwrap().title, "Blue Bird");
}

#[test]
fn tui_song_rows_measure_cjk_width_correctly() {
    let rendered = melo::tui::ui::content::render_song_title("夜に駆ける", 5);

    assert_eq!(rendered, "夜に…");
    assert_eq!(unicode_width::UnicodeWidthStr::width(rendered.as_str()), 5);
}

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
fn app_source_label_is_rendered_in_status_line() {
    let mut app = crate::tui::app::App::new_for_test();
    app.set_source_label("cwd:/music");

    assert!(app.footer_status().contains("cwd:/music"));
}

#[test]
fn quit_key_still_maps_to_quit_action() {
    let mut app = crate::tui::app::App::new_for_test();
    let action = app.handle_key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE));

    assert_eq!(action, Some(crate::tui::event::Action::Quit));
}

#[test]
fn startup_notice_is_included_in_status_line() {
    let mut app = crate::tui::app::App::new_for_test();
    app.startup_notice = Some("open_scan_failed".to_string());

    assert!(app.footer_status().contains("open_scan_failed"));
}

#[test]
fn top_task_bar_layout_only_reserves_space_when_needed() {
    let full = crate::tui::ui::layout::split(ratatui::layout::Rect::new(0, 0, 100, 30), true);
    let compact = crate::tui::ui::layout::split(ratatui::layout::Rect::new(0, 0, 100, 30), false);

    assert!(full.task_bar.is_some());
    assert!(compact.task_bar.is_none());
    assert!(full.content.y > compact.content.y);
}

#[test]
fn repeat_mode_cycles_off_all_one_off() {
    assert_eq!(crate::tui::run::next_repeat_mode("off"), "all");
    assert_eq!(crate::tui::run::next_repeat_mode("all"), "one");
    assert_eq!(crate::tui::run::next_repeat_mode("one"), "off");
}

#[test]
fn should_stop_on_tui_exit_for_active_sessions_only() {
    assert!(crate::tui::run::should_stop_on_tui_exit("playing"));
    assert!(crate::tui::run::should_stop_on_tui_exit("paused"));
    assert!(crate::tui::run::should_stop_on_tui_exit("error"));
    assert!(!crate::tui::run::should_stop_on_tui_exit("stopped"));
    assert!(!crate::tui::run::should_stop_on_tui_exit("idle"));
}

#[test]
fn interactive_terminal_guard_accepts_real_terminal_shape() {
    let result = crate::tui::run::ensure_interactive_terminal(true, true);
    assert!(result.is_ok());
}

#[test]
fn interactive_terminal_guard_rejects_non_interactive_stdio_with_hint() {
    let err = crate::tui::run::ensure_interactive_terminal(false, true).unwrap_err();
    assert!(err.to_string().contains("interactive terminal"));
    assert!(err.to_string().contains("melo status"));
}

#[test]
fn hit_test_mouse_target_maps_sidebar_row_to_playlist_item() {
    let mut app = crate::tui::app::App::new_for_test();
    app.apply_tui_snapshot(crate::core::model::tui::TuiSnapshot {
        player: crate::core::model::player::PlayerSnapshot::default(),
        active_task: None,
        playlist_browser: browser_snapshot(),
        current_track: crate::core::model::tui::CurrentTrackSnapshot::default(),
    });
    let layout = crate::tui::ui::layout::split(ratatui::layout::Rect::new(0, 0, 100, 30), false);

    let target =
        super::hit_test_mouse_target(layout, &app, layout.sidebar.x + 1, layout.sidebar.y + 2);

    assert_eq!(target, crate::tui::mouse::MouseTarget::PlaylistRow(0));
}

#[test]
fn status_lines_include_launch_cwd_context() {
    let mut app = crate::tui::app::App::new_for_test();
    app.set_launch_cwd("D:/Music/Aimer");

    let lines = crate::tui::ui::playlist::render_status_lines(&app);

    assert!(lines.iter().any(|line| line.contains("D:/Music/Aimer")));
}

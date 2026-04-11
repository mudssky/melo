use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

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

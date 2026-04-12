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

#[test]
fn viewport_scrolls_when_selected_item_moves_below_visible_window() {
    let mut viewport = crate::tui::viewports::ViewportState::new(4);
    viewport.follow_selection(6, 12);
    assert_eq!(viewport.scroll_top, 3);
}

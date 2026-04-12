use std::time::{Duration, Instant};

use crate::tui::mouse::{ClickKind, ClickTracker, MouseTarget};

#[test]
fn click_tracker_promotes_second_click_on_same_target_to_double_click() {
    let mut tracker = ClickTracker::default();
    let now = Instant::now();

    assert_eq!(
        tracker.classify(MouseTarget::PlaylistRow(3), now),
        ClickKind::Single
    );
    assert_eq!(
        tracker.classify(
            MouseTarget::PlaylistRow(3),
            now + Duration::from_millis(200)
        ),
        ClickKind::Double
    );
}

#[test]
fn click_tracker_resets_when_target_changes() {
    let mut tracker = ClickTracker::default();
    let now = Instant::now();

    assert_eq!(
        tracker.classify(MouseTarget::PlaylistRow(1), now),
        ClickKind::Single
    );
    assert_eq!(
        tracker.classify(MouseTarget::PreviewRow(1), now + Duration::from_millis(200)),
        ClickKind::Single
    );
}

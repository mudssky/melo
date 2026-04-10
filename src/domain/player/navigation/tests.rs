use crate::core::model::player::RepeatMode;
use crate::domain::player::navigation::PlaybackNavigation;

#[test]
fn repeat_all_wraps_manual_next_from_tail_to_head() {
    let navigation = PlaybackNavigation::linear(3, Some(2));
    assert_eq!(navigation.next_index(RepeatMode::All, false), Some(0));
}

#[test]
fn repeat_one_replays_current_track_on_track_end() {
    let navigation = PlaybackNavigation::linear(3, Some(1));
    assert_eq!(navigation.track_end_index(RepeatMode::One, false), Some(1));
}

#[test]
fn shuffle_uses_projected_order_without_mutating_visible_queue() {
    let navigation = PlaybackNavigation::shuffled(4, Some(1), 7);
    let projected = navigation.order().to_vec();

    assert_eq!(projected.len(), 4);
    assert!(projected.contains(&0));
    assert!(projected.contains(&1));
    assert!(projected.contains(&2));
    assert!(projected.contains(&3));
    assert_eq!(navigation.current_visible_index(), Some(1));
}

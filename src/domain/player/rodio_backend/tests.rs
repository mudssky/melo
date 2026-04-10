use super::should_emit_track_end;

#[test]
fn emits_track_end_only_for_current_generation_when_queue_is_empty() {
    assert!(should_emit_track_end(3, 3, true));
}

#[test]
fn ignores_track_end_when_generation_is_stale() {
    assert!(!should_emit_track_end(4, 3, true));
}

#[test]
fn ignores_track_end_when_player_still_has_audio() {
    assert!(!should_emit_track_end(3, 3, false));
}

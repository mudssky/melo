use super::should_emit_track_end;

#[test]
fn rodio_session_emits_natural_eof_only_for_current_generation() {
    assert!(should_emit_track_end(3, 3, true));
    assert!(!should_emit_track_end(4, 3, true));
    assert!(!should_emit_track_end(3, 3, false));
}

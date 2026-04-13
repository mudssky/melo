use crate::domain::player::backend::PlaybackBackend;
use crate::domain::player::runtime::PlaybackStopReason;

#[test]
fn libmpv_end_file_reason_maps_to_runtime_reason() {
    assert_eq!(
        super::map_end_file_reason("eof"),
        PlaybackStopReason::NaturalEof
    );
    assert_eq!(
        super::map_end_file_reason("stop"),
        PlaybackStopReason::UserStop
    );
    assert_eq!(
        super::map_end_file_reason("quit"),
        PlaybackStopReason::UserClosedBackend
    );
}

#[test]
fn libmpv_backend_reports_stable_name() {
    let backend = super::LibmpvBackend::new_for_test();
    assert_eq!(backend.backend_name(), "mpv_lib");
}

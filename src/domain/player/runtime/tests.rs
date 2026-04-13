use super::{PlaybackRuntimeEvent, PlaybackStopReason};

#[test]
fn playback_stopped_event_keeps_generation_and_reason() {
    let event = PlaybackRuntimeEvent::PlaybackStopped {
        generation: 7,
        reason: PlaybackStopReason::UserStop,
    };

    match event {
        PlaybackRuntimeEvent::PlaybackStopped { generation, reason } => {
            assert_eq!(generation, 7);
            assert_eq!(reason, PlaybackStopReason::UserStop);
        }
    }
}

#[test]
fn stop_reasons_remain_distinct() {
    assert_ne!(
        PlaybackStopReason::NaturalEof,
        PlaybackStopReason::BackendAborted
    );
    assert_ne!(
        PlaybackStopReason::UserStop,
        PlaybackStopReason::UserClosedBackend
    );
}

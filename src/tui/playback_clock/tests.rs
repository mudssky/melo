use std::time::{Duration, Instant};

use crate::core::model::playback_mode::PlaybackMode;
use crate::core::model::playback_runtime::PlaybackRuntimeSnapshot;

#[test]
fn playback_clock_advances_locally_while_playing() {
    let now = Instant::now();
    let mut clock = crate::tui::playback_clock::PlaybackClock::default();
    clock.apply_runtime(
        &PlaybackRuntimeSnapshot {
            generation: 3,
            playback_state: "playing".into(),
            current_source_ref: Some("Favorites".into()),
            current_song_id: Some(7),
            current_index: Some(0),
            position_seconds: Some(10.0),
            duration_seconds: Some(200.0),
            playback_mode: PlaybackMode::Ordered,
            volume_percent: 100,
            muted: false,
            last_error_code: None,
        },
        now,
    );

    let display = clock.display_position(now + Duration::from_millis(900));
    assert!(display.is_some_and(|value| value >= 10.8));
}

#[test]
fn playback_clock_freezes_when_runtime_is_paused() {
    let now = Instant::now();
    let mut clock = crate::tui::playback_clock::PlaybackClock::default();
    let runtime = PlaybackRuntimeSnapshot {
        generation: 5,
        playback_state: "paused".into(),
        current_source_ref: Some("Favorites".into()),
        current_song_id: Some(7),
        current_index: Some(0),
        position_seconds: Some(42.0),
        duration_seconds: Some(200.0),
        playback_mode: PlaybackMode::Ordered,
        volume_percent: 100,
        muted: false,
        last_error_code: None,
    };
    clock.apply_runtime(&runtime, now);

    assert_eq!(
        clock.display_position(now + Duration::from_secs(2)),
        Some(42.0)
    );
}

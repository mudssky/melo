use crate::core::model::playback_mode::PlaybackMode;
use crate::core::model::player::RepeatMode;

#[test]
fn playback_mode_projects_to_legacy_repeat_and_shuffle_flags() {
    let ordered = PlaybackMode::Ordered.project();
    assert_eq!(ordered.repeat_mode, RepeatMode::Off);
    assert!(!ordered.shuffle_enabled);
    assert!(!ordered.stop_after_current);

    let single = PlaybackMode::Single.project();
    assert_eq!(single.repeat_mode, RepeatMode::Off);
    assert!(!single.shuffle_enabled);
    assert!(single.stop_after_current);
}

#[test]
fn playback_mode_parses_config_strings() {
    assert_eq!(
        PlaybackMode::from_config("ordered").unwrap(),
        PlaybackMode::Ordered
    );
    assert_eq!(
        PlaybackMode::from_config("repeat_one").unwrap(),
        PlaybackMode::RepeatOne
    );
    assert_eq!(
        PlaybackMode::from_config("shuffle").unwrap(),
        PlaybackMode::Shuffle
    );
    assert_eq!(
        PlaybackMode::from_config("single").unwrap(),
        PlaybackMode::Single
    );
}

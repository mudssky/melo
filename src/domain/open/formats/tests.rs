use std::path::Path;

use crate::domain::open::formats::is_supported_audio_path;

#[test]
fn supports_case_insensitive_audio_extensions() {
    assert!(is_supported_audio_path(Path::new("Always Online.FLAC")));
    assert!(is_supported_audio_path(Path::new("always-online.Mp3")));
    assert!(is_supported_audio_path(Path::new("always-online.m4a")));
    assert!(is_supported_audio_path(Path::new("always-online.AAC")));
}

#[test]
fn rejects_non_audio_extensions() {
    assert!(!is_supported_audio_path(Path::new("cover.jpg")));
}

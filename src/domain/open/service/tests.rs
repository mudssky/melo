use std::path::PathBuf;

use crate::domain::open::service::{classify_target, discover_audio_paths};

#[test]
fn classify_target_rejects_images() {
    let err = classify_target(&PathBuf::from("cover.jpg")).unwrap_err();
    assert!(err.to_string().contains("unsupported_open_format"));
}

#[test]
fn discover_audio_paths_respects_max_depth() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(temp.path().join("a/b/c")).unwrap();
    std::fs::write(temp.path().join("a").join("one.mp3"), b"audio").unwrap();
    std::fs::write(temp.path().join("a/b").join("two.flac"), b"audio").unwrap();
    std::fs::write(temp.path().join("a/b/c").join("three.ogg"), b"audio").unwrap();

    let found = discover_audio_paths(&temp.path().join("a"), 1).unwrap();
    assert_eq!(found.len(), 2);
}

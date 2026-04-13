use crate::core::model::track_content::{ArtworkSummary, LyricLine, TrackContentSnapshot};

#[test]
fn track_content_reports_current_line_for_runtime_position() {
    let content = TrackContentSnapshot {
        song_id: 7,
        title: "Blue Bird".into(),
        duration_seconds: Some(212.0),
        artwork: Some(ArtworkSummary {
            source_kind: "sidecar".into(),
            source_path: Some("D:/Music/cover.jpg".into()),
            terminal_summary: "Cover: sidecar".into(),
        }),
        lyrics: vec![
            LyricLine {
                timestamp_seconds: 1.0,
                text: "a".into(),
            },
            LyricLine {
                timestamp_seconds: 5.0,
                text: "b".into(),
            },
            LyricLine {
                timestamp_seconds: 9.0,
                text: "c".into(),
            },
        ],
        refresh_token: "song-7-v1".into(),
    };

    assert_eq!(content.current_lyric_index(0.5), None);
    assert_eq!(content.current_lyric_index(1.1), Some(0));
    assert_eq!(content.current_lyric_index(5.2), Some(1));
    assert_eq!(content.current_lyric_index(12.0), Some(2));
}

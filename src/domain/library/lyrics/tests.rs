use crate::domain::library::lyrics::parse_lyrics_timeline;

#[test]
fn parse_lyrics_timeline_extracts_lrc_tags_in_order() {
    let lines = parse_lyrics_timeline("[00:01.00]hello\n[00:05.50]world");
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0].timestamp_seconds, 1.0);
    assert_eq!(lines[0].text, "hello");
    assert_eq!(lines[1].timestamp_seconds, 5.5);
    assert_eq!(lines[1].text, "world");
}

#[test]
fn parse_lyrics_timeline_falls_back_to_plain_lines() {
    let lines = parse_lyrics_timeline("plain one\nplain two");
    assert_eq!(lines[0].timestamp_seconds, 0.0);
    assert_eq!(lines[1].timestamp_seconds, 1.0);
}

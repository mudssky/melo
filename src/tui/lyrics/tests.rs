#[test]
fn lyric_follow_state_pauses_and_resumes_after_timeout() {
    use std::time::{Duration, Instant};

    let now = Instant::now();
    let mut state = crate::tui::lyrics::LyricFollowState::new(Duration::from_secs(3));
    state.pause_for_manual_scroll(now);
    assert!(state.is_manual_browse());
    assert!(!state.should_resume(now + Duration::from_secs(2)));
    assert!(state.should_resume(now + Duration::from_secs(3)));
}

use crate::domain::player::backend::{PlaybackBackend, PlaybackStartRequest};

#[test]
fn noop_backend_creates_noop_session() {
    let backend = crate::domain::player::backend::NoopBackend;
    let session = backend
        .start_session(PlaybackStartRequest {
            path: "tests/fixtures/full_test.mp3".into(),
            generation: 7,
            volume_factor: 1.0,
        })
        .unwrap();

    assert_eq!(session.current_position(), None);
    session.pause().unwrap();
    session.resume().unwrap();
    session.stop().unwrap();
}

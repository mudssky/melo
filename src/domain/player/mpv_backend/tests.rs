use crate::domain::player::mpv_backend::{build_mpv_command, map_pipe_event_to_runtime};
use crate::domain::player::runtime::{PlaybackRuntimeEvent, PlaybackStopReason};

#[test]
fn mpv_backend_creates_session_and_maps_headless_flags() {
    let command = build_mpv_command(
        "C:/Tools/mpv.exe",
        "\\\\.\\pipe\\melo-mpv-test",
        &Vec::<String>::new(),
    );
    let args = command
        .get_args()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect::<Vec<_>>();

    assert!(args.iter().any(|arg| arg == "--idle=yes"));
    assert!(args.iter().any(|arg| arg == "--force-window=no"));
    assert!(args.iter().any(|arg| arg == "--no-video"));
}

#[test]
fn mpv_session_maps_end_file_to_runtime_stop_reason() {
    assert_eq!(
        map_pipe_event_to_runtime(r#"{"event":"end-file","reason":"eof"}"#, 3).unwrap(),
        Some(PlaybackRuntimeEvent::PlaybackStopped {
            generation: 3,
            reason: PlaybackStopReason::NaturalEof,
        })
    );
}

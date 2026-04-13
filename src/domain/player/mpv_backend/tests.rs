use crate::domain::player::mpv_backend::{build_mpv_command, parse_mpv_event};
use crate::domain::player::runtime::{PlaybackRuntimeEvent, PlaybackStopReason};

#[test]
fn build_mpv_command_forces_headless_audio_client_mode() {
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
    assert!(args.iter().any(|arg| arg == "--no-video"));
    assert!(args.iter().any(|arg| arg == "--force-window=no"));
}

#[test]
fn parse_end_file_event_distinguishes_eof_and_user_close() {
    assert_eq!(
        parse_mpv_event(r#"{"event":"end-file","reason":"eof"}"#, 7).unwrap(),
        Some(PlaybackRuntimeEvent::PlaybackStopped {
            generation: 7,
            reason: PlaybackStopReason::NaturalEof,
        })
    );
    assert_eq!(
        parse_mpv_event(r#"{"event":"end-file","reason":"quit"}"#, 7).unwrap(),
        Some(PlaybackRuntimeEvent::PlaybackStopped {
            generation: 7,
            reason: PlaybackStopReason::UserClosedBackend,
        })
    );
}

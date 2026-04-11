use crate::domain::player::mpv_backend::{build_mpv_command, parse_mpv_event};
use crate::domain::player::runtime::PlaybackRuntimeEvent;

#[test]
fn build_mpv_command_includes_windows_ipc_server_argument() {
    let command = build_mpv_command(
        "C:/Tools/mpv.exe",
        "\\\\.\\pipe\\melo-mpv-test",
        &["--no-video".to_string()],
    );
    let args = command
        .get_args()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect::<Vec<_>>();

    assert!(args.iter().any(|arg| arg == "--idle=yes"));
    assert!(args.iter().any(|arg| arg == "--no-video"));
    assert!(
        args.iter()
            .any(|arg| arg.contains("--input-ipc-server=\\\\.\\pipe\\melo-mpv-test"))
    );
}

#[test]
fn parse_end_file_event_turns_into_track_end() {
    let event = parse_mpv_event(r#"{"event":"end-file","reason":"eof"}"#, 7).unwrap();

    assert_eq!(
        event,
        Some(PlaybackRuntimeEvent::TrackEnded { generation: 7 })
    );
}

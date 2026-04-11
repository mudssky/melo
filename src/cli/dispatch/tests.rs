use std::ffi::OsString;

use crate::cli::dispatch::{Dispatch, dispatch_args};

#[test]
fn dispatch_without_args_uses_default_launch() {
    assert_eq!(
        dispatch_args(&[OsString::from("melo")]),
        Dispatch::DefaultLaunch
    );
}

#[test]
fn dispatch_with_audio_path_prefers_direct_open() {
    let dispatch = dispatch_args(&[OsString::from("melo"), OsString::from("song.flac")]);
    assert_eq!(dispatch, Dispatch::DirectOpen("song.flac".into()));
}

#[test]
fn dispatch_with_known_subcommand_stays_in_clap_mode() {
    let dispatch = dispatch_args(&[OsString::from("melo"), OsString::from("play")]);
    assert_eq!(dispatch, Dispatch::Clap);
}

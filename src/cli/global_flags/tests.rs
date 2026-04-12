use std::ffi::OsString;

use crate::cli::dispatch::{Dispatch, dispatch_args};
use crate::cli::global_flags::prepare_args;
use crate::core::config::settings::LoggingLevel;

#[test]
fn prepare_args_keeps_default_launch_when_only_verbose_is_present() {
    let prepared = prepare_args(&[OsString::from("melo"), OsString::from("--verbose")]).unwrap();

    assert!(prepared.logging.verbose);
    assert_eq!(prepared.dispatch_args, vec![OsString::from("melo")]);
    assert_eq!(
        dispatch_args(&prepared.dispatch_args),
        Dispatch::DefaultLaunch
    );
}

#[test]
fn prepare_args_preserves_direct_open_target_after_logging_flags() {
    let prepared = prepare_args(&[
        OsString::from("melo"),
        OsString::from("--verbose"),
        OsString::from("D:/Music"),
    ])
    .unwrap();

    assert_eq!(
        dispatch_args(&prepared.dispatch_args),
        Dispatch::DirectOpen("D:/Music".to_string())
    );
}

#[test]
fn prepare_args_extracts_explicit_levels_and_prefix_toggle() {
    let prepared = prepare_args(&[
        OsString::from("melo"),
        OsString::from("--log-level"),
        OsString::from("debug"),
        OsString::from("--daemon-log-level"),
        OsString::from("trace"),
        OsString::from("--no-log-prefix"),
        OsString::from("status"),
    ])
    .unwrap();

    assert_eq!(prepared.logging.log_level, Some(LoggingLevel::Debug));
    assert_eq!(prepared.logging.daemon_log_level, Some(LoggingLevel::Trace));
    assert!(prepared.logging.no_log_prefix);
    assert_eq!(
        prepared.clap_args,
        vec![OsString::from("melo"), OsString::from("status")]
    );
}

#[test]
fn prepare_args_preserves_subcommand_verbose_flags_after_dispatch_target() {
    let prepared = prepare_args(&[
        OsString::from("melo"),
        OsString::from("daemon"),
        OsString::from("status"),
        OsString::from("--verbose"),
    ])
    .unwrap();

    assert!(!prepared.logging.verbose);
    assert_eq!(
        prepared.clap_args,
        vec![
            OsString::from("melo"),
            OsString::from("daemon"),
            OsString::from("status"),
            OsString::from("--verbose"),
        ]
    );
    assert_eq!(prepared.dispatch_args, prepared.clap_args);
}

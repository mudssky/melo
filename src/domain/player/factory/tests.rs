use crate::core::config::settings::Settings;
use crate::domain::player::factory::{BackendChoice, build_backend_for_choice};

#[test]
fn build_backend_for_choice_rejects_mpv_lib_until_backend_exists() {
    let settings = Settings::default();
    let err = build_backend_for_choice(BackendChoice::MpvLib, &settings)
        .err()
        .expect("mpv_lib should still be unavailable in task 1");
    assert!(err.to_string().contains("mpv_lib_backend_unavailable"));
}

#[test]
fn build_backend_for_choice_supports_rodio_and_mpv_ipc() {
    let settings = Settings::default();

    let rodio = build_backend_for_choice(BackendChoice::Rodio, &settings).unwrap();
    assert_eq!(rodio.backend_name(), "rodio");

    let mpv = build_backend_for_choice(BackendChoice::MpvIpc, &settings).unwrap();
    assert_eq!(mpv.backend_name(), "mpv_ipc");
}

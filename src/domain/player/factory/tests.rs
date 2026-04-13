use crate::core::config::settings::Settings;
use crate::domain::player::factory::{BackendChoice, build_backend_for_choice};

#[test]
fn build_backend_for_choice_supports_mpv_lib() {
    let settings = Settings::default();
    let backend = build_backend_for_choice(BackendChoice::MpvLib, &settings).unwrap();
    assert_eq!(backend.backend_name(), "mpv_lib");
}

#[test]
fn build_backend_for_choice_supports_rodio_and_mpv_ipc() {
    let settings = Settings::default();

    let rodio = build_backend_for_choice(BackendChoice::Rodio, &settings).unwrap();
    assert_eq!(rodio.backend_name(), "rodio");

    let mpv = build_backend_for_choice(BackendChoice::MpvIpc, &settings).unwrap();
    assert_eq!(mpv.backend_name(), "mpv_ipc");
}

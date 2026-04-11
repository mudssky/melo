use crate::core::config::settings::{MpvSettings, PlayerSettings};
use crate::domain::player::factory::{BackendChoice, resolve_backend_choice};

#[test]
fn auto_prefers_mpv_when_probe_succeeds() {
    let settings = PlayerSettings {
        backend: "auto".to_string(),
        mpv: MpvSettings {
            path: "mpv".to_string(),
            ipc_dir: "auto".to_string(),
            extra_args: Vec::new(),
        },
        ..PlayerSettings::default()
    };

    let choice = resolve_backend_choice(&settings, || true).unwrap();
    assert_eq!(choice, BackendChoice::Mpv);
}

#[test]
fn auto_falls_back_to_rodio_when_mpv_missing() {
    let choice = resolve_backend_choice(&PlayerSettings::default(), || false).unwrap();
    assert_eq!(choice, BackendChoice::Rodio);
}

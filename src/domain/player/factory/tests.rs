use crate::core::config::settings::{MpvSettings, PlayerSettings};
use crate::domain::player::factory::{BackendChoice, resolve_backend_choice};

#[test]
fn auto_prefers_mpv_ipc_when_probe_succeeds() {
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
    assert_eq!(choice, BackendChoice::MpvIpc);
}

#[test]
fn explicit_mpv_alias_maps_to_mpv_ipc() {
    let settings = PlayerSettings {
        backend: "mpv".to_string(),
        ..PlayerSettings::default()
    };

    let choice = resolve_backend_choice(&settings, || true).unwrap();
    assert_eq!(choice, BackendChoice::MpvIpc);
}

#[test]
fn explicit_mpv_lib_is_reserved_but_unavailable_in_phase_one() {
    let settings = PlayerSettings {
        backend: "mpv_lib".to_string(),
        ..PlayerSettings::default()
    };

    let err = resolve_backend_choice(&settings, || true).unwrap_err();
    assert!(err.to_string().contains("mpv_lib_backend_unavailable"));
}

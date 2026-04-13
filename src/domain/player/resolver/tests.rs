use crate::core::config::settings::PlayerSettings;
use crate::domain::player::factory::BackendChoice;
use crate::domain::player::resolver::{BackendAvailability, BackendResolver};

fn settings(backend: &str) -> PlayerSettings {
    PlayerSettings {
        backend: backend.to_string(),
        ..PlayerSettings::default()
    }
}

#[test]
fn auto_prefers_libmpv_then_mpv_ipc_then_rodio() {
    let resolver = BackendResolver::default();

    let libmpv = resolver.resolve_choice(
        &settings("auto"),
        BackendAvailability {
            mpv_lib: true,
            mpv_ipc: true,
            rodio: true,
        },
    );
    assert_eq!(libmpv.unwrap().choice, BackendChoice::MpvLib);

    let ipc = resolver.resolve_choice(
        &settings("auto"),
        BackendAvailability {
            mpv_lib: false,
            mpv_ipc: true,
            rodio: true,
        },
    );
    assert_eq!(ipc.unwrap().choice, BackendChoice::MpvIpc);

    let rodio = resolver.resolve_choice(
        &settings("auto"),
        BackendAvailability {
            mpv_lib: false,
            mpv_ipc: false,
            rodio: true,
        },
    );
    assert_eq!(rodio.unwrap().choice, BackendChoice::Rodio);
}

#[test]
fn auto_generates_user_visible_notice_when_fallback_happens() {
    let resolver = BackendResolver::default();
    let resolved = resolver
        .resolve_choice(
            &settings("auto"),
            BackendAvailability {
                mpv_lib: false,
                mpv_ipc: true,
                rodio: true,
            },
        )
        .unwrap();

    assert_eq!(resolved.choice, BackendChoice::MpvIpc);
    assert_eq!(
        resolved.notice.as_deref(),
        Some("mpv_lib unavailable, fell back to mpv_ipc")
    );
}

#[test]
fn explicit_backends_do_not_fallback() {
    let resolver = BackendResolver::default();

    let lib = resolver.resolve_choice(
        &settings("mpv_lib"),
        BackendAvailability {
            mpv_lib: true,
            mpv_ipc: true,
            rodio: true,
        },
    );
    assert_eq!(lib.unwrap().choice, BackendChoice::MpvLib);

    let err = resolver
        .resolve_choice(
            &settings("mpv_ipc"),
            BackendAvailability {
                mpv_lib: true,
                mpv_ipc: false,
                rodio: true,
            },
        )
        .unwrap_err();
    assert!(err.to_string().contains("mpv_backend_unavailable"));
}

#[test]
fn auto_falls_back_all_the_way_to_rodio_with_notice() {
    let resolver = BackendResolver::default();
    let resolved = resolver
        .resolve_choice(
            &settings("auto"),
            BackendAvailability {
                mpv_lib: false,
                mpv_ipc: false,
                rodio: true,
            },
        )
        .unwrap();

    assert_eq!(resolved.choice, BackendChoice::Rodio);
    assert_eq!(
        resolved.notice.as_deref(),
        Some("mpv_lib and mpv_ipc unavailable, fell back to rodio")
    );
}

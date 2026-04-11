use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use tokio::net::TcpListener;
use tokio::sync::Mutex;

use crate::core::config::settings::Settings;
use crate::daemon::manager::{restart_with_paths, start_with_paths, stop_with_paths};
use crate::daemon::observe::DaemonState;
use crate::daemon::registry::{DaemonPaths, store_registration_to};

fn daemon_paths(root: &std::path::Path) -> DaemonPaths {
    DaemonPaths {
        state_file: root.join("daemon.json"),
        log_file: root.join("daemon.log"),
    }
}

async fn spawn_registered_router(
    paths: &DaemonPaths,
    instance_id: &str,
) -> (
    crate::daemon::app::AppState,
    tokio::task::JoinHandle<()>,
    std::net::SocketAddr,
) {
    let state = crate::daemon::app::AppState::for_test_with_instance_id(instance_id);
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let registration = state.daemon_registration(addr);

    store_registration_to(&paths.state_file, &registration)
        .await
        .unwrap();

    let app = crate::daemon::server::router(state.clone());
    let shutdown_state = state.clone();
    let handle = tokio::spawn(async move {
        axum::serve(listener, app)
            .with_graceful_shutdown(async move {
                shutdown_state.wait_for_shutdown().await;
            })
            .await
            .unwrap();
    });

    (state, handle, addr)
}

#[tokio::test(flavor = "multi_thread")]
async fn start_with_paths_reuses_running_instance_without_spawning() {
    let temp = tempfile::tempdir().unwrap();
    let paths = daemon_paths(temp.path());
    let (state, handle, _addr) = spawn_registered_router(&paths, "running-instance").await;
    let calls = Arc::new(AtomicUsize::new(0));

    let result = start_with_paths(&Settings::default(), &paths, {
        let calls = Arc::clone(&calls);
        move || {
            calls.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    })
    .await
    .unwrap();

    assert_eq!(result.observation.state, DaemonState::Running);
    assert_eq!(
        result.observation.instance_id.as_deref(),
        Some("running-instance")
    );
    assert_eq!(calls.load(Ordering::SeqCst), 0);

    state.request_shutdown();
    handle.await.unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn restart_with_paths_waits_for_shutdown_and_accepts_new_instance() {
    let temp = tempfile::tempdir().unwrap();
    let paths = daemon_paths(temp.path());
    let (old_state, old_handle, _old_addr) = spawn_registered_router(&paths, "old-instance").await;
    let new_server = Arc::new(Mutex::new(None));

    let result = restart_with_paths(&Settings::default(), &paths, {
        let paths = paths.clone();
        let new_server = Arc::clone(&new_server);
        move || {
            tokio::spawn(async move {
                tokio::time::sleep(Duration::from_millis(100)).await;
                let spawned = spawn_registered_router(&paths, "new-instance").await;
                *new_server.lock().await = Some(spawned);
            });
            Ok(())
        }
    })
    .await
    .unwrap();

    assert_eq!(result.previous.instance_id.as_deref(), Some("old-instance"));
    assert_eq!(result.current.instance_id.as_deref(), Some("new-instance"));
    assert_eq!(result.current.state, DaemonState::Running);

    old_state.request_shutdown();
    old_handle.await.unwrap();

    let (new_state, new_handle, _new_addr) = new_server.lock().await.take().unwrap();
    new_state.request_shutdown();
    new_handle.await.unwrap();
}

#[tokio::test]
async fn stop_with_paths_clears_stale_registration_when_server_is_unreachable() {
    let temp = tempfile::tempdir().unwrap();
    let paths = daemon_paths(temp.path());
    tokio::fs::write(&paths.log_file, "stale log\n")
        .await
        .unwrap();
    store_registration_to(
        &paths.state_file,
        &crate::daemon::registry::DaemonRegistration {
            instance_id: "stale-instance".to_string(),
            base_url: "http://127.0.0.1:65530".to_string(),
            pid: 999_999,
            started_at: "2026-04-11T00:00:00Z".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            backend: "noop".to_string(),
            host: "127.0.0.1".to_string(),
            port: 65530,
            log_path: paths.log_file.to_string_lossy().to_string(),
        },
    )
    .await
    .unwrap();

    let result = stop_with_paths(&Settings::default(), &paths).await.unwrap();

    assert_eq!(result.action, "stale_registration_cleared");
    assert!(!paths.state_file.exists());
}

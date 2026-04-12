use std::sync::{Arc, Mutex};

use axum::body::{Body, to_bytes};
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode};
use futures_util::StreamExt;
use melo::domain::player::backend::{PlaybackBackend, PlaybackCommand};
use melo::domain::player::runtime::PlaybackRuntimeEvent;
use tokio::net::TcpListener;
use tokio::sync::broadcast;
use tokio_tungstenite::connect_async;
use tower::ServiceExt;

/// 为测试请求补充连接来源信息。
///
/// # 参数
/// - `request`：原始请求
/// - `addr`：连接来源地址
///
/// # 返回值
/// - `Request<Body>`：带连接信息的请求
fn with_connect_info(mut request: Request<Body>, addr: &str) -> Request<Body> {
    request
        .extensions_mut()
        .insert(ConnectInfo(addr.parse::<std::net::SocketAddr>().unwrap()));
    request
}

#[derive(Clone)]
struct EventedBackend {
    commands: Arc<Mutex<Vec<PlaybackCommand>>>,
    runtime_tx: broadcast::Sender<PlaybackRuntimeEvent>,
}

impl Default for EventedBackend {
    fn default() -> Self {
        let (runtime_tx, _) = broadcast::channel(16);
        Self {
            commands: Arc::new(Mutex::new(Vec::new())),
            runtime_tx,
        }
    }
}

impl EventedBackend {
    fn emit_track_end(&self, generation: u64) {
        let _ = self
            .runtime_tx
            .send(PlaybackRuntimeEvent::TrackEnded { generation });
    }
}

impl PlaybackBackend for EventedBackend {
    fn backend_name(&self) -> &'static str {
        "evented"
    }

    fn load_and_play(
        &self,
        path: &std::path::Path,
        generation: u64,
    ) -> melo::core::error::MeloResult<()> {
        self.commands.lock().unwrap().push(PlaybackCommand::Load {
            path: path.to_path_buf(),
            generation,
        });
        Ok(())
    }

    fn pause(&self) -> melo::core::error::MeloResult<()> {
        Ok(())
    }

    fn resume(&self) -> melo::core::error::MeloResult<()> {
        Ok(())
    }

    fn stop(&self) -> melo::core::error::MeloResult<()> {
        Ok(())
    }

    fn subscribe_runtime_events(&self) -> broadcast::Receiver<PlaybackRuntimeEvent> {
        self.runtime_tx.subscribe()
    }

    fn current_position(&self) -> Option<std::time::Duration> {
        None
    }

    fn set_volume(&self, _factor: f32) -> melo::core::error::MeloResult<()> {
        Ok(())
    }
}

#[tokio::test]
async fn api_health_and_player_status_endpoints_work() {
    let app = melo::daemon::app::test_router().await;

    let health = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/system/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(health.status(), StatusCode::OK);

    let status = app
        .oneshot(
            Request::builder()
                .uri("/api/player/status")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(status.status(), StatusCode::OK);
}

#[tokio::test]
async fn system_shutdown_endpoint_marks_app_state_for_shutdown() {
    let state = melo::daemon::app::AppState::for_test().await;
    let app = melo::daemon::server::router(state.clone());

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/system/shutdown")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::ACCEPTED);
    assert!(state.shutdown_requested());
}

#[tokio::test]
async fn system_status_endpoint_returns_managed_identity() {
    let app = melo::daemon::app::test_router().await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/system/status")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload: melo::api::response::ApiResponse<melo::api::system::DaemonStatusResponse> =
        serde_json::from_slice(&body).unwrap();
    let payload = payload.data.unwrap();

    assert_eq!(payload.backend, "noop");
    assert!(payload.instance_id.starts_with("test-instance"));
    assert!(payload.log_path.ends_with("daemon.log"));
    assert!(!payload.shutdown_requested);
}

#[tokio::test]
async fn system_status_endpoint_wraps_payload_in_api_response() {
    let app = melo::daemon::app::test_router().await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/system/status")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(payload["code"], 0);
    assert_eq!(payload["msg"], "ok");
    assert_eq!(payload["data"]["backend"], "noop");
}

#[tokio::test]
async fn open_endpoint_returns_structured_error_body() {
    let app = melo::daemon::app::test_router().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/open")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"target":"cover.jpg","mode":"replace"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(payload["code"], 1302);
    assert!(
        payload["msg"]
            .as_str()
            .is_some_and(|message| message.contains("unsupported"))
    );
    assert!(payload["data"].is_null());
}

#[tokio::test]
async fn queue_endpoints_and_ws_broadcast_share_snapshot_contract() {
    let app = melo::daemon::app::test_router().await;

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/queue/add")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"items":[{"song_id":1,"path":"tests/fixtures/full_test.mp3","title":"Blue Bird","duration_seconds":212.0}]}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let status = app
        .oneshot(
            Request::builder()
                .uri("/api/player/status")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(status.status(), StatusCode::OK);
}

#[tokio::test(flavor = "multi_thread")]
async fn api_websocket_route_accepts_connections() {
    let app = melo::daemon::app::test_router().await;
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let (stream, response) = connect_async(format!("ws://{addr}/api/ws/player"))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SWITCHING_PROTOCOLS);
    drop(stream);
}

#[tokio::test(flavor = "multi_thread")]
async fn websocket_receives_auto_advanced_snapshot_after_track_end() {
    let backend = Arc::new(EventedBackend::default());
    let state = melo::daemon::app::AppState::with_backend(backend.clone());
    state
        .player
        .append(melo::core::model::player::QueueItem {
            song_id: 1,
            path: "tests/fixtures/full_test.mp3".into(),
            title: "One".into(),
            duration_seconds: Some(212.0),
        })
        .await
        .unwrap();
    state
        .player
        .append(melo::core::model::player::QueueItem {
            song_id: 2,
            path: "tests/fixtures/full_test.mp3".into(),
            title: "Two".into(),
            duration_seconds: Some(212.0),
        })
        .await
        .unwrap();
    state.player.play().await.unwrap();

    let app = melo::daemon::server::router(state);
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let (mut stream, _response) = connect_async(format!("ws://{addr}/api/ws/player"))
        .await
        .unwrap();

    let _initial = stream.next().await.unwrap().unwrap();
    backend.emit_track_end(1);

    let advanced = stream.next().await.unwrap().unwrap();
    let text = advanced.into_text().unwrap();
    let snapshot: melo::core::model::player::PlayerSnapshot = serde_json::from_str(&text).unwrap();

    assert_eq!(snapshot.queue_index, Some(1));
    assert_eq!(snapshot.current_song.unwrap().title, "Two");
}

#[tokio::test(flavor = "multi_thread")]
async fn websocket_status_contract_includes_progress_fields() {
    let app = melo::daemon::app::test_router().await;
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let (mut stream, _response) = connect_async(format!("ws://{addr}/api/ws/player"))
        .await
        .unwrap();
    let message = stream.next().await.unwrap().unwrap();
    let snapshot: melo::core::model::player::PlayerSnapshot =
        serde_json::from_str(&message.into_text().unwrap()).unwrap();

    assert!(snapshot.position_seconds.is_some() || snapshot.position_seconds.is_none());
    assert!(snapshot.position_fraction.is_some() || snapshot.position_fraction.is_none());
}

#[tokio::test(flavor = "multi_thread")]
async fn api_tui_websocket_initial_snapshot_includes_active_task() {
    let state = melo::daemon::app::AppState::for_test().await;
    let handle = state
        .runtime_tasks()
        .start_scan("D:/Music/Aimer".to_string(), 4);
    handle.mark_indexing(2, 2, Some("track-02.flac".to_string()));
    let app = melo::daemon::server::router(state);
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let (mut stream, _response) = connect_async(format!("ws://{addr}/api/ws/tui"))
        .await
        .unwrap();
    let message = stream.next().await.unwrap().unwrap();
    let snapshot: melo::core::model::tui::TuiSnapshot =
        serde_json::from_str(&message.into_text().unwrap()).unwrap();

    assert_eq!(snapshot.player.backend_name, "noop");
    assert_eq!(snapshot.active_task.unwrap().indexed_count, 2);
}

#[tokio::test]
async fn player_volume_endpoint_updates_snapshot_contract() {
    let app = melo::daemon::app::test_router().await;
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/player/volume")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"volume_percent":55}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn player_volume_endpoint_returns_wrapped_snapshot() {
    let app = melo::daemon::app::test_router().await;
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/player/volume")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"volume_percent":55}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(payload["code"], 0);
    assert_eq!(payload["data"]["volume_percent"], 55);
}

#[tokio::test]
async fn queue_play_endpoint_returns_business_error_when_index_is_invalid() {
    let app = melo::daemon::app::test_router().await;
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/queue/play")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"index":99}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(payload["code"], 1102);
}

#[tokio::test]
async fn openapi_json_endpoint_is_available() {
    let app = melo::daemon::app::test_router().await;
    let response = app
        .oneshot(with_connect_info(
            Request::builder()
                .uri("/api/openapi.json")
                .body(Body::empty())
                .unwrap(),
            "127.0.0.1:38123",
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(payload["openapi"].is_string());
    assert!(payload["paths"]["/api/player/status"].is_object());
}

#[tokio::test]
async fn docs_route_is_disabled_when_docs_mode_is_disabled() {
    let mut settings = melo::core::config::settings::Settings::default();
    settings.daemon.docs = melo::core::config::settings::DaemonDocsMode::Disabled;
    let app = melo::daemon::app::test_router_with_settings(settings).await;

    let response = app
        .oneshot(with_connect_info(
            Request::builder()
                .uri("/api/docs/")
                .body(Body::empty())
                .unwrap(),
            "127.0.0.1:38123",
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn docs_route_rejects_non_loopback_when_docs_mode_is_local() {
    let mut settings = melo::core::config::settings::Settings::default();
    settings.daemon.docs = melo::core::config::settings::DaemonDocsMode::Local;
    let app = melo::daemon::app::test_router_with_settings(settings).await;

    let response = app
        .oneshot(with_connect_info(
            Request::builder()
                .uri("/api/docs/")
                .body(Body::empty())
                .unwrap(),
            "192.168.1.20:38123",
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn docs_page_endpoint_is_available() {
    let app = melo::daemon::app::test_router().await;
    let response = app
        .oneshot(with_connect_info(
            Request::builder()
                .uri("/api/docs/")
                .body(Body::empty())
                .unwrap(),
            "127.0.0.1:38123",
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

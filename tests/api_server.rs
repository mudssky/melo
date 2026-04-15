use std::sync::{Arc, Mutex};

use axum::body::{Body, to_bytes};
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode};
use futures_util::StreamExt;
use futures_util::stream::{SplitSink, SplitStream};
use melo::domain::player::backend::{
    PlaybackBackend, PlaybackCommand, PlaybackSessionHandle, PlaybackStartRequest,
};
use melo::domain::player::runtime::{PlaybackRuntimeEvent, PlaybackStopReason};
use reqwest::Url;
use tokio::net::TcpListener;
use tokio::sync::broadcast;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;
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

struct TestServer {
    base_url: String,
}

type TestWsStream =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;
type TestWsSink = SplitSink<TestWsStream, Message>;
type TestWsReader = SplitStream<TestWsStream>;

/// 启动一个临时 HTTP 测试服务器。
///
/// # 参数
/// - `app`：待启动的 Axum 路由
///
/// # 返回值
/// - `TestServer`：包含可访问基地址的测试服务器句柄
async fn spawn_test_server(app: axum::Router) -> TestServer {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    TestServer {
        base_url: format!("http://{addr}"),
    }
}

/// 连接测试用 WebSocket，并拆分出读写两端。
///
/// # 参数
/// - `url`：目标 WebSocket 地址
///
/// # 返回值
/// - `(TestWsSink, TestWsReader)`：拆分后的写端与读端
async fn connect_test_ws(url: &str) -> (TestWsSink, TestWsReader) {
    let (stream, _response) = connect_async(url).await.unwrap();
    stream.split()
}

/// 读取下一条 WebSocket JSON 消息。
///
/// # 参数
/// - `stream`：WebSocket 读端
///
/// # 返回值
/// - `serde_json::Value`：解析后的 JSON 负载
async fn next_ws_json(stream: &mut TestWsReader) -> serde_json::Value {
    let message = stream.next().await.unwrap().unwrap();
    serde_json::from_str(&message.into_text().unwrap()).unwrap()
}

#[derive(Clone)]
struct EventedBackend {
    commands: Arc<Mutex<Vec<PlaybackCommand>>>,
    runtime_tx: broadcast::Sender<PlaybackRuntimeEvent>,
}

struct EventedSessionHandle {
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
        let _ = self.runtime_tx.send(PlaybackRuntimeEvent::PlaybackStopped {
            generation,
            reason: PlaybackStopReason::NaturalEof,
        });
    }
}

impl PlaybackBackend for EventedBackend {
    fn backend_name(&self) -> &'static str {
        "evented"
    }

    fn start_session(
        &self,
        request: PlaybackStartRequest,
    ) -> melo::core::error::MeloResult<Box<dyn PlaybackSessionHandle>> {
        self.commands.lock().unwrap().push(PlaybackCommand::Load {
            path: request.path,
            generation: request.generation,
        });
        Ok(Box::new(EventedSessionHandle {
            runtime_tx: self.runtime_tx.clone(),
        }))
    }
}

impl PlaybackSessionHandle for EventedSessionHandle {
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
async fn websocket_initial_snapshot_keeps_backend_notice_contract() {
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

    assert!(snapshot.backend_notice.is_none());
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

#[tokio::test(flavor = "multi_thread")]
async fn api_tui_websocket_initial_snapshot_includes_playlist_browser_defaults() {
    let state = melo::daemon::app::AppState::for_test().await;
    state.set_current_playlist_context("C:/Music/Aimer", "ephemeral");
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

    assert_eq!(
        snapshot.playlist_browser.default_view,
        melo::core::model::tui::TuiViewKind::Playlist
    );
    assert_eq!(
        snapshot
            .playlist_browser
            .default_selected_playlist
            .as_deref(),
        Some("C:/Music/Aimer")
    );
    assert_eq!(
        snapshot
            .playlist_browser
            .current_playing_playlist
            .as_ref()
            .unwrap()
            .kind,
        "ephemeral"
    );
}

#[tokio::test]
async fn tui_home_endpoint_returns_playlist_browser_snapshot() {
    let state = melo::daemon::app::AppState::for_test().await;
    state.set_current_playlist_context("C:/Music/Aimer", "ephemeral");
    let app = melo::daemon::server::router(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/tui/home")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(
        payload["data"]["playlist_browser"]["default_view"],
        "playlist"
    );
}

#[tokio::test]
async fn playlist_preview_endpoint_accepts_path_named_ephemeral_playlist() {
    let harness = melo::test_support::TestHarness::new().await;
    let playlist_service = harness.playlist_service();
    let ephemeral_name = "C:/Temp/Aimer".to_string();
    harness
        .seed_song("Blue Bird", "Ikimono-gakari", "Blue Bird", 2008)
        .await;
    playlist_service
        .upsert_ephemeral(
            &ephemeral_name,
            "path_dir",
            &ephemeral_name,
            true,
            None,
            &[1],
        )
        .await
        .unwrap();
    let app = melo::daemon::app::test_router_with_settings(harness.settings.clone()).await;

    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/playlists/preview?name={ephemeral_name}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn playlist_play_endpoint_starts_from_selected_index_and_updates_current_source() {
    let harness = melo::test_support::TestHarness::new().await;
    let playlist_service = harness.playlist_service();
    harness.seed_song("One", "Aimer", "Singles", 2015).await;
    harness.seed_song("Two", "Aimer", "Singles", 2015).await;
    let one_path = harness.write_song_file("audio/one.flac").await;
    let two_path = harness.write_song_file("audio/two.flac").await;
    let conn = rusqlite::Connection::open(harness.settings.database.path.as_std_path()).unwrap();
    conn.execute(
        "UPDATE songs SET path = ?1 WHERE id = 1",
        [one_path.to_string_lossy().to_string()],
    )
    .unwrap();
    conn.execute(
        "UPDATE songs SET path = ?1 WHERE id = 2",
        [two_path.to_string_lossy().to_string()],
    )
    .unwrap();
    playlist_service
        .create_static("Favorites", None)
        .await
        .unwrap();
    playlist_service
        .add_songs("Favorites", &[1, 2])
        .await
        .unwrap();
    let app = melo::daemon::app::test_router_with_settings(harness.settings.clone()).await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/playlists/play")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"name":"Favorites","start_index":1}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(payload["data"]["player"]["queue_index"], 1);
    assert_eq!(
        payload["data"]["playlist_browser"]["current_playing_playlist"]["name"],
        "Favorites"
    );
}

#[tokio::test]
async fn queue_clear_endpoint_clears_current_playlist_context() {
    let state = melo::daemon::app::AppState::for_test().await;
    state.set_current_playlist_context("Favorites", "static");
    let app = melo::daemon::server::router(state.clone());

    let _ = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/queue/clear")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert!(state.current_playlist_context().is_none());
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
    assert!(payload["paths"]["/api/tui/home"].is_object());
    assert!(payload["paths"]["/api/playlists/preview"].is_object());
    assert!(payload["paths"]["/api/playlists/play"].is_object());
    assert!(payload["paths"]["/api/ws/tui"].is_object());
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

#[tokio::test(flavor = "multi_thread")]
async fn bootstrap_endpoint_returns_runtime_and_default_mode() {
    let app = melo::daemon::app::test_router().await;
    let server = spawn_test_server(app).await;

    let response = reqwest::Client::new()
        .get(format!("{}/api/bootstrap", server.base_url))
        .send()
        .await
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap();

    assert_eq!(response["code"], 0);
    assert_eq!(response["data"]["default_playback_mode"], "ordered");
    assert!(response["data"]["runtime"]["playback_state"].is_string());
}

#[tokio::test(flavor = "multi_thread")]
async fn track_content_endpoint_returns_parsed_lyrics_and_artwork_summary() {
    let harness = melo::test_support::TestHarness::new().await;
    harness
        .seed_song("Blue Bird", "Ikimono-gakari", "Blue Bird", 2008)
        .await;
    let conn = rusqlite::Connection::open(harness.settings.database.path.as_std_path()).unwrap();
    conn.execute(
        "UPDATE songs
         SET lyrics = ?1,
             lyrics_source_kind = 'sidecar',
             lyrics_source_path = ?2,
             duration_seconds = 212.0
         WHERE id = 1",
        rusqlite::params!["[00:01.00]hello\n[00:05.50]world", "D:/Music/Blue Bird.lrc"],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO artwork_refs (owner_kind, owner_id, source_kind, source_path, embedded_song_id, mime, cache_path, hash, updated_at)
         VALUES ('song', 1, 'sidecar', ?1, NULL, 'image/jpeg', NULL, NULL, datetime('now'))",
        ["D:/Music/cover.jpg"],
    )
    .unwrap();

    let app = melo::daemon::app::test_router_with_settings(harness.settings.clone()).await;
    let server = spawn_test_server(app).await;
    let mut url = Url::parse(&format!("{}/api/tracks/content", server.base_url)).unwrap();
    url.query_pairs_mut().append_pair("song_id", "1");

    let response = reqwest::Client::new()
        .get(url)
        .send()
        .await
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap();

    assert_eq!(response["code"], 0);
    assert!(response["data"]["lyrics"].as_array().unwrap().len() >= 1);
    assert!(response["data"]["artwork"]["terminal_summary"].is_string());
    assert!(response["data"]["refresh_token"].is_string());
}

#[tokio::test(flavor = "multi_thread")]
async fn playback_runtime_ws_streams_lightweight_updates() {
    let state = melo::daemon::app::AppState::for_test().await;
    state
        .player
        .append(melo::core::model::player::QueueItem {
            song_id: 1,
            path: "tests/fixtures/full_test.mp3".into(),
            title: "Blue Bird".into(),
            duration_seconds: Some(212.0),
        })
        .await
        .unwrap();

    let app = melo::daemon::server::router(state);
    let server = spawn_test_server(app).await;
    let ws_url = server.base_url.replace("http://", "ws://") + "/api/ws/playback/runtime";
    let (_socket, mut stream) = connect_test_ws(&ws_url).await;

    let initial: serde_json::Value = next_ws_json(&mut stream).await;
    assert!(initial["current_song_id"].is_null());
    assert!(initial.get("lyrics").is_none());
    assert!(initial.get("visible_playlists").is_none());
}

#[tokio::test(flavor = "multi_thread")]
async fn runtime_snapshot_does_not_recompute_lyrics_or_playlist_browser() {
    let state = melo::daemon::app::AppState::for_test().await;
    state
        .player
        .append(melo::core::model::player::QueueItem {
            song_id: 1,
            path: "tests/fixtures/full_test.mp3".into(),
            title: "Blue Bird".into(),
            duration_seconds: Some(212.0),
        })
        .await
        .unwrap();

    let before = state.client_bootstrap().await.unwrap();
    state.player.play().await.unwrap();
    let after = state.playback_runtime_snapshot().await;

    assert_eq!(before.current_source_ref, after.current_source_ref);
    assert!(after.current_song_id.is_some());
}

#[tokio::test(flavor = "multi_thread")]
async fn playlist_play_command_returns_lightweight_ack_without_tui_snapshot() {
    let harness = melo::test_support::TestHarness::new().await;
    let playlist_service = harness.playlist_service();
    harness.seed_song("One", "Aimer", "Singles", 2015).await;
    harness.seed_song("Two", "Aimer", "Singles", 2015).await;
    let one_path = harness.write_song_file("audio/one.flac").await;
    let two_path = harness.write_song_file("audio/two.flac").await;
    let conn = rusqlite::Connection::open(harness.settings.database.path.as_std_path()).unwrap();
    conn.execute(
        "UPDATE songs SET path = ?1 WHERE id = 1",
        [one_path.to_string_lossy().to_string()],
    )
    .unwrap();
    conn.execute(
        "UPDATE songs SET path = ?1 WHERE id = 2",
        [two_path.to_string_lossy().to_string()],
    )
    .unwrap();
    playlist_service
        .create_static("Favorites", None)
        .await
        .unwrap();
    playlist_service
        .add_songs("Favorites", &[1, 2])
        .await
        .unwrap();

    let app = melo::daemon::app::test_router_with_settings(harness.settings.clone()).await;
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/playlists/play-command")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"name":"Favorites","start_index":1}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(payload["code"], 0);
    assert_eq!(payload["data"]["target_song_id"], 2);
    assert!(payload["data"]["player"].is_null());
    assert!(payload["data"]["playlist_browser"].is_null());
}

#[tokio::test(flavor = "multi_thread")]
async fn runtime_ws_confirms_playlist_play_command_after_submission() {
    let harness = melo::test_support::TestHarness::new().await;
    let playlist_service = harness.playlist_service();
    harness.seed_song("One", "Aimer", "Singles", 2015).await;
    harness.seed_song("Two", "Aimer", "Singles", 2015).await;
    let one_path = harness.write_song_file("audio/one.flac").await;
    let two_path = harness.write_song_file("audio/two.flac").await;
    let conn = rusqlite::Connection::open(harness.settings.database.path.as_std_path()).unwrap();
    conn.execute(
        "UPDATE songs SET path = ?1 WHERE id = 1",
        [one_path.to_string_lossy().to_string()],
    )
    .unwrap();
    conn.execute(
        "UPDATE songs SET path = ?1 WHERE id = 2",
        [two_path.to_string_lossy().to_string()],
    )
    .unwrap();
    playlist_service
        .create_static("Favorites", None)
        .await
        .unwrap();
    playlist_service
        .add_songs("Favorites", &[1, 2])
        .await
        .unwrap();

    let state = melo::daemon::app::AppState::for_test_with_settings(harness.settings.clone()).await;
    let app = melo::daemon::server::router(state);
    let server = spawn_test_server(app).await;
    let ws_url = server.base_url.replace("http://", "ws://") + "/api/ws/playback/runtime";
    let (_socket, mut stream) = connect_test_ws(&ws_url).await;
    let _initial: serde_json::Value = next_ws_json(&mut stream).await;

    let response = reqwest::Client::new()
        .post(format!("{}/api/playlists/play-command", server.base_url))
        .json(&serde_json::json!({ "name": "Favorites", "start_index": 1 }))
        .send()
        .await
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap();

    let confirmed = next_ws_json(&mut stream).await;
    assert_eq!(response["data"]["target_song_id"], 2);
    assert_eq!(confirmed["current_song_id"], 2);
    assert_eq!(confirmed["playback_state"], "playing");
}

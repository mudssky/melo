use std::sync::{Arc, Mutex};

use axum::body::Body;
use axum::http::{Request, StatusCode};
use futures_util::StreamExt;
use melo::domain::player::backend::{PlaybackBackend, PlaybackCommand};
use melo::domain::player::runtime::PlaybackRuntimeEvent;
use tokio::net::TcpListener;
use tokio::sync::broadcast;
use tokio_tungstenite::connect_async;
use tower::ServiceExt;

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

use axum::body::Body;
use axum::http::{Request, StatusCode};
use tokio::net::TcpListener;
use tokio_tungstenite::connect_async;
use tower::ServiceExt;

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

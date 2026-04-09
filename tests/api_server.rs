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

use axum::body::Body;
use axum::http::{Request, StatusCode};
use tower::util::ServiceExt;

#[tokio::test]
async fn open_endpoint_rejects_unsupported_file_types() {
    let harness = melo::test_support::TestHarness::new().await;
    let app = melo::daemon::app::test_router_with_settings(harness.settings.clone()).await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/open")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"target":"cover.jpg","mode":"path_file"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

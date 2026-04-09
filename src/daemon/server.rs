use axum::Router;
use tower_http::trace::TraceLayer;

use crate::daemon::app::AppState;

/// 构建 daemon 路由。
///
/// # 参数
/// - `state`：应用状态
///
/// # 返回值
/// - `Router`：Axum 路由
pub fn router(state: AppState) -> Router {
    Router::new()
        .route(
            "/api/system/health",
            axum::routing::get(crate::api::system::health),
        )
        .route(
            "/api/player/status",
            axum::routing::get(crate::api::player::status),
        )
        .route(
            "/api/ws/player",
            axum::routing::get(crate::api::ws::player_updates),
        )
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

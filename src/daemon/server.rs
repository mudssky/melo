use axum::Router;

use crate::daemon::app::AppState;

/// 构造 daemon 路由。
///
/// # 参数
/// - `state`：应用状态
///
/// # 返回
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
        .with_state(state)
}

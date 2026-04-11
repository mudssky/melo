use axum::{Json, extract::State, http::StatusCode};
use serde::Serialize;

use crate::daemon::app::AppState;

/// 健康检查响应。
#[derive(Debug, Serialize)]
pub struct HealthResponse {
    /// 服务状态。
    pub status: &'static str,
}

/// 返回 daemon 健康状态。
///
/// # 参数
/// - 无
///
/// # 返回
/// - `Json<HealthResponse>`：健康检查响应
pub async fn health() -> Json<HealthResponse> {
    Json(HealthResponse { status: "ok" })
}

/// 请求 daemon 优雅退出。
///
/// # 参数
/// - `state`：应用状态
///
/// # 返回
/// - `StatusCode`：接受关闭请求时返回 `202 Accepted`
pub async fn shutdown(State(state): State<AppState>) -> StatusCode {
    state.request_shutdown();
    StatusCode::ACCEPTED
}

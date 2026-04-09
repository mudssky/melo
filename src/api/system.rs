use axum::Json;
use serde::Serialize;

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

use axum::{Json, extract::State, http::StatusCode};
use serde::{Deserialize, Serialize};

use crate::daemon::app::AppState;

/// 健康检查响应。
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HealthResponse {
    /// 服务状态。
    pub status: String,
    /// 当前实例 ID。
    pub instance_id: String,
}

/// daemon 系统状态响应。
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DaemonStatusResponse {
    /// 当前实例 ID。
    pub instance_id: String,
    /// 当前进程 ID。
    pub pid: u32,
    /// daemon 启动时间。
    pub started_at: String,
    /// 当前 Melo 版本。
    pub version: String,
    /// 当前后端名。
    pub backend: String,
    /// 固定日志文件路径。
    pub log_path: String,
    /// 是否已收到关闭请求。
    pub shutdown_requested: bool,
}

/// 返回 daemon 健康状态。
///
/// # 参数
/// - `state`：应用状态
///
/// # 返回值
/// - `Json<HealthResponse>`：健康检查响应
pub async fn health(State(state): State<AppState>) -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".to_string(),
        instance_id: state.system_status().instance_id,
    })
}

/// 返回 daemon 的系统身份与运行状态。
///
/// # 参数
/// - `state`：应用状态
///
/// # 返回值
/// - `Json<DaemonStatusResponse>`：系统状态响应
pub async fn status(State(state): State<AppState>) -> Json<DaemonStatusResponse> {
    Json(state.system_status())
}

/// 请求 daemon 优雅退出。
///
/// # 参数
/// - `state`：应用状态
///
/// # 返回值
/// - `StatusCode`：接受关闭请求时返回 `202 Accepted`
pub async fn shutdown(State(state): State<AppState>) -> StatusCode {
    state.request_shutdown();
    StatusCode::ACCEPTED
}

use axum::{Json, extract::State, http::StatusCode};

use crate::daemon::app::AppState;
use crate::domain::open::service::{OpenRequest, OpenResponse};

/// 请求 daemon 直接打开一个目标路径。
///
/// # 参数
/// - `state`：应用状态
/// - `request`：直接打开请求
///
/// # 返回值
/// - `Result<Json<OpenResponse>, (StatusCode, String)>`：打开结果或错误响应
pub async fn open(
    State(state): State<AppState>,
    Json(request): Json<OpenRequest>,
) -> Result<Json<OpenResponse>, (StatusCode, String)> {
    state
        .open_target(request)
        .await
        .map(Json)
        .map_err(|err| (StatusCode::BAD_REQUEST, err.to_string()))
}

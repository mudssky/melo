use axum::{Json, extract::State};

use crate::api::error::ApiError;
use crate::api::response::ApiResponse;
use crate::daemon::app::AppState;
use crate::domain::open::service::{OpenRequest, OpenResponse};

/// 请求 daemon 直接打开一个目标路径。
///
/// # 参数
/// - `state`：应用状态
/// - `request`：直接打开请求
///
/// # 返回值
/// - `Result<Json<ApiResponse<OpenResponse>>, ApiError>`：打开结果或错误响应
#[utoipa::path(
    post,
    path = "/api/open",
    request_body = OpenRequest,
    responses(
        (status = 200, description = "直接打开目标成功", body = crate::api::response::ApiResponse<OpenResponse>),
        (status = 400, description = "目标类型不支持或请求无效", body = crate::api::response::ApiResponse<serde_json::Value>),
        (status = 409, description = "目标可识别但没有可播放内容", body = crate::api::response::ApiResponse<serde_json::Value>)
    )
)]
pub async fn open(
    State(state): State<AppState>,
    Json(request): Json<OpenRequest>,
) -> Result<Json<ApiResponse<OpenResponse>>, ApiError> {
    state
        .open_target(request)
        .await
        .map(ApiResponse::ok)
        .map(Json)
        .map_err(ApiError::from)
}

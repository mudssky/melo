use axum::{Json, extract::State};

use crate::api::error::ApiError;
use crate::api::response::ApiResponse;
use crate::daemon::app::AppState;

/// 返回新客户端初始化所需的低频 bootstrap 快照。
///
/// # 参数
/// - `state`：应用状态
///
/// # 返回值
/// - `Result<Json<ApiResponse<ClientBootstrapSnapshot>>, ApiError>`：bootstrap 快照响应
pub async fn show(
    State(state): State<AppState>,
) -> Result<
    Json<ApiResponse<crate::core::model::playback_runtime::ClientBootstrapSnapshot>>,
    ApiError,
> {
    state
        .client_bootstrap()
        .await
        .map(ApiResponse::ok)
        .map(Json)
        .map_err(ApiError::from)
}

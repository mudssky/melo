use axum::{Json, extract::State};

use crate::api::error::ApiError;
use crate::api::response::ApiResponse;
use crate::daemon::app::AppState;

/// 返回 TUI 首页聚合快照。
///
/// # 参数
/// - `state`：应用状态
///
/// # 返回值
/// - `Result<Json<ApiResponse<crate::core::model::tui::TuiSnapshot>>, ApiError>`：TUI 首页快照
#[utoipa::path(
    get,
    path = "/api/tui/home",
    responses(
        (status = 200, description = "TUI 首页聚合快照", body = crate::api::response::ApiResponse<crate::core::model::tui::TuiSnapshot>),
        (status = 500, description = "快照聚合失败", body = crate::api::response::ApiResponse<serde_json::Value>)
    )
)]
pub async fn home(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<crate::core::model::tui::TuiSnapshot>>, ApiError> {
    state
        .legacy_tui_snapshot()
        .await
        .map(ApiResponse::ok)
        .map(Json)
        .map_err(ApiError::from)
}

use axum::{
    Json,
    extract::{Query, State},
};
use serde::Deserialize;

use crate::api::error::ApiError;
use crate::api::response::ApiResponse;
use crate::daemon::app::AppState;

/// 曲目内容查询参数。
#[derive(Debug, Deserialize)]
pub struct TrackContentQuery {
    /// 歌曲 ID。
    pub song_id: i64,
}

/// 返回指定歌曲的低频内容快照。
///
/// # 参数
/// - `state`：应用状态
/// - `query`：曲目内容查询参数
///
/// # 返回值
/// - `Result<Json<ApiResponse<crate::core::model::track_content::TrackContentSnapshot>>, ApiError>`：曲目内容响应
pub async fn content(
    State(state): State<AppState>,
    Query(query): Query<TrackContentQuery>,
) -> Result<Json<ApiResponse<crate::core::model::track_content::TrackContentSnapshot>>, ApiError> {
    state
        .library
        .track_content(query.song_id)
        .await
        .map(ApiResponse::ok)
        .map(Json)
        .map_err(ApiError::from)
}

/// 强制刷新并返回指定歌曲的低频内容快照。
///
/// # 参数
/// - `state`：应用状态
/// - `query`：曲目内容查询参数
///
/// # 返回值
/// - `Result<Json<ApiResponse<crate::core::model::track_content::TrackContentSnapshot>>, ApiError>`：刷新后的曲目内容响应
pub async fn refresh(
    State(state): State<AppState>,
    Query(query): Query<TrackContentQuery>,
) -> Result<Json<ApiResponse<crate::core::model::track_content::TrackContentSnapshot>>, ApiError> {
    state
        .library
        .refresh_track_content(query.song_id)
        .await
        .map(ApiResponse::ok)
        .map(Json)
        .map_err(ApiError::from)
}

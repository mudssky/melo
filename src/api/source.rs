use axum::{
    Json,
    extract::{Query, State},
};
use serde::Deserialize;

use crate::api::error::ApiError;
use crate::api::response::ApiResponse;
use crate::daemon::app::AppState;

/// 来源曲目列表查询参数。
#[derive(Debug, Deserialize)]
pub struct SourceTracksQuery {
    /// 来源名称。
    pub name: String,
}

/// 返回当前可播放来源列表。
///
/// # 参数
/// - `state`：应用状态
///
/// # 返回值
/// - `Result<Json<ApiResponse<Vec<crate::core::model::tui::PlaylistListItem>>>, ApiError>`：来源列表响应
pub async fn list(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Vec<crate::core::model::tui::PlaylistListItem>>>, ApiError> {
    let mut sources = state
        .playlists
        .list_all()
        .await?
        .into_iter()
        .map(|playlist| crate::core::model::tui::PlaylistListItem {
            is_current_playing_source: false,
            is_ephemeral: playlist.kind == "ephemeral",
            name: playlist.name,
            kind: playlist.kind,
            count: playlist.count,
        })
        .collect::<Vec<_>>();
    sources.sort_by(|left, right| left.name.cmp(&right.name));

    Ok(Json(ApiResponse::ok(sources)))
}

/// 返回指定来源内的歌曲列表。
///
/// # 参数
/// - `state`：应用状态
/// - `query`：来源查询参数
///
/// # 返回值
/// - `Result<Json<ApiResponse<Vec<crate::core::model::player::QueueItem>>>, ApiError>`：来源曲目列表响应
pub async fn tracks(
    State(state): State<AppState>,
    Query(query): Query<SourceTracksQuery>,
) -> Result<Json<ApiResponse<Vec<crate::core::model::player::QueueItem>>>, ApiError> {
    state
        .playlists
        .source_tracks(&query.name)
        .await
        .map(ApiResponse::ok)
        .map(Json)
        .map_err(ApiError::from)
}

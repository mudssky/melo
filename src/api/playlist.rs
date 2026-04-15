use axum::{
    Json,
    extract::{Query, State},
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::api::error::ApiError;
use crate::api::response::ApiResponse;
use crate::daemon::app::AppState;

/// 歌单预览查询参数。
#[derive(Debug, Deserialize, ToSchema)]
pub struct PlaylistPreviewQuery {
    /// 歌单名。
    pub name: String,
}

/// 歌单播放请求。
#[derive(Debug, Deserialize, ToSchema)]
pub struct PlaylistPlayRequest {
    /// 歌单名。
    pub name: String,
    /// 起播索引。
    pub start_index: usize,
}

/// 预览歌曲摘要。
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PlaylistPreviewSong {
    /// 歌曲 ID。
    pub id: i64,
    /// 歌曲标题。
    pub title: String,
}

/// 歌单预览响应。
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PlaylistPreviewResponse {
    /// 歌单名。
    pub name: String,
    /// 预览歌曲列表。
    pub songs: Vec<PlaylistPreviewSong>,
}

/// 轻量歌单播放命令响应。
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PlaylistPlayCommandResponse {
    /// 提交的来源名。
    pub source_name: String,
    /// 提交的来源类型。
    pub source_kind: String,
    /// 目标歌曲 ID。
    pub target_song_id: i64,
    /// 目标来源内索引。
    pub target_index: usize,
    /// daemon 接收命令后的最新代次。
    pub accepted_generation: u64,
}

/// 预览歌单内容。
///
/// # 参数
/// - `state`：应用状态
/// - `query`：查询参数
///
/// # 返回值
/// - `Result<Json<ApiResponse<PlaylistPreviewResponse>>, ApiError>`：歌单预览结果
#[utoipa::path(
    get,
    path = "/api/playlists/preview",
    params(("name" = String, Query, description = "歌单名")),
    responses((status = 200, body = crate::api::response::ApiResponse<PlaylistPreviewResponse>))
)]
pub async fn preview(
    State(state): State<AppState>,
    Query(query): Query<PlaylistPreviewQuery>,
) -> Result<Json<ApiResponse<PlaylistPreviewResponse>>, ApiError> {
    let songs = state
        .playlists
        .preview(&query.name)
        .await?
        .into_iter()
        .map(|song| PlaylistPreviewSong {
            id: song.id,
            title: song.title,
        })
        .collect::<Vec<_>>();

    Ok(Json(ApiResponse::ok(PlaylistPreviewResponse {
        name: query.name,
        songs,
    })))
}

/// 从指定歌单和起播索引开始播放。
///
/// # 参数
/// - `state`：应用状态
/// - `request`：播放请求
///
/// # 返回值
/// - `Result<Json<ApiResponse<crate::core::model::tui::TuiSnapshot>>, ApiError>`：新的 TUI 聚合快照
#[utoipa::path(
    post,
    path = "/api/playlists/play",
    request_body = PlaylistPlayRequest,
    responses((status = 200, body = crate::api::response::ApiResponse<crate::core::model::tui::TuiSnapshot>))
)]
pub async fn play(
    State(state): State<AppState>,
    Json(request): Json<PlaylistPlayRequest>,
) -> Result<Json<ApiResponse<crate::core::model::tui::TuiSnapshot>>, ApiError> {
    let items = state.playlists.queue_items(&request.name).await?;
    let kind = state
        .playlists
        .list_all()
        .await?
        .into_iter()
        .find(|playlist| playlist.name == request.name)
        .map(|playlist| playlist.kind)
        .unwrap_or_else(|| "static".to_string());

    state
        .player
        .replace_queue(items, request.start_index)
        .await?;
    state.set_current_playlist_context(&request.name, &kind);

    state
        .tui_snapshot()
        .await
        .map(ApiResponse::ok)
        .map(Json)
        .map_err(ApiError::from)
}

/// 轻量提交歌单播放命令，不返回整页 TUI 聚合快照。
///
/// # 参数
/// - `state`：应用状态
/// - `request`：播放请求
///
/// # 返回值
/// - `Result<Json<ApiResponse<PlaylistPlayCommandResponse>>, ApiError>`：轻量播放命令结果
#[utoipa::path(
    post,
    path = "/api/playlists/play-command",
    request_body = PlaylistPlayRequest,
    responses((status = 200, body = crate::api::response::ApiResponse<PlaylistPlayCommandResponse>))
)]
pub async fn play_command(
    State(state): State<AppState>,
    Json(request): Json<PlaylistPlayRequest>,
) -> Result<Json<ApiResponse<PlaylistPlayCommandResponse>>, ApiError> {
    state
        .submit_playlist_play_command(&request.name, request.start_index)
        .await
        .map(ApiResponse::ok)
        .map(Json)
        .map_err(ApiError::from)
}

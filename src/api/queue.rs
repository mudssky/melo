use axum::{Json, extract::State};
use utoipa::ToSchema;

use crate::api::error::ApiError;
use crate::api::response::ApiResponse;
use crate::core::model::player::{PlayerSnapshot, QueueItem};
use crate::daemon::app::AppState;

/// 队列批量追加请求。
#[derive(Debug, serde::Deserialize, ToSchema)]
pub struct QueueAddRequest {
    /// 待追加的队列项列表。
    pub items: Vec<QueueItem>,
}

/// 基于单索引的队列请求。
#[derive(Debug, serde::Deserialize, ToSchema)]
pub struct QueueIndexRequest {
    /// 目标索引。
    pub index: usize,
}

/// 队列插入请求。
#[derive(Debug, serde::Deserialize, ToSchema)]
pub struct QueueInsertRequest {
    /// 插入位置。
    pub index: usize,
    /// 待插入队列项。
    pub item: QueueItem,
}

/// 队列删除请求。
#[derive(Debug, serde::Deserialize, ToSchema)]
pub struct QueueRemoveRequest {
    /// 待删除索引。
    pub index: usize,
}

/// 队列移动请求。
#[derive(Debug, serde::Deserialize, ToSchema)]
pub struct QueueMoveRequest {
    /// 源索引。
    pub from: usize,
    /// 目标索引。
    pub to: usize,
}

/// 批量向队列尾部追加歌曲。
///
/// # 参数
/// - `state`：应用状态
/// - `request`：追加请求
///
/// # 返回值
/// - `Json<PlayerSnapshot>`：最新播放器快照
#[utoipa::path(
    post,
    path = "/api/queue/add",
    request_body = QueueAddRequest,
    responses(
        (status = 200, body = crate::api::response::ApiResponse<PlayerSnapshot>),
        (status = 500, body = crate::api::response::ApiResponse<serde_json::Value>)
    )
)]
pub async fn add(
    State(state): State<AppState>,
    Json(request): Json<QueueAddRequest>,
) -> Result<Json<ApiResponse<PlayerSnapshot>>, ApiError> {
    let mut snapshot = state.player.snapshot().await;
    for item in request.items {
        snapshot = state.player.append(item).await.map_err(ApiError::from)?;
    }
    state.clear_current_playlist_context();

    Ok(Json(ApiResponse::ok(snapshot)))
}

/// 在指定位置插入歌曲。
///
/// # 参数
/// - `state`：应用状态
/// - `request`：插入请求
///
/// # 返回值
/// - `Json<PlayerSnapshot>`：最新播放器快照
#[utoipa::path(
    post,
    path = "/api/queue/insert",
    request_body = QueueInsertRequest,
    responses(
        (status = 200, body = crate::api::response::ApiResponse<PlayerSnapshot>),
        (status = 400, body = crate::api::response::ApiResponse<serde_json::Value>),
        (status = 500, body = crate::api::response::ApiResponse<serde_json::Value>)
    )
)]
pub async fn insert(
    State(state): State<AppState>,
    Json(request): Json<QueueInsertRequest>,
) -> Result<Json<ApiResponse<PlayerSnapshot>>, ApiError> {
    let snapshot = state
        .player
        .insert(request.index, request.item)
        .await
        .map_err(ApiError::from)?;
    state.clear_current_playlist_context();
    Ok(Json(ApiResponse::ok(snapshot)))
}

/// 清空整个播放队列。
///
/// # 参数
/// - `state`：应用状态
///
/// # 返回值
/// - `Json<PlayerSnapshot>`：最新播放器快照
#[utoipa::path(post, path = "/api/queue/clear", responses((status = 200, body = crate::api::response::ApiResponse<PlayerSnapshot>), (status = 500, body = crate::api::response::ApiResponse<serde_json::Value>)))]
pub async fn clear(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<PlayerSnapshot>>, ApiError> {
    let snapshot = state.player.clear().await.map_err(ApiError::from)?;
    state.clear_current_playlist_context();
    Ok(Json(ApiResponse::ok(snapshot)))
}

/// 选择指定队列索引并播放。
///
/// # 参数
/// - `state`：应用状态
/// - `request`：播放请求
///
/// # 返回值
/// - `Json<PlayerSnapshot>`：最新播放器快照
#[utoipa::path(
    post,
    path = "/api/queue/play",
    request_body = QueueIndexRequest,
    responses(
        (status = 200, body = crate::api::response::ApiResponse<PlayerSnapshot>),
        (status = 400, body = crate::api::response::ApiResponse<serde_json::Value>),
        (status = 409, body = crate::api::response::ApiResponse<serde_json::Value>),
        (status = 500, body = crate::api::response::ApiResponse<serde_json::Value>)
    )
)]
pub async fn play_index(
    State(state): State<AppState>,
    Json(request): Json<QueueIndexRequest>,
) -> Result<Json<ApiResponse<PlayerSnapshot>>, ApiError> {
    state
        .player
        .play_index(request.index)
        .await
        .map(ApiResponse::ok)
        .map(Json)
        .map_err(ApiError::from)
}

/// 删除指定队列项。
///
/// # 参数
/// - `state`：应用状态
/// - `request`：删除请求
///
/// # 返回值
/// - `Json<PlayerSnapshot>`：最新播放器快照
#[utoipa::path(
    post,
    path = "/api/queue/remove",
    request_body = QueueRemoveRequest,
    responses(
        (status = 200, body = crate::api::response::ApiResponse<PlayerSnapshot>),
        (status = 400, body = crate::api::response::ApiResponse<serde_json::Value>),
        (status = 500, body = crate::api::response::ApiResponse<serde_json::Value>)
    )
)]
pub async fn remove(
    State(state): State<AppState>,
    Json(request): Json<QueueRemoveRequest>,
) -> Result<Json<ApiResponse<PlayerSnapshot>>, ApiError> {
    let snapshot = state
        .player
        .remove(request.index)
        .await
        .map_err(ApiError::from)?;
    state.clear_current_playlist_context();
    Ok(Json(ApiResponse::ok(snapshot)))
}

/// 移动指定队列项。
///
/// # 参数
/// - `state`：应用状态
/// - `request`：移动请求
///
/// # 返回值
/// - `Json<PlayerSnapshot>`：最新播放器快照
#[utoipa::path(
    post,
    path = "/api/queue/move",
    request_body = QueueMoveRequest,
    responses(
        (status = 200, body = crate::api::response::ApiResponse<PlayerSnapshot>),
        (status = 400, body = crate::api::response::ApiResponse<serde_json::Value>),
        (status = 500, body = crate::api::response::ApiResponse<serde_json::Value>)
    )
)]
pub async fn move_item(
    State(state): State<AppState>,
    Json(request): Json<QueueMoveRequest>,
) -> Result<Json<ApiResponse<PlayerSnapshot>>, ApiError> {
    let snapshot = state
        .player
        .move_item(request.from, request.to)
        .await
        .map_err(ApiError::from)?;
    state.clear_current_playlist_context();
    Ok(Json(ApiResponse::ok(snapshot)))
}

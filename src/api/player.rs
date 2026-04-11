use axum::{Json, extract::State};
use utoipa::ToSchema;

use crate::api::error::ApiError;
use crate::api::response::ApiResponse;
use crate::core::model::player::RepeatMode;
use crate::daemon::app::AppState;

/// 调整播放器音量的请求体。
#[derive(Debug, serde::Deserialize, ToSchema)]
pub struct PlayerVolumeRequest {
    /// 目标音量百分比。
    pub volume_percent: u8,
}

/// 调整播放器模式的请求体。
#[derive(Debug, serde::Deserialize, ToSchema)]
pub struct PlayerModeRequest {
    /// 目标循环模式。
    pub repeat_mode: Option<String>,
    /// 是否启用随机播放。
    pub shuffle_enabled: Option<bool>,
}

/// 返回当前播放器状态快照。
///
/// # 参数
/// - `state`：应用状态
///
/// # 返回
/// - `Json<ApiResponse<PlayerSnapshot>>`：播放器快照
#[utoipa::path(
    get,
    path = "/api/player/status",
    responses(
        (status = 200, description = "当前播放器状态", body = crate::api::response::ApiResponse<crate::core::model::player::PlayerSnapshot>)
    )
)]
pub async fn status(
    State(state): State<AppState>,
) -> Json<ApiResponse<crate::core::model::player::PlayerSnapshot>> {
    Json(ApiResponse::ok(state.player.snapshot().await))
}

/// 请求 daemon 开始播放。
///
/// # 参数
/// - `state`：应用状态
///
/// # 返回值
/// - `Result<Json<ApiResponse<PlayerSnapshot>>, ApiError>`：最新播放器快照
#[utoipa::path(post, path = "/api/player/play", responses((status = 200, body = crate::api::response::ApiResponse<crate::core::model::player::PlayerSnapshot>), (status = 409, body = crate::api::response::ApiResponse<serde_json::Value>), (status = 500, body = crate::api::response::ApiResponse<serde_json::Value>)))]
pub async fn play(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<crate::core::model::player::PlayerSnapshot>>, ApiError> {
    state
        .player
        .play()
        .await
        .map(ApiResponse::ok)
        .map(Json)
        .map_err(ApiError::from)
}

/// 请求 daemon 暂停播放。
///
/// # 参数
/// - `state`：应用状态
///
/// # 返回值
/// - `Json<PlayerSnapshot>`：最新播放器快照
#[utoipa::path(post, path = "/api/player/pause", responses((status = 200, body = crate::api::response::ApiResponse<crate::core::model::player::PlayerSnapshot>), (status = 500, body = crate::api::response::ApiResponse<serde_json::Value>)))]
pub async fn pause(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<crate::core::model::player::PlayerSnapshot>>, ApiError> {
    state
        .player
        .pause()
        .await
        .map(ApiResponse::ok)
        .map(Json)
        .map_err(ApiError::from)
}

/// 请求 daemon 切换播放状态。
///
/// # 参数
/// - `state`：应用状态
///
/// # 返回值
/// - `Json<PlayerSnapshot>`：最新播放器快照
#[utoipa::path(post, path = "/api/player/toggle", responses((status = 200, body = crate::api::response::ApiResponse<crate::core::model::player::PlayerSnapshot>), (status = 409, body = crate::api::response::ApiResponse<serde_json::Value>), (status = 500, body = crate::api::response::ApiResponse<serde_json::Value>)))]
pub async fn toggle(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<crate::core::model::player::PlayerSnapshot>>, ApiError> {
    state
        .player
        .toggle()
        .await
        .map(ApiResponse::ok)
        .map(Json)
        .map_err(ApiError::from)
}

/// 请求 daemon 停止播放。
///
/// # 参数
/// - `state`：应用状态
///
/// # 返回值
/// - `Json<PlayerSnapshot>`：最新播放器快照
#[utoipa::path(post, path = "/api/player/stop", responses((status = 200, body = crate::api::response::ApiResponse<crate::core::model::player::PlayerSnapshot>), (status = 500, body = crate::api::response::ApiResponse<serde_json::Value>)))]
pub async fn stop(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<crate::core::model::player::PlayerSnapshot>>, ApiError> {
    state
        .player
        .stop()
        .await
        .map(ApiResponse::ok)
        .map(Json)
        .map_err(ApiError::from)
}

/// 请求 daemon 切到下一首。
///
/// # 参数
/// - `state`：应用状态
///
/// # 返回值
/// - `Json<PlayerSnapshot>`：最新播放器快照
#[utoipa::path(post, path = "/api/player/next", responses((status = 200, body = crate::api::response::ApiResponse<crate::core::model::player::PlayerSnapshot>), (status = 409, body = crate::api::response::ApiResponse<serde_json::Value>), (status = 500, body = crate::api::response::ApiResponse<serde_json::Value>)))]
pub async fn next(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<crate::core::model::player::PlayerSnapshot>>, ApiError> {
    state
        .player
        .next()
        .await
        .map(ApiResponse::ok)
        .map(Json)
        .map_err(ApiError::from)
}

/// 请求 daemon 切到上一首。
///
/// # 参数
/// - `state`：应用状态
///
/// # 返回值
/// - `Json<PlayerSnapshot>`：最新播放器快照
#[utoipa::path(post, path = "/api/player/prev", responses((status = 200, body = crate::api::response::ApiResponse<crate::core::model::player::PlayerSnapshot>), (status = 409, body = crate::api::response::ApiResponse<serde_json::Value>), (status = 500, body = crate::api::response::ApiResponse<serde_json::Value>)))]
pub async fn prev(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<crate::core::model::player::PlayerSnapshot>>, ApiError> {
    state
        .player
        .prev()
        .await
        .map(ApiResponse::ok)
        .map(Json)
        .map_err(ApiError::from)
}

/// 请求 daemon 调整音量。
///
/// # 参数
/// - `state`：应用状态
/// - `request`：音量调整请求
///
/// # 返回值
/// - `Json<PlayerSnapshot>`：最新播放器快照
#[utoipa::path(
    post,
    path = "/api/player/volume",
    request_body = PlayerVolumeRequest,
    responses(
        (status = 200, description = "更新音量后的快照", body = crate::api::response::ApiResponse<crate::core::model::player::PlayerSnapshot>),
        (status = 400, description = "音量参数无效", body = crate::api::response::ApiResponse<serde_json::Value>),
        (status = 500, description = "更新音量失败", body = crate::api::response::ApiResponse<serde_json::Value>)
    )
)]
pub async fn set_volume(
    State(state): State<AppState>,
    Json(request): Json<PlayerVolumeRequest>,
) -> Result<Json<ApiResponse<crate::core::model::player::PlayerSnapshot>>, ApiError> {
    state
        .player
        .set_volume_percent(request.volume_percent)
        .await
        .map(ApiResponse::ok)
        .map(Json)
        .map_err(ApiError::from)
}

/// 请求 daemon 进入静音状态。
///
/// # 参数
/// - `state`：应用状态
///
/// # 返回值
/// - `Json<PlayerSnapshot>`：最新播放器快照
#[utoipa::path(post, path = "/api/player/mute", responses((status = 200, body = crate::api::response::ApiResponse<crate::core::model::player::PlayerSnapshot>), (status = 500, body = crate::api::response::ApiResponse<serde_json::Value>)))]
pub async fn mute(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<crate::core::model::player::PlayerSnapshot>>, ApiError> {
    state
        .player
        .mute()
        .await
        .map(ApiResponse::ok)
        .map(Json)
        .map_err(ApiError::from)
}

/// 请求 daemon 取消静音。
///
/// # 参数
/// - `state`：应用状态
///
/// # 返回值
/// - `Json<PlayerSnapshot>`：最新播放器快照
#[utoipa::path(post, path = "/api/player/unmute", responses((status = 200, body = crate::api::response::ApiResponse<crate::core::model::player::PlayerSnapshot>), (status = 500, body = crate::api::response::ApiResponse<serde_json::Value>)))]
pub async fn unmute(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<crate::core::model::player::PlayerSnapshot>>, ApiError> {
    state
        .player
        .unmute()
        .await
        .map(ApiResponse::ok)
        .map(Json)
        .map_err(ApiError::from)
}

/// 请求 daemon 更新循环/随机模式。
///
/// # 参数
/// - `state`：应用状态
/// - `request`：模式调整请求
///
/// # 返回值
/// - `Json<PlayerSnapshot>`：最新播放器快照
#[utoipa::path(
    post,
    path = "/api/player/mode",
    request_body = PlayerModeRequest,
    responses(
        (status = 200, description = "更新模式后的快照", body = crate::api::response::ApiResponse<crate::core::model::player::PlayerSnapshot>),
        (status = 400, description = "模式参数无效", body = crate::api::response::ApiResponse<serde_json::Value>),
        (status = 500, description = "更新模式失败", body = crate::api::response::ApiResponse<serde_json::Value>)
    )
)]
pub async fn set_mode(
    State(state): State<AppState>,
    Json(request): Json<PlayerModeRequest>,
) -> Result<Json<ApiResponse<crate::core::model::player::PlayerSnapshot>>, ApiError> {
    let mut snapshot = state.player.snapshot().await;
    if let Some(repeat_mode) = request.repeat_mode {
        snapshot = state
            .player
            .set_repeat_mode(parse_repeat_mode(&repeat_mode))
            .await
            .map_err(ApiError::from)?;
    }
    if let Some(shuffle_enabled) = request.shuffle_enabled {
        snapshot = state
            .player
            .set_shuffle_enabled(shuffle_enabled)
            .await
            .map_err(ApiError::from)?;
    }
    Ok(Json(ApiResponse::ok(snapshot)))
}

/// 解析对外传入的循环模式字符串。
///
/// # 参数
/// - `value`：对外传入的模式字符串
///
/// # 返回值
/// - `RepeatMode`：解析后的循环模式
fn parse_repeat_mode(value: &str) -> RepeatMode {
    match value {
        "one" => RepeatMode::One,
        "all" => RepeatMode::All,
        _ => RepeatMode::Off,
    }
}

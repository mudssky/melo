use axum::{Json, extract::State};

use crate::daemon::app::AppState;

/// 返回当前播放器状态快照。
///
/// # 参数
/// - `state`：应用状态
///
/// # 返回
/// - `Json<PlayerSnapshot>`：播放器快照
pub async fn status(
    State(state): State<AppState>,
) -> Json<crate::core::model::player::PlayerSnapshot> {
    Json(state.player.snapshot().await)
}

/// 请求 daemon 开始播放。
///
/// # 参数
/// - `state`：应用状态
///
/// # 返回值
/// - `Json<PlayerSnapshot>`：最新播放器快照
pub async fn play(
    State(state): State<AppState>,
) -> Json<crate::core::model::player::PlayerSnapshot> {
    Json(state.player.play().await.unwrap())
}

/// 请求 daemon 暂停播放。
///
/// # 参数
/// - `state`：应用状态
///
/// # 返回值
/// - `Json<PlayerSnapshot>`：最新播放器快照
pub async fn pause(
    State(state): State<AppState>,
) -> Json<crate::core::model::player::PlayerSnapshot> {
    Json(state.player.pause().await.unwrap())
}

/// 请求 daemon 切换播放状态。
///
/// # 参数
/// - `state`：应用状态
///
/// # 返回值
/// - `Json<PlayerSnapshot>`：最新播放器快照
pub async fn toggle(
    State(state): State<AppState>,
) -> Json<crate::core::model::player::PlayerSnapshot> {
    Json(state.player.toggle().await.unwrap())
}

/// 请求 daemon 停止播放。
///
/// # 参数
/// - `state`：应用状态
///
/// # 返回值
/// - `Json<PlayerSnapshot>`：最新播放器快照
pub async fn stop(
    State(state): State<AppState>,
) -> Json<crate::core::model::player::PlayerSnapshot> {
    Json(state.player.stop().await.unwrap())
}

/// 请求 daemon 切到下一首。
///
/// # 参数
/// - `state`：应用状态
///
/// # 返回值
/// - `Json<PlayerSnapshot>`：最新播放器快照
pub async fn next(
    State(state): State<AppState>,
) -> Json<crate::core::model::player::PlayerSnapshot> {
    Json(state.player.next().await.unwrap())
}

/// 请求 daemon 切到上一首。
///
/// # 参数
/// - `state`：应用状态
///
/// # 返回值
/// - `Json<PlayerSnapshot>`：最新播放器快照
pub async fn prev(
    State(state): State<AppState>,
) -> Json<crate::core::model::player::PlayerSnapshot> {
    Json(state.player.prev().await.unwrap())
}

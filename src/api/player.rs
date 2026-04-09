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

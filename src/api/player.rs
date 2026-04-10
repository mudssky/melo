use axum::{Json, extract::State};

use crate::core::model::player::RepeatMode;
use crate::daemon::app::AppState;

/// 调整播放器音量的请求体。
#[derive(Debug, serde::Deserialize)]
pub struct PlayerVolumeRequest {
    /// 目标音量百分比。
    pub volume_percent: u8,
}

/// 调整播放器模式的请求体。
#[derive(Debug, serde::Deserialize)]
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

/// 请求 daemon 调整音量。
///
/// # 参数
/// - `state`：应用状态
/// - `request`：音量调整请求
///
/// # 返回值
/// - `Json<PlayerSnapshot>`：最新播放器快照
pub async fn set_volume(
    State(state): State<AppState>,
    Json(request): Json<PlayerVolumeRequest>,
) -> Json<crate::core::model::player::PlayerSnapshot> {
    Json(
        state
            .player
            .set_volume_percent(request.volume_percent)
            .await
            .unwrap(),
    )
}

/// 请求 daemon 进入静音状态。
///
/// # 参数
/// - `state`：应用状态
///
/// # 返回值
/// - `Json<PlayerSnapshot>`：最新播放器快照
pub async fn mute(
    State(state): State<AppState>,
) -> Json<crate::core::model::player::PlayerSnapshot> {
    Json(state.player.mute().await.unwrap())
}

/// 请求 daemon 取消静音。
///
/// # 参数
/// - `state`：应用状态
///
/// # 返回值
/// - `Json<PlayerSnapshot>`：最新播放器快照
pub async fn unmute(
    State(state): State<AppState>,
) -> Json<crate::core::model::player::PlayerSnapshot> {
    Json(state.player.unmute().await.unwrap())
}

/// 请求 daemon 更新循环/随机模式。
///
/// # 参数
/// - `state`：应用状态
/// - `request`：模式调整请求
///
/// # 返回值
/// - `Json<PlayerSnapshot>`：最新播放器快照
pub async fn set_mode(
    State(state): State<AppState>,
    Json(request): Json<PlayerModeRequest>,
) -> Json<crate::core::model::player::PlayerSnapshot> {
    let mut snapshot = state.player.snapshot().await;
    if let Some(repeat_mode) = request.repeat_mode {
        snapshot = state
            .player
            .set_repeat_mode(parse_repeat_mode(&repeat_mode))
            .await
            .unwrap();
    }
    if let Some(shuffle_enabled) = request.shuffle_enabled {
        snapshot = state
            .player
            .set_shuffle_enabled(shuffle_enabled)
            .await
            .unwrap();
    }
    Json(snapshot)
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

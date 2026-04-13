use axum::{Json, extract::State};

use crate::api::error::ApiError;
use crate::api::response::ApiResponse;
use crate::core::model::playback_mode::PlaybackMode;
use crate::core::model::playback_runtime::{ClientBootstrapSnapshot, PlaybackRuntimeSnapshot};
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
) -> Result<Json<ApiResponse<ClientBootstrapSnapshot>>, ApiError> {
    let player = state.player.snapshot().await;
    let current_source_ref = state.current_playlist_context().map(|context| context.name);
    let current_song = player.current_song.clone();
    let runtime = PlaybackRuntimeSnapshot {
        generation: player.version,
        playback_state: player.playback_state,
        current_source_ref: current_source_ref.clone(),
        current_song_id: current_song.as_ref().map(|song| song.song_id),
        current_index: player.queue_index,
        position_seconds: player.position_seconds,
        duration_seconds: current_song.and_then(|song| song.duration_seconds),
        playback_mode: infer_playback_mode(
            &player.repeat_mode,
            player.shuffle_enabled,
            state.settings.player.default_mode,
        ),
        volume_percent: player.volume_percent,
        muted: player.muted,
        last_error_code: player.last_error.map(|error| error.code),
    };

    Ok(Json(ApiResponse::ok(ClientBootstrapSnapshot {
        runtime,
        default_playback_mode: state.settings.player.default_mode,
        current_source_ref,
    })))
}

/// 根据旧播放器字段推断当前用户可见播放模式。
///
/// # 参数
/// - `repeat_mode`：旧快照中的循环模式字符串
/// - `shuffle_enabled`：旧快照中的随机播放开关
/// - `default_mode`：配置中的默认播放模式
///
/// # 返回值
/// - `PlaybackMode`：推断后的播放模式
fn infer_playback_mode(
    repeat_mode: &str,
    shuffle_enabled: bool,
    default_mode: PlaybackMode,
) -> PlaybackMode {
    if shuffle_enabled {
        PlaybackMode::Shuffle
    } else {
        match repeat_mode {
            "one" => PlaybackMode::RepeatOne,
            "off" => default_mode,
            _ => PlaybackMode::Ordered,
        }
    }
}

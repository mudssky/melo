use utoipa::OpenApi;

/// Melo daemon HTTP API 的 OpenAPI 聚合定义。
#[derive(OpenApi)]
#[openapi(
    paths(
        crate::api::system::health,
        crate::api::system::status,
        crate::api::system::shutdown,
        crate::api::player::status,
        crate::api::player::play,
        crate::api::player::pause,
        crate::api::player::toggle,
        crate::api::player::stop,
        crate::api::player::next,
        crate::api::player::prev,
        crate::api::player::set_volume,
        crate::api::player::mute,
        crate::api::player::unmute,
        crate::api::player::set_mode,
        crate::api::queue::add,
        crate::api::queue::insert,
        crate::api::queue::clear,
        crate::api::queue::play_index,
        crate::api::queue::remove,
        crate::api::queue::move_item,
        crate::api::open::open,
        crate::api::playlist::preview,
        crate::api::playlist::play,
        crate::api::playlist::play_command,
        crate::api::tui::home,
        crate::api::ws::player_updates,
        crate::api::ws::tui_updates
    ),
    components(
        schemas(
            crate::api::response::ApiResponse<crate::api::system::HealthResponse>,
            crate::api::response::ApiResponse<crate::api::system::DaemonStatusResponse>,
            crate::api::response::ApiResponse<crate::core::model::player::PlayerSnapshot>,
            crate::api::response::ApiResponse<crate::domain::open::service::OpenResponse>,
            crate::api::response::ApiResponse<crate::api::playlist::PlaylistPreviewResponse>,
            crate::api::response::ApiResponse<crate::core::model::tui::TuiSnapshot>,
            crate::api::response::ApiResponse<serde_json::Value>,
            crate::api::system::HealthResponse,
            crate::api::system::DaemonStatusResponse,
            crate::api::player::PlayerVolumeRequest,
            crate::api::player::PlayerModeRequest,
            crate::api::playlist::PlaylistPreviewQuery,
            crate::api::playlist::PlaylistPlayRequest,
            crate::api::playlist::PlaylistPlayCommandResponse,
            crate::api::playlist::PlaylistPreviewSong,
            crate::api::playlist::PlaylistPreviewResponse,
            crate::api::response::ApiResponse<crate::api::playlist::PlaylistPlayCommandResponse>,
            crate::api::queue::QueueAddRequest,
            crate::api::queue::QueueIndexRequest,
            crate::api::queue::QueueInsertRequest,
            crate::api::queue::QueueRemoveRequest,
            crate::api::queue::QueueMoveRequest,
            crate::domain::open::service::OpenRequest,
            crate::domain::open::service::OpenResponse,
            crate::core::model::tui::TuiSnapshot,
            crate::core::model::tui::PlaylistBrowserSnapshot,
            crate::core::model::tui::PlaylistListItem,
            crate::core::model::tui::TuiViewKind,
            crate::core::model::player::PlayerSnapshot,
            crate::core::model::player::PlayerErrorInfo,
            crate::core::model::player::QueueItem,
            crate::core::model::player::NowPlayingSong
        )
    ),
    info(
        title = "Melo API",
        version = env!("CARGO_PKG_VERSION"),
        description = "Melo daemon 的命令型 HTTP API"
    )
)]
pub struct MeloOpenApi;

/// 导出格式化后的 OpenAPI JSON 文本。
///
/// # 参数
/// - 无
///
/// # 返回值
/// - `String`：OpenAPI JSON
pub fn openapi_json() -> String {
    MeloOpenApi::openapi().to_pretty_json().unwrap()
}

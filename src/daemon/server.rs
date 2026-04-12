use axum::Router;
use axum::extract::{ConnectInfo, Request, State};
use axum::http::StatusCode;
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use tower_http::trace::TraceLayer;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use crate::daemon::app::AppState;

/// 构建 daemon 路由。
///
/// # 参数
/// - `state`：应用状态
///
/// # 返回值
/// - `Router`：Axum 路由
pub fn router(state: AppState) -> Router {
    let mut router = Router::new()
        .route(
            "/api/system/health",
            axum::routing::get(crate::api::system::health),
        )
        .route(
            "/api/system/status",
            axum::routing::get(crate::api::system::status),
        )
        .route(
            "/api/system/shutdown",
            axum::routing::post(crate::api::system::shutdown),
        )
        .route(
            "/api/player/status",
            axum::routing::get(crate::api::player::status),
        )
        .route(
            "/api/player/play",
            axum::routing::post(crate::api::player::play),
        )
        .route(
            "/api/player/pause",
            axum::routing::post(crate::api::player::pause),
        )
        .route(
            "/api/player/toggle",
            axum::routing::post(crate::api::player::toggle),
        )
        .route(
            "/api/player/stop",
            axum::routing::post(crate::api::player::stop),
        )
        .route(
            "/api/player/next",
            axum::routing::post(crate::api::player::next),
        )
        .route(
            "/api/player/prev",
            axum::routing::post(crate::api::player::prev),
        )
        .route(
            "/api/player/volume",
            axum::routing::post(crate::api::player::set_volume),
        )
        .route(
            "/api/player/mute",
            axum::routing::post(crate::api::player::mute),
        )
        .route(
            "/api/player/unmute",
            axum::routing::post(crate::api::player::unmute),
        )
        .route(
            "/api/player/mode",
            axum::routing::post(crate::api::player::set_mode),
        )
        .route(
            "/api/queue/add",
            axum::routing::post(crate::api::queue::add),
        )
        .route(
            "/api/queue/insert",
            axum::routing::post(crate::api::queue::insert),
        )
        .route(
            "/api/queue/clear",
            axum::routing::post(crate::api::queue::clear),
        )
        .route(
            "/api/queue/play",
            axum::routing::post(crate::api::queue::play_index),
        )
        .route(
            "/api/queue/remove",
            axum::routing::post(crate::api::queue::remove),
        )
        .route(
            "/api/queue/move",
            axum::routing::post(crate::api::queue::move_item),
        )
        .route("/api/open", axum::routing::post(crate::api::open::open))
        .route(
            "/api/ws/player",
            axum::routing::get(crate::api::ws::player_updates),
        )
        .route(
            "/api/ws/tui",
            axum::routing::get(crate::api::ws::tui_updates),
        )
        .layer(TraceLayer::new_for_http())
        .with_state(state.clone());

    if state.docs_mode() != crate::core::config::settings::DaemonDocsMode::Disabled {
        let docs_router = Router::new()
            .merge(SwaggerUi::new("/api/docs").url(
                "/api/openapi.json",
                crate::api::docs::MeloOpenApi::openapi(),
            ))
            .layer(middleware::from_fn_with_state(state.clone(), docs_guard));
        router = router.merge(docs_router);
    }

    router
}

/// 为 docs 与 openapi 路由执行访问控制。
///
/// # 参数
/// - `state`：应用状态
/// - `addr`：连接来源地址
/// - `request`：原始请求
/// - `next`：后续中间件
///
/// # 返回值
/// - `Response`：中间件处理后的响应
async fn docs_guard(State(state): State<AppState>, request: Request, next: Next) -> Response {
    match state.docs_mode() {
        crate::core::config::settings::DaemonDocsMode::Disabled => {
            StatusCode::NOT_FOUND.into_response()
        }
        crate::core::config::settings::DaemonDocsMode::Local => {
            let Some(ConnectInfo(addr)) = request
                .extensions()
                .get::<ConnectInfo<std::net::SocketAddr>>()
                .cloned()
            else {
                return StatusCode::FORBIDDEN.into_response();
            };
            if !addr.ip().is_loopback() {
                return StatusCode::FORBIDDEN.into_response();
            }
            next.run(request).await
        }
        crate::core::config::settings::DaemonDocsMode::Network => next.run(request).await,
    }
}

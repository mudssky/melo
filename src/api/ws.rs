use axum::extract::State;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::response::Response;
use serde::Serialize;

use crate::daemon::app::AppState;

/// 升级为播放器快照 WebSocket 连接。
///
/// # 参数
/// - `socket`：WebSocket 升级请求
/// - `state`：应用状态
///
/// # 返回值
/// - `Response`：升级响应
#[utoipa::path(
    get,
    path = "/api/ws/player",
    responses(
        (status = 101, description = "升级为播放器快照 WebSocket 流")
    )
)]
pub async fn player_updates(socket: WebSocketUpgrade, State(state): State<AppState>) -> Response {
    socket.on_upgrade(move |websocket| stream_player_snapshots(websocket, state))
}

/// 升级为 TUI 聚合快照 WebSocket 连接。
///
/// # 参数
/// - `socket`：WebSocket 升级请求
/// - `state`：应用状态
///
/// # 返回值
/// - `Response`：升级响应
#[utoipa::path(
    get,
    path = "/api/ws/tui",
    responses(
        (status = 101, description = "升级为 TUI 聚合状态 WebSocket 流")
    )
)]
pub async fn tui_updates(socket: WebSocketUpgrade, State(state): State<AppState>) -> Response {
    socket.on_upgrade(move |websocket| stream_tui_snapshots(websocket, state))
}

/// 持续向客户端发送播放器快照。
///
/// # 参数
/// - `socket`：WebSocket 连接
/// - `state`：应用状态
///
/// # 返回值
/// - 无
async fn stream_player_snapshots(mut socket: WebSocket, state: AppState) {
    let mut receiver = state.player.subscribe();
    let initial_snapshot = receiver.borrow().clone();
    if send_payload(&mut socket, &initial_snapshot).await.is_err() {
        return;
    }

    while receiver.changed().await.is_ok() {
        let snapshot = receiver.borrow().clone();
        if send_payload(&mut socket, &snapshot).await.is_err() {
            break;
        }
    }
}

/// 持续向客户端发送 TUI 聚合快照。
///
/// # 参数
/// - `socket`：WebSocket 连接
/// - `state`：应用状态
///
/// # 返回值
/// - 无
async fn stream_tui_snapshots(mut socket: WebSocket, state: AppState) {
    let mut player_rx = state.player.subscribe();
    let mut task_rx = state.runtime_tasks().subscribe();

    if send_payload(&mut socket, &state.tui_snapshot().await)
        .await
        .is_err()
    {
        return;
    }

    loop {
        tokio::select! {
            changed = player_rx.changed() => {
                if changed.is_err() {
                    break;
                }
            }
            changed = task_rx.changed() => {
                if changed.is_err() {
                    break;
                }
            }
        }

        let snapshot = state.tui_snapshot().await;
        if send_payload(&mut socket, &snapshot).await.is_err() {
            break;
        }
    }
}

/// 序列化并发送单条 JSON 负载。
///
/// # 参数
/// - `socket`：WebSocket 连接
/// - `payload`：待发送负载
///
/// # 返回值
/// - `Result<(), axum::Error>`：发送结果
async fn send_payload<T>(socket: &mut WebSocket, payload: &T) -> Result<(), axum::Error>
where
    T: Serialize,
{
    let payload = serde_json::to_string(payload).unwrap_or_else(|_| "{}".to_string());
    socket.send(Message::Text(payload.into())).await
}

use axum::extract::State;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::response::Response;

use crate::core::model::player::PlayerSnapshot;
use crate::daemon::app::AppState;

/// 升级为播放器快照 WebSocket 连接。
///
/// # 参数
/// - `socket`：WebSocket 升级请求
/// - `state`：应用状态
///
/// # 返回值
/// - `Response`：升级响应
pub async fn player_updates(socket: WebSocketUpgrade, State(state): State<AppState>) -> Response {
    socket.on_upgrade(move |websocket| stream_player_snapshots(websocket, state))
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
    if send_snapshot(&mut socket, initial_snapshot).await.is_err() {
        return;
    }

    while receiver.changed().await.is_ok() {
        let snapshot = receiver.borrow().clone();
        if send_snapshot(&mut socket, snapshot).await.is_err() {
            break;
        }
    }
}

/// 序列化并发送单条播放器快照。
///
/// # 参数
/// - `socket`：WebSocket 连接
/// - `snapshot`：播放器快照
///
/// # 返回值
/// - `Result<(), axum::Error>`：发送结果
async fn send_snapshot(
    socket: &mut WebSocket,
    snapshot: PlayerSnapshot,
) -> Result<(), axum::Error> {
    let payload = serde_json::to_string(&snapshot).unwrap_or_else(|_| "{}".to_string());
    socket.send(Message::Text(payload.into())).await
}

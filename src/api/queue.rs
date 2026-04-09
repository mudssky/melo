use axum::{Json, extract::State};

use crate::core::model::player::{PlayerSnapshot, QueueItem};
use crate::daemon::app::AppState;

/// 队列批量追加请求。
#[derive(Debug, serde::Deserialize)]
pub struct QueueAddRequest {
    /// 待追加的队列项列表。
    pub items: Vec<QueueItem>,
}

/// 基于单索引的队列请求。
#[derive(Debug, serde::Deserialize)]
pub struct QueueIndexRequest {
    /// 目标索引。
    pub index: usize,
}

/// 队列插入请求。
#[derive(Debug, serde::Deserialize)]
pub struct QueueInsertRequest {
    /// 插入位置。
    pub index: usize,
    /// 待插入队列项。
    pub item: QueueItem,
}

/// 队列删除请求。
#[derive(Debug, serde::Deserialize)]
pub struct QueueRemoveRequest {
    /// 待删除索引。
    pub index: usize,
}

/// 队列移动请求。
#[derive(Debug, serde::Deserialize)]
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
pub async fn add(
    State(state): State<AppState>,
    Json(request): Json<QueueAddRequest>,
) -> Json<PlayerSnapshot> {
    let mut snapshot = state.player.snapshot().await;
    for item in request.items {
        snapshot = state.player.append(item).await.unwrap();
    }

    Json(snapshot)
}

/// 在指定位置插入歌曲。
///
/// # 参数
/// - `state`：应用状态
/// - `request`：插入请求
///
/// # 返回值
/// - `Json<PlayerSnapshot>`：最新播放器快照
pub async fn insert(
    State(state): State<AppState>,
    Json(request): Json<QueueInsertRequest>,
) -> Json<PlayerSnapshot> {
    Json(
        state
            .player
            .insert(request.index, request.item)
            .await
            .unwrap(),
    )
}

/// 清空整个播放队列。
///
/// # 参数
/// - `state`：应用状态
///
/// # 返回值
/// - `Json<PlayerSnapshot>`：最新播放器快照
pub async fn clear(State(state): State<AppState>) -> Json<PlayerSnapshot> {
    Json(state.player.clear().await.unwrap())
}

/// 选择指定队列索引并播放。
///
/// # 参数
/// - `state`：应用状态
/// - `request`：播放请求
///
/// # 返回值
/// - `Json<PlayerSnapshot>`：最新播放器快照
pub async fn play_index(
    State(state): State<AppState>,
    Json(request): Json<QueueIndexRequest>,
) -> Json<PlayerSnapshot> {
    Json(state.player.play_index(request.index).await.unwrap())
}

/// 删除指定队列项。
///
/// # 参数
/// - `state`：应用状态
/// - `request`：删除请求
///
/// # 返回值
/// - `Json<PlayerSnapshot>`：最新播放器快照
pub async fn remove(
    State(state): State<AppState>,
    Json(request): Json<QueueRemoveRequest>,
) -> Json<PlayerSnapshot> {
    Json(state.player.remove(request.index).await.unwrap())
}

/// 移动指定队列项。
///
/// # 参数
/// - `state`：应用状态
/// - `request`：移动请求
///
/// # 返回值
/// - `Json<PlayerSnapshot>`：最新播放器快照
pub async fn move_item(
    State(state): State<AppState>,
    Json(request): Json<QueueMoveRequest>,
) -> Json<PlayerSnapshot> {
    Json(
        state
            .player
            .move_item(request.from, request.to)
            .await
            .unwrap(),
    )
}

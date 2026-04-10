use std::time::{SystemTime, UNIX_EPOCH};

use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
};

use crate::core::error::{MeloError, MeloResult};
use crate::core::model::player::{PlaybackState, QueueItem};

/// 持久化的播放器会话快照。
#[derive(Debug, Clone, PartialEq)]
pub struct PersistedPlayerSession {
    /// 当前播放状态。
    pub playback_state: PlaybackState,
    /// 当前队列索引。
    pub queue_index: Option<usize>,
    /// 最近一次已知播放位置。
    pub position_seconds: Option<f64>,
    /// 当前完整队列。
    pub queue: Vec<QueueItem>,
}

/// 播放会话持久化仓储。
#[derive(Clone)]
pub struct PlayerSessionStore {
    db: DatabaseConnection,
}

impl PlayerSessionStore {
    /// 创建新的播放器会话仓储。
    ///
    /// # 参数
    /// - `db`：数据库连接
    ///
    /// # 返回值
    /// - `Self`：播放器会话仓储
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    /// 判断当前会话是否值得落盘。
    ///
    /// # 参数
    /// - `previous`：上一次成功落盘的会话
    /// - `current`：当前待持久化会话
    ///
    /// # 返回值
    /// - `bool`：是否应该持久化
    pub fn should_persist(
        &self,
        previous: Option<&PersistedPlayerSession>,
        current: &PersistedPlayerSession,
    ) -> bool {
        let Some(previous) = previous else {
            return true;
        };

        previous.playback_state != current.playback_state
            || previous.queue_index != current.queue_index
            || previous.queue != current.queue
            || match (previous.position_seconds, current.position_seconds) {
                (Some(a), Some(b)) => (a - b).abs() >= 1.0,
                (None, None) => false,
                _ => true,
            }
    }

    /// 保存当前播放器会话。
    ///
    /// # 参数
    /// - `session`：待保存的播放器会话
    ///
    /// # 返回值
    /// - `MeloResult<()>`：持久化结果
    pub async fn save(&self, session: &PersistedPlayerSession) -> MeloResult<()> {
        use crate::core::db::entities::{player_session_items, player_sessions};

        player_session_items::Entity::delete_many()
            .exec(&self.db)
            .await
            .map_err(|err| MeloError::Message(err.to_string()))?;
        player_sessions::Entity::delete_many()
            .exec(&self.db)
            .await
            .map_err(|err| MeloError::Message(err.to_string()))?;

        let header = player_sessions::ActiveModel {
            playback_state: Set(session.playback_state.as_str().to_string()),
            queue_index: Set(session.queue_index.map(|value| value as i64)),
            position_seconds: Set(session.position_seconds),
            updated_at: Set(current_timestamp_string()),
            ..Default::default()
        }
        .insert(&self.db)
        .await
        .map_err(|err| MeloError::Message(err.to_string()))?;

        for (position, item) in session.queue.iter().enumerate() {
            player_session_items::ActiveModel {
                session_id: Set(header.id),
                position: Set(position as i64),
                song_id: Set(item.song_id),
                path: Set(item.path.clone()),
                title: Set(item.title.clone()),
                duration_seconds: Set(item.duration_seconds),
                ..Default::default()
            }
            .insert(&self.db)
            .await
            .map_err(|err| MeloError::Message(err.to_string()))?;
        }

        Ok(())
    }

    /// 读取最近一次持久化的播放器会话。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - `MeloResult<Option<PersistedPlayerSession>>`：读取结果
    pub async fn load(&self) -> MeloResult<Option<PersistedPlayerSession>> {
        use crate::core::db::entities::{player_session_items, player_sessions};

        let Some(header) = player_sessions::Entity::find()
            .one(&self.db)
            .await
            .map_err(|err| MeloError::Message(err.to_string()))?
        else {
            return Ok(None);
        };

        let items = player_session_items::Entity::find()
            .filter(player_session_items::Column::SessionId.eq(header.id))
            .order_by_asc(player_session_items::Column::Position)
            .all(&self.db)
            .await
            .map_err(|err| MeloError::Message(err.to_string()))?;

        Ok(Some(PersistedPlayerSession {
            playback_state: parse_playback_state(&header.playback_state),
            queue_index: header.queue_index.map(|value| value as usize),
            position_seconds: header.position_seconds,
            queue: items
                .into_iter()
                .map(|item| QueueItem {
                    song_id: item.song_id,
                    path: item.path,
                    title: item.title,
                    duration_seconds: item.duration_seconds,
                })
                .collect(),
        }))
    }
}

/// 将持久化字符串状态还原为领域层播放状态。
///
/// # 参数
/// - `value`：数据库中的状态字符串
///
/// # 返回值
/// - `PlaybackState`：解析后的播放状态
fn parse_playback_state(value: &str) -> PlaybackState {
    match value {
        "idle" => PlaybackState::Idle,
        "playing" => PlaybackState::Playing,
        "paused" => PlaybackState::Paused,
        "stopped" => PlaybackState::Stopped,
        "error" => PlaybackState::Error,
        _ => PlaybackState::Idle,
    }
}

/// 生成用于落盘的时间戳字符串。
///
/// # 参数
/// - 无
///
/// # 返回值
/// - `String`：当前时间戳字符串
fn current_timestamp_string() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs().to_string())
        .unwrap_or_else(|_| "0".to_string())
}

#[cfg(test)]
mod tests;

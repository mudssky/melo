use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::core::model::player::PlayerSnapshot;
use crate::core::model::runtime_task::RuntimeTaskSnapshot;

/// 提供给 TUI 的聚合快照。
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, ToSchema)]
pub struct TuiSnapshot {
    /// 当前播放器快照。
    pub player: PlayerSnapshot,
    /// 当前活动运行时任务。
    pub active_task: Option<RuntimeTaskSnapshot>,
}

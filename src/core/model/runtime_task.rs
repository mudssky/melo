use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// 运行时任务种类。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeTaskKind {
    /// 媒体库扫描任务。
    LibraryScan,
}

/// 运行时任务阶段。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeTaskPhase {
    /// 正在发现待处理项目。
    Discovering,
    /// 正在同步预热首批项目。
    Prewarming,
    /// 正在持续索引并补齐后续项目。
    Indexing,
    /// 任务已完成。
    Completed,
    /// 任务执行失败。
    Failed,
}

/// 提供给 CLI / TUI / WebSocket 的运行时任务快照。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, ToSchema)]
pub struct RuntimeTaskSnapshot {
    /// 任务 ID。
    pub task_id: String,
    /// 任务种类。
    pub kind: RuntimeTaskKind,
    /// 当前阶段。
    pub phase: RuntimeTaskPhase,
    /// 当前任务来源标签。
    pub source_label: String,
    /// 已发现项目数。
    pub discovered_count: usize,
    /// 已索引项目数。
    pub indexed_count: usize,
    /// 已入队项目数。
    pub queued_count: usize,
    /// 当前处理项目名。
    pub current_item_name: Option<String>,
    /// 最近一次错误信息。
    pub last_error: Option<String>,
}

use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::sync::watch;
use uuid::Uuid;

use crate::core::model::runtime_task::{RuntimeTaskKind, RuntimeTaskPhase, RuntimeTaskSnapshot};

/// 运行时任务存储，负责向外广播当前活动任务快照。
#[derive(Clone)]
pub struct RuntimeTaskStore {
    snapshot_tx: watch::Sender<Option<RuntimeTaskSnapshot>>,
    success_ttl: Duration,
    failure_ttl: Duration,
}

/// 单个运行时任务的可变句柄。
#[derive(Clone)]
pub struct RuntimeTaskHandle {
    store: RuntimeTaskStore,
    snapshot: Arc<Mutex<RuntimeTaskSnapshot>>,
}

impl RuntimeTaskStore {
    /// 创建新的运行时任务存储。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - `Self`：初始化完成的运行时任务存储
    pub fn new() -> Self {
        let (snapshot_tx, _snapshot_rx) = watch::channel(None);
        Self {
            snapshot_tx,
            success_ttl: Duration::from_secs(3),
            failure_ttl: Duration::from_secs(5),
        }
    }

    /// 订阅当前运行时任务快照。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - `watch::Receiver<Option<RuntimeTaskSnapshot>>`：快照订阅器
    pub fn subscribe(&self) -> watch::Receiver<Option<RuntimeTaskSnapshot>> {
        self.snapshot_tx.subscribe()
    }

    /// 读取当前活动任务快照。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - `Option<RuntimeTaskSnapshot>`：当前活动任务；无任务时为 `None`
    pub fn current(&self) -> Option<RuntimeTaskSnapshot> {
        self.snapshot_tx.borrow().clone()
    }

    /// 启动一个新的扫描任务，并立即广播初始快照。
    ///
    /// # 参数
    /// - `source_label`：来源标签
    /// - `discovered_count`：已发现项目数
    ///
    /// # 返回值
    /// - `RuntimeTaskHandle`：后续可更新此任务状态的句柄
    pub fn start_scan(&self, source_label: String, discovered_count: usize) -> RuntimeTaskHandle {
        let snapshot = RuntimeTaskSnapshot {
            task_id: Uuid::new_v4().to_string(),
            kind: RuntimeTaskKind::LibraryScan,
            phase: RuntimeTaskPhase::Discovering,
            source_label,
            discovered_count,
            indexed_count: 0,
            queued_count: 0,
            current_item_name: None,
            last_error: None,
        };
        self.snapshot_tx.send_replace(Some(snapshot.clone()));
        RuntimeTaskHandle {
            store: self.clone(),
            snapshot: Arc::new(Mutex::new(snapshot)),
        }
    }

    /// 仅当当前活动任务仍是指定任务时才清除快照，避免旧任务延时清理误删新任务状态。
    ///
    /// # 参数
    /// - `task_id`：需要清理的任务 ID
    ///
    /// # 返回值
    /// - 无
    fn clear_if_current(&self, task_id: &str) {
        let should_clear = self
            .snapshot_tx
            .borrow()
            .as_ref()
            .is_some_and(|snapshot| snapshot.task_id == task_id);
        if should_clear {
            self.snapshot_tx.send_replace(None);
        }
    }
}

impl Default for RuntimeTaskStore {
    /// 返回默认的运行时任务存储。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - `Self`：默认任务存储
    fn default() -> Self {
        Self::new()
    }
}

impl RuntimeTaskHandle {
    /// 标记任务进入预热阶段。
    ///
    /// # 参数
    /// - `current_item_name`：当前正在处理的项目名
    ///
    /// # 返回值
    /// - 无
    pub fn mark_prewarming(&self, current_item_name: Option<String>) {
        self.update(|snapshot| {
            snapshot.phase = RuntimeTaskPhase::Prewarming;
            snapshot.current_item_name = current_item_name;
            snapshot.last_error = None;
        });
    }

    /// 标记任务进入索引阶段并刷新进度。
    ///
    /// # 参数
    /// - `indexed_count`：已索引数量
    /// - `queued_count`：已入队数量
    /// - `current_item_name`：当前项目名
    ///
    /// # 返回值
    /// - 无
    pub fn mark_indexing(
        &self,
        indexed_count: usize,
        queued_count: usize,
        current_item_name: Option<String>,
    ) {
        self.update(|snapshot| {
            snapshot.phase = RuntimeTaskPhase::Indexing;
            snapshot.indexed_count = indexed_count;
            snapshot.queued_count = queued_count;
            snapshot.current_item_name = current_item_name;
            snapshot.last_error = None;
        });
    }

    /// 标记任务成功完成，并在短暂保留后自动清理。
    ///
    /// # 参数
    /// - `queued_count`：最终入队数量
    ///
    /// # 返回值
    /// - 无
    pub fn mark_completed(&self, queued_count: usize) {
        self.update(|snapshot| {
            snapshot.phase = RuntimeTaskPhase::Completed;
            snapshot.indexed_count = queued_count;
            snapshot.queued_count = queued_count;
            snapshot.current_item_name = None;
            snapshot.last_error = None;
        });
        self.schedule_clear(self.store.success_ttl);
    }

    /// 标记任务失败，并在短暂保留后自动清理。
    ///
    /// # 参数
    /// - `error_message`：错误文案
    ///
    /// # 返回值
    /// - 无
    pub fn mark_failed(&self, error_message: String) {
        self.update(|snapshot| {
            snapshot.phase = RuntimeTaskPhase::Failed;
            snapshot.current_item_name = None;
            snapshot.last_error = Some(error_message);
        });
        self.schedule_clear(self.store.failure_ttl);
    }

    /// 以原子方式更新任务快照并广播。
    ///
    /// # 参数
    /// - `updater`：原地修改快照的闭包
    ///
    /// # 返回值
    /// - 无
    fn update<F>(&self, updater: F)
    where
        F: FnOnce(&mut RuntimeTaskSnapshot),
    {
        let mut snapshot = self
            .snapshot
            .lock()
            .expect("运行时任务快照锁不应在正常流程中中毒");
        updater(&mut snapshot);
        self.store.snapshot_tx.send_replace(Some(snapshot.clone()));
    }

    /// 为当前任务安排延时清理。
    ///
    /// # 参数
    /// - `ttl`：保留时长
    ///
    /// # 返回值
    /// - 无
    fn schedule_clear(&self, ttl: Duration) {
        let store = self.store.clone();
        let task_id = self
            .snapshot
            .lock()
            .expect("运行时任务快照锁不应在正常流程中中毒")
            .task_id
            .clone();
        tokio::spawn(async move {
            tokio::time::sleep(ttl).await;
            store.clear_if_current(&task_id);
        });
    }
}

#[cfg(test)]
mod tests;

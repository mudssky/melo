use std::sync::Arc;

use tokio::sync::watch;

use crate::core::error::MeloResult;
use crate::core::model::player::PlayerSnapshot;
use crate::domain::player::backend::{NoopBackend, PlaybackBackend};
use crate::domain::player::rodio_backend::RodioBackend;
use crate::domain::player::service::PlayerService;

/// daemon 共享应用状态。
#[derive(Clone)]
pub struct AppState {
    /// 播放服务。
    pub player: Arc<PlayerService>,
    snapshot_tx: watch::Sender<PlayerSnapshot>,
}

impl AppState {
    /// 使用生产播放后端构造应用状态。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - `MeloResult<Self>`：生产用应用状态
    pub fn new() -> MeloResult<Self> {
        let backend = Arc::new(RodioBackend::new()?);
        Ok(Self::with_backend(backend))
    }

    /// 使用指定后端构造应用状态。
    ///
    /// # 参数
    /// - `backend`：播放后端
    ///
    /// # 返回值
    /// - `Self`：应用状态
    pub fn with_backend(backend: Arc<dyn PlaybackBackend>) -> Self {
        let player = Arc::new(PlayerService::new(backend));
        let (snapshot_tx, _snapshot_rx) = watch::channel(PlayerSnapshot::default());
        Self {
            player,
            snapshot_tx,
        }
    }

    /// 推送一份播放器快照到订阅者。
    ///
    /// # 参数
    /// - `snapshot`：待广播的播放器快照
    ///
    /// # 返回值
    /// - `MeloResult<()>`：广播结果
    pub fn push_snapshot(&self, snapshot: PlayerSnapshot) -> MeloResult<()> {
        self.snapshot_tx.send_replace(snapshot);
        Ok(())
    }

    /// 创建播放器快照订阅器。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - `watch::Receiver<PlayerSnapshot>`：快照订阅器
    pub fn snapshot_receiver(&self) -> watch::Receiver<PlayerSnapshot> {
        self.snapshot_tx.subscribe()
    }

    /// 构造测试用应用状态。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - `Self`：测试用状态
    pub async fn for_test() -> Self {
        let backend = Arc::new(NoopBackend);
        Self::with_backend(backend)
    }
}

/// 构造测试用 router。
///
/// # 参数
/// - 无
///
/// # 返回值
/// - `Router`：测试路由
pub async fn test_router() -> axum::Router {
    let state = AppState::for_test().await;
    crate::daemon::server::router(state)
}

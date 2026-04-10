use std::sync::Arc;

use crate::core::error::MeloResult;
use crate::domain::player::backend::{NoopBackend, PlaybackBackend};
use crate::domain::player::rodio_backend::RodioBackend;
use crate::domain::player::service::PlayerService;

/// daemon 共享应用状态。
#[derive(Clone)]
pub struct AppState {
    /// 播放服务。
    pub player: Arc<PlayerService>,
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
        player.start_runtime_event_loop();
        player.start_progress_loop();
        Self { player }
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

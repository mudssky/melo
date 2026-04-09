use std::sync::Arc;

use crate::domain::player::backend::NoopBackend;
use crate::domain::player::service::PlayerService;

/// daemon 共享应用状态。
#[derive(Clone)]
pub struct AppState {
    /// 播放服务。
    pub player: Arc<PlayerService>,
}

impl AppState {
    /// 构造测试用应用状态。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回
    /// - `Self`：测试用状态
    pub async fn for_test() -> Self {
        let backend = Arc::new(NoopBackend);
        let player = Arc::new(PlayerService::new(backend));
        Self { player }
    }
}

/// 构造测试用 router。
///
/// # 参数
/// - 无
///
/// # 返回
/// - `Router`：测试路由
pub async fn test_router() -> axum::Router {
    let state = AppState::for_test().await;
    crate::daemon::server::router(state)
}

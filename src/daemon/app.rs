use std::sync::Arc;

use crate::core::error::MeloResult;
use crate::domain::player::backend::{NoopBackend, PlaybackBackend};
use crate::domain::player::rodio_backend::RodioBackend;
use crate::domain::player::service::PlayerService;
use crate::domain::player::session_store::PlayerSessionStore;

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

    /// 使用指定后端与会话仓储构造应用状态，并在启动时恢复会话。
    ///
    /// # 参数
    /// - `backend`：播放后端
    /// - `session_store`：播放器会话仓储
    ///
    /// # 返回值
    /// - `MeloResult<Self>`：应用状态
    pub async fn with_backend_and_session_store(
        backend: Arc<dyn PlaybackBackend>,
        session_store: Arc<PlayerSessionStore>,
    ) -> MeloResult<Self> {
        let player = Arc::new(PlayerService::new(backend));
        player.start_runtime_event_loop();
        player.start_progress_loop();

        if let Some(persisted) = session_store.load().await? {
            let _ = player.restore_persisted_session(persisted).await?;
        }

        let state = Self {
            player: Arc::clone(&player),
        };
        state.spawn_session_save_loop(session_store);
        Ok(state)
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

    /// 启动播放器会话保存循环。
    ///
    /// # 参数
    /// - `session_store`：播放器会话仓储
    ///
    /// # 返回值
    /// - 无
    fn spawn_session_save_loop(&self, session_store: Arc<PlayerSessionStore>) {
        let player = Arc::clone(&self.player);
        tokio::spawn(async move {
            let mut receiver = player.subscribe();
            let mut last_saved = None;
            loop {
                if receiver.changed().await.is_err() {
                    break;
                }

                let current = player.export_persisted_session().await;
                if session_store.should_persist(last_saved.as_ref(), &current)
                    && session_store.save(&current).await.is_ok()
                {
                    last_saved = Some(current);
                }
            }
        });
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

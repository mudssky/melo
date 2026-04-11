use std::sync::Arc;

use crate::core::config::settings::Settings;
use crate::core::error::MeloResult;
use crate::domain::library::service::LibraryService;
use crate::domain::player::backend::{NoopBackend, PlaybackBackend};
use crate::domain::player::factory;
use crate::domain::player::service::PlayerService;
use crate::domain::player::session_store::PlayerSessionStore;
use crate::domain::playlist::service::PlaylistService;

/// daemon 共享应用状态。
#[derive(Clone)]
pub struct AppState {
    /// 播放服务。
    pub player: Arc<PlayerService>,
    /// 全局配置。
    pub settings: Settings,
    /// 直接打开服务。
    pub open: Arc<crate::domain::open::service::OpenService>,
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
        let settings = Settings::load()?;
        let backend = factory::build_backend(&settings)?;
        Ok(Self::with_backend_and_settings(
            backend,
            settings,
            LibraryService::with_lofty,
        ))
    }

    /// 使用指定后端构造应用状态。
    ///
    /// # 参数
    /// - `backend`：播放后端
    ///
    /// # 返回值
    /// - `Self`：应用状态
    pub fn with_backend(backend: Arc<dyn PlaybackBackend>) -> Self {
        Self::with_backend_and_settings(backend, Settings::default(), LibraryService::for_test)
    }

    /// 使用指定后端与配置构造应用状态。
    ///
    /// # 参数
    /// - `backend`：播放后端
    /// - `settings`：全局配置
    /// - `library_factory`：媒体库服务工厂
    ///
    /// # 返回值
    /// - `Self`：应用状态
    fn with_backend_and_settings<F>(
        backend: Arc<dyn PlaybackBackend>,
        settings: Settings,
        library_factory: F,
    ) -> Self
    where
        F: FnOnce(Settings) -> LibraryService,
    {
        let player = Arc::new(PlayerService::new(backend));
        player.start_runtime_event_loop();
        player.start_progress_loop();
        let library = library_factory(settings.clone());
        let playlists = PlaylistService::new(settings.clone());
        let open = Arc::new(crate::domain::open::service::OpenService::new(
            settings.clone(),
            library,
            playlists,
            Arc::clone(&player),
        ));
        Self {
            player,
            settings,
            open,
        }
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
        let settings = Settings::default();
        let player = Arc::new(PlayerService::new(backend));
        player.start_runtime_event_loop();
        player.start_progress_loop();

        if let Some(persisted) = session_store.load().await? {
            let _ = player.restore_persisted_session(persisted).await?;
        }

        let library = LibraryService::for_test(settings.clone());
        let playlists = PlaylistService::new(settings.clone());
        let open = Arc::new(crate::domain::open::service::OpenService::new(
            settings.clone(),
            library,
            playlists,
            Arc::clone(&player),
        ));
        let state = Self {
            player: Arc::clone(&player),
            settings,
            open,
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
        Self::for_test_with_settings(Settings::default()).await
    }

    /// 构造带指定配置的测试用应用状态。
    ///
    /// # 参数
    /// - `settings`：测试配置
    ///
    /// # 返回值
    /// - `Self`：测试用状态
    pub async fn for_test_with_settings(settings: Settings) -> Self {
        let backend = Arc::new(NoopBackend);
        Self::with_backend_and_settings(backend, settings, LibraryService::for_test)
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

    /// 通过共享的直接打开服务处理一个打开请求。
    ///
    /// # 参数
    /// - `request`：直接打开请求
    ///
    /// # 返回值
    /// - `crate::core::error::MeloResult<crate::domain::open::service::OpenResponse>`：打开结果
    pub async fn open_target(
        &self,
        request: crate::domain::open::service::OpenRequest,
    ) -> crate::core::error::MeloResult<crate::domain::open::service::OpenResponse> {
        self.open.open(request).await
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

/// 构造带指定配置的测试用 router。
///
/// # 参数
/// - `settings`：测试配置
///
/// # 返回值
/// - `Router`：测试路由
pub async fn test_router_with_settings(settings: Settings) -> axum::Router {
    let state = AppState::for_test_with_settings(settings).await;
    crate::daemon::server::router(state)
}

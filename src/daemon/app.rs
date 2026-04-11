use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use sysinfo::{Pid, System};
use tokio::sync::Notify;
use uuid::Uuid;

use crate::core::config::settings::Settings;
use crate::core::error::MeloResult;
use crate::domain::library::service::LibraryService;
use crate::domain::player::backend::{NoopBackend, PlaybackBackend};
use crate::domain::player::factory;
use crate::domain::player::service::PlayerService;
use crate::domain::player::session_store::PlayerSessionStore;
use crate::domain::playlist::service::PlaylistService;

/// daemon 运行时元数据。
#[derive(Debug, Clone)]
pub struct DaemonRuntimeMeta {
    /// 当前实例 ID。
    pub instance_id: String,
    /// 当前进程 ID。
    pub pid: u32,
    /// 启动时间。
    pub started_at: String,
    /// 当前版本。
    pub version: String,
    /// 当前后端名。
    pub backend: String,
    /// 固定日志文件路径。
    pub log_path: String,
}

impl DaemonRuntimeMeta {
    /// 为生产 daemon 生成运行时元数据。
    ///
    /// # 参数
    /// - `backend`：后端名称
    ///
    /// # 返回值
    /// - `MeloResult<Self>`：运行时元数据
    pub fn live(backend: &str) -> MeloResult<Self> {
        let paths = crate::daemon::registry::runtime_paths()?;
        Ok(Self {
            instance_id: format!("instance-{}", Uuid::new_v4()),
            pid: std::process::id(),
            started_at: crate::daemon::registry::now_started_at_text()?,
            version: env!("CARGO_PKG_VERSION").to_string(),
            backend: backend.to_string(),
            log_path: paths.log_file.to_string_lossy().to_string(),
        })
    }

    /// 为测试环境生成稳定的运行时元数据。
    ///
    /// # 参数
    /// - `backend`：后端名称
    ///
    /// # 返回值
    /// - `Self`：测试元数据
    pub fn for_test(backend: &str) -> Self {
        Self {
            instance_id: "test-instance-1".to_string(),
            pid: std::process::id(),
            started_at: current_process_started_at_text(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            backend: backend.to_string(),
            log_path: "C:/Temp/melo-tests/daemon.log".to_string(),
        }
    }

    /// 为测试环境生成带指定实例 ID 的运行时元数据。
    ///
    /// # 参数
    /// - `instance_id`：测试实例 ID
    /// - `backend`：后端名称
    ///
    /// # 返回值
    /// - `Self`：测试元数据
    #[cfg(test)]
    pub fn for_test_with_instance_id(instance_id: &str, backend: &str) -> Self {
        Self {
            instance_id: instance_id.to_string(),
            pid: std::process::id(),
            started_at: current_process_started_at_text(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            backend: backend.to_string(),
            log_path: "C:/Temp/melo-tests/daemon.log".to_string(),
        }
    }
}

/// 读取当前测试进程的真实启动时间文本，保证 pid + started_at 校验能命中本进程。
///
/// # 参数
/// - 无
///
/// # 返回值
/// - `String`：当前进程启动时间的 RFC3339 文本
fn current_process_started_at_text() -> String {
    let system = System::new_all();
    let pid = Pid::from_u32(std::process::id());
    system
        .process(pid)
        .and_then(|process| {
            crate::daemon::registry::started_at_text_from_unix_seconds(process.start_time()).ok()
        })
        .unwrap_or_else(|| "1970-01-01T00:00:00Z".to_string())
}

/// daemon 共享应用状态。
#[derive(Clone)]
pub struct AppState {
    /// 播放服务。
    pub player: Arc<PlayerService>,
    /// 全局配置。
    pub settings: Settings,
    /// 直接打开服务。
    pub open: Arc<crate::domain::open::service::OpenService>,
    /// daemon 运行时元数据。
    runtime: Arc<DaemonRuntimeMeta>,
    /// daemon 关闭通知器。
    shutdown_notify: Arc<Notify>,
    /// daemon 是否已收到关闭请求。
    shutdown_requested: Arc<AtomicBool>,
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
        let backend_name = backend.backend_name().to_string();
        let runtime = DaemonRuntimeMeta::live(&backend_name)?;
        Ok(Self::with_backend_and_runtime(
            backend,
            settings,
            runtime,
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
        let backend_name = backend.backend_name().to_string();
        Self::with_backend_and_runtime(
            backend,
            Settings::default(),
            DaemonRuntimeMeta::for_test(&backend_name),
            LibraryService::for_test,
        )
    }

    /// 使用指定后端与配置构造应用状态。
    ///
    /// # 参数
    /// - `backend`：播放后端
    /// - `settings`：全局配置
    /// - `runtime`：daemon 运行时元数据
    /// - `library_factory`：媒体库服务工厂
    ///
    /// # 返回值
    /// - `Self`：应用状态
    fn with_backend_and_runtime<F>(
        backend: Arc<dyn PlaybackBackend>,
        settings: Settings,
        runtime: DaemonRuntimeMeta,
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
            runtime: Arc::new(runtime),
            shutdown_notify: Arc::new(Notify::new()),
            shutdown_requested: Arc::new(AtomicBool::new(false)),
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
        let backend_name = backend.backend_name().to_string();
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
            runtime: Arc::new(DaemonRuntimeMeta::for_test(&backend_name)),
            shutdown_notify: Arc::new(Notify::new()),
            shutdown_requested: Arc::new(AtomicBool::new(false)),
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
        Self::with_backend_and_runtime(
            backend,
            settings,
            DaemonRuntimeMeta::for_test("noop"),
            LibraryService::for_test,
        )
    }

    /// 构造带指定实例 ID 的测试用应用状态。
    ///
    /// # 参数
    /// - `instance_id`：测试实例 ID
    ///
    /// # 返回值
    /// - `Self`：测试用状态
    #[cfg(test)]
    pub(crate) fn for_test_with_instance_id(instance_id: &str) -> Self {
        let backend = Arc::new(NoopBackend);
        Self::with_backend_and_runtime(
            backend,
            Settings::default(),
            DaemonRuntimeMeta::for_test_with_instance_id(instance_id, "noop"),
            LibraryService::for_test,
        )
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

    /// 返回当前 daemon 的系统状态响应。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - `crate::api::system::DaemonStatusResponse`：当前系统状态
    pub fn system_status(&self) -> crate::api::system::DaemonStatusResponse {
        crate::api::system::DaemonStatusResponse {
            instance_id: self.runtime.instance_id.clone(),
            pid: self.runtime.pid,
            started_at: self.runtime.started_at.clone(),
            version: self.runtime.version.clone(),
            backend: self.runtime.backend.clone(),
            log_path: self.runtime.log_path.clone(),
            shutdown_requested: self.shutdown_requested(),
        }
    }

    /// 为当前监听地址生成完整注册信息。
    ///
    /// # 参数
    /// - `listener_addr`：实际监听地址
    ///
    /// # 返回值
    /// - `crate::daemon::registry::DaemonRegistration`：当前 daemon 注册信息
    pub fn daemon_registration(
        &self,
        listener_addr: SocketAddr,
    ) -> crate::daemon::registry::DaemonRegistration {
        crate::daemon::registry::DaemonRegistration {
            instance_id: self.runtime.instance_id.clone(),
            base_url: format!("http://{listener_addr}"),
            pid: self.runtime.pid,
            started_at: self.runtime.started_at.clone(),
            version: self.runtime.version.clone(),
            backend: self.runtime.backend.clone(),
            host: listener_addr.ip().to_string(),
            port: listener_addr.port(),
            log_path: self.runtime.log_path.clone(),
        }
    }

    /// 返回当前 docs 可见性模式。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - `crate::core::config::settings::DaemonDocsMode`：docs 模式
    pub fn docs_mode(&self) -> crate::core::config::settings::DaemonDocsMode {
        self.settings.daemon.docs
    }

    /// 请求 daemon 进入关闭流程。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - 无
    pub fn request_shutdown(&self) {
        self.shutdown_requested.store(true, Ordering::SeqCst);
        self.shutdown_notify.notify_waiters();
    }

    /// 等待 daemon 收到关闭信号。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - 无
    pub async fn wait_for_shutdown(&self) {
        if self.shutdown_requested() {
            return;
        }
        self.shutdown_notify.notified().await;
    }

    /// 判断当前是否已收到关闭请求。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - `bool`：是否已请求关闭
    pub fn shutdown_requested(&self) -> bool {
        self.shutdown_requested.load(Ordering::SeqCst)
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

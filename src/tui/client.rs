use crate::core::error::MeloResult;
use crate::core::model::tui::TuiSnapshot;

/// TUI 远程客户端。
#[derive(Clone)]
pub struct TuiClient {
    ws_client: crate::tui::ws_client::WsClient,
    runtime_ws_client: crate::tui::ws_client::WsClient,
}

impl TuiClient {
    /// 根据 HTTP 基地址创建 TUI 客户端。
    ///
    /// # 参数
    /// - `base_url`：daemon 的 HTTP 基地址
    ///
    /// # 返回值
    /// - `Self`：TUI 客户端
    pub fn new(base_url: String) -> Self {
        let ws_base = if let Some(stripped) = base_url.strip_prefix("https://") {
            format!("wss://{stripped}")
        } else if let Some(stripped) = base_url.strip_prefix("http://") {
            format!("ws://{stripped}")
        } else {
            base_url
        };
        let ws_url = format!("{}/api/ws/tui", ws_base.trim_end_matches('/'));
        let runtime_ws_url = format!("{}/api/ws/playback/runtime", ws_base.trim_end_matches('/'));
        Self {
            ws_client: crate::tui::ws_client::WsClient::new(ws_url),
            runtime_ws_client: crate::tui::ws_client::WsClient::new(runtime_ws_url),
        }
    }

    /// 连接到 TUI 聚合快照流。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - `MeloResult<crate::tui::ws_client::WsSnapshotStream>`：持续快照流
    pub async fn connect(&self) -> MeloResult<crate::tui::ws_client::WsSnapshotStream> {
        self.ws_client.connect().await
    }

    /// 读取下一条 TUI 聚合快照。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - `MeloResult<TuiSnapshot>`：从 daemon 收到的聚合快照
    pub async fn next_snapshot(&self) -> MeloResult<TuiSnapshot> {
        let mut stream = self.connect().await?;
        stream.next_json::<TuiSnapshot>().await
    }

    /// 获取新客户端初始化所需的 bootstrap 快照。
    ///
    /// # 参数
    /// - `api_client`：HTTP API 客户端
    ///
    /// # 返回值
    /// - `MeloResult<crate::core::model::playback_runtime::ClientBootstrapSnapshot>`：bootstrap 快照
    pub async fn bootstrap(
        &self,
        api_client: &crate::cli::client::ApiClient,
    ) -> MeloResult<crate::core::model::playback_runtime::ClientBootstrapSnapshot> {
        api_client.bootstrap().await
    }

    /// 读取最新的低频首页聚合快照。
    ///
    /// # 参数
    /// - `api_client`：HTTP API 客户端
    ///
    /// # 返回值
    /// - `MeloResult<TuiSnapshot>`：最新首页聚合快照
    pub async fn refresh_home(
        &self,
        api_client: &crate::cli::client::ApiClient,
    ) -> MeloResult<TuiSnapshot> {
        api_client.tui_home().await
    }

    /// 连接到轻量播放运行时快照流。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - `MeloResult<crate::tui::ws_client::WsSnapshotStream>`：轻量运行时快照流
    pub async fn runtime_connect(&self) -> MeloResult<crate::tui::ws_client::WsSnapshotStream> {
        self.runtime_ws_client.connect().await
    }
}

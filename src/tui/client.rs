use crate::core::error::MeloResult;
use crate::core::model::player::PlayerSnapshot;

/// TUI 远程客户端。
#[derive(Clone)]
pub struct TuiClient {
    ws_client: crate::tui::ws_client::WsClient,
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
        let ws_url = format!("{}/api/ws/player", ws_base.trim_end_matches('/'));
        Self {
            ws_client: crate::tui::ws_client::WsClient::new(ws_url),
        }
    }

    /// 读取下一条播放器快照。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - `MeloResult<PlayerSnapshot>`：从 daemon 收到的快照
    pub async fn next_snapshot(&self) -> MeloResult<PlayerSnapshot> {
        self.ws_client.next_snapshot().await
    }
}

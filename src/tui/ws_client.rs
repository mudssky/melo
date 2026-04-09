use futures_util::StreamExt;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

use crate::core::error::{MeloError, MeloResult};
use crate::core::model::player::PlayerSnapshot;

/// 面向 WebSocket 的最小客户端封装。
#[derive(Clone)]
pub struct WsClient {
    url: String,
}

impl WsClient {
    /// 创建新的 WebSocket 客户端。
    ///
    /// # 参数
    /// - `url`：WebSocket 地址
    ///
    /// # 返回值
    /// - `Self`：WebSocket 客户端
    pub fn new(url: String) -> Self {
        Self { url }
    }

    /// 连接到 daemon 并读取第一条播放器快照。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - `MeloResult<PlayerSnapshot>`：读取到的播放器快照
    pub async fn next_snapshot(&self) -> MeloResult<PlayerSnapshot> {
        let (mut stream, _response) = connect_async(&self.url)
            .await
            .map_err(|err| MeloError::Message(err.to_string()))?;

        while let Some(message) = stream.next().await {
            match message.map_err(|err| MeloError::Message(err.to_string()))? {
                Message::Text(text) => {
                    return serde_json::from_str::<PlayerSnapshot>(&text)
                        .map_err(|err| MeloError::Message(err.to_string()));
                }
                Message::Close(_) => break,
                _ => {}
            }
        }

        Err(MeloError::Message("WebSocket 未收到播放器快照".to_string()))
    }
}

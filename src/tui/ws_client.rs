use futures_util::StreamExt;
use serde::de::DeserializeOwned;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async};

use crate::core::error::{MeloError, MeloResult};

/// 面向 WebSocket 的最小客户端封装。
#[derive(Clone)]
pub struct WsClient {
    url: String,
}

/// 面向持续快照消费场景的 WebSocket 流包装。
pub struct WsSnapshotStream {
    stream: WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>,
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

    /// 连接到 daemon 并返回可持续读取 JSON 快照的流。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - `MeloResult<WsSnapshotStream>`：已连接的快照流
    pub async fn connect(&self) -> MeloResult<WsSnapshotStream> {
        let (stream, _response) = connect_async(&self.url)
            .await
            .map_err(|err| MeloError::Message(err.to_string()))?;
        Ok(WsSnapshotStream { stream })
    }
}

impl WsSnapshotStream {
    /// 读取下一条 JSON 消息并反序列化成目标类型。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - `MeloResult<T>`：反序列化后的消息体
    pub async fn next_json<T>(&mut self) -> MeloResult<T>
    where
        T: DeserializeOwned,
    {
        while let Some(message) = self.stream.next().await {
            match message.map_err(|err| MeloError::Message(err.to_string()))? {
                Message::Text(text) => {
                    return serde_json::from_str::<T>(&text)
                        .map_err(|err| MeloError::Message(err.to_string()));
                }
                Message::Close(_) => break,
                _ => {}
            }
        }

        Err(MeloError::Message("WebSocket 未收到快照".to_string()))
    }
}

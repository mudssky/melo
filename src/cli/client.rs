use crate::core::error::{MeloError, MeloResult};
use crate::core::model::player::PlayerSnapshot;

/// 命令行客户端对 daemon HTTP API 的最小封装。
#[derive(Clone)]
pub struct ApiClient {
    client: reqwest::Client,
    base_url: String,
}

impl ApiClient {
    /// 创建新的 API 客户端。
    ///
    /// # 参数
    /// - `base_url`：daemon 基础地址
    ///
    /// # 返回
    /// - `Self`：客户端实例
    pub fn new(base_url: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            base_url,
        }
    }

    /// 从环境变量构造客户端。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回
    /// - `Self`：客户端实例
    pub fn from_env() -> Self {
        let base_url =
            std::env::var("MELO_BASE_URL").unwrap_or_else(|_| "http://127.0.0.1:8080".to_string());
        Self::new(base_url)
    }

    /// 获取播放器状态快照。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回
    /// - `MeloResult<PlayerSnapshot>`：状态快照
    pub async fn status(&self) -> MeloResult<PlayerSnapshot> {
        let url = format!("{}/api/player/status", self.base_url);
        self.client
            .get(url)
            .send()
            .await
            .map_err(|err| MeloError::Message(err.to_string()))?
            .json()
            .await
            .map_err(|err| MeloError::Message(err.to_string()))
    }

    /// 发送一个无请求体的 POST 命令。
    ///
    /// # 参数
    /// - `path`：API 路径
    ///
    /// # 返回
    /// - `MeloResult<()>`：调用结果
    pub async fn post_no_body(&self, path: &str) -> MeloResult<()> {
        let url = format!("{}{}", self.base_url, path);
        self.client
            .post(url)
            .send()
            .await
            .map_err(|err| MeloError::Message(err.to_string()))?;
        Ok(())
    }
}

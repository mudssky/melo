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
            .error_for_status()
            .map_err(|err| MeloError::Message(err.to_string()))?
            .json()
            .await
            .map_err(|err| MeloError::Message(err.to_string()))
    }

    /// 检查 daemon 健康状态。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回
    /// - `MeloResult<()>`：健康时返回 `Ok(())`
    pub async fn health(&self) -> MeloResult<()> {
        let url = format!("{}/api/system/health", self.base_url);
        self.client
            .get(url)
            .send()
            .await
            .map_err(|err| MeloError::Message(err.to_string()))?
            .error_for_status()
            .map_err(|err| MeloError::Message(err.to_string()))?;
        Ok(())
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
            .map_err(|err| MeloError::Message(err.to_string()))?
            .error_for_status()
            .map_err(|err| MeloError::Message(err.to_string()))?;
        Ok(())
    }

    /// 发送一个无请求体的 POST 命令，并读取播放器快照。
    ///
    /// # 参数
    /// - `path`：API 路径
    ///
    /// # 返回值
    /// - `MeloResult<PlayerSnapshot>`：接口返回的最新快照
    pub async fn post_json(&self, path: &str) -> MeloResult<PlayerSnapshot> {
        let url = format!("{}{}", self.base_url, path);
        self.client
            .post(url)
            .send()
            .await
            .map_err(|err| MeloError::Message(err.to_string()))?
            .error_for_status()
            .map_err(|err| MeloError::Message(err.to_string()))?
            .json()
            .await
            .map_err(|err| MeloError::Message(err.to_string()))
    }

    /// 发送带 JSON 请求体的 POST 命令，并读取播放器快照。
    ///
    /// # 参数
    /// - `path`：API 路径
    /// - `body`：JSON 请求体
    ///
    /// # 返回值
    /// - `MeloResult<PlayerSnapshot>`：接口返回的最新快照
    pub async fn post_json_with_body(
        &self,
        path: &str,
        body: serde_json::Value,
    ) -> MeloResult<PlayerSnapshot> {
        let url = format!("{}{}", self.base_url, path);
        self.client
            .post(url)
            .json(&body)
            .send()
            .await
            .map_err(|err| MeloError::Message(err.to_string()))?
            .error_for_status()
            .map_err(|err| MeloError::Message(err.to_string()))?
            .json()
            .await
            .map_err(|err| MeloError::Message(err.to_string()))
    }

    /// 获取当前队列快照。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - `MeloResult<PlayerSnapshot>`：当前快照
    pub async fn queue_show(&self) -> MeloResult<PlayerSnapshot> {
        let url = format!("{}/api/player/status", self.base_url);
        self.client
            .get(url)
            .send()
            .await
            .map_err(|err| MeloError::Message(err.to_string()))?
            .error_for_status()
            .map_err(|err| MeloError::Message(err.to_string()))?
            .json()
            .await
            .map_err(|err| MeloError::Message(err.to_string()))
    }

    /// 清空远端队列。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - `MeloResult<PlayerSnapshot>`：最新快照
    pub async fn queue_clear(&self) -> MeloResult<PlayerSnapshot> {
        let url = format!("{}/api/queue/clear", self.base_url);
        self.client
            .post(url)
            .send()
            .await
            .map_err(|err| MeloError::Message(err.to_string()))?
            .error_for_status()
            .map_err(|err| MeloError::Message(err.to_string()))?
            .json()
            .await
            .map_err(|err| MeloError::Message(err.to_string()))
    }

    /// 选择远端队列中的某一项并播放。
    ///
    /// # 参数
    /// - `index`：目标索引
    ///
    /// # 返回值
    /// - `MeloResult<PlayerSnapshot>`：最新快照
    pub async fn queue_play_index(&self, index: usize) -> MeloResult<PlayerSnapshot> {
        let url = format!("{}/api/queue/play", self.base_url);
        self.client
            .post(url)
            .json(&serde_json::json!({ "index": index }))
            .send()
            .await
            .map_err(|err| MeloError::Message(err.to_string()))?
            .error_for_status()
            .map_err(|err| MeloError::Message(err.to_string()))?
            .json()
            .await
            .map_err(|err| MeloError::Message(err.to_string()))
    }

    /// 删除远端队列中的某一项。
    ///
    /// # 参数
    /// - `index`：待删除索引
    ///
    /// # 返回值
    /// - `MeloResult<PlayerSnapshot>`：最新快照
    pub async fn queue_remove(&self, index: usize) -> MeloResult<PlayerSnapshot> {
        let url = format!("{}/api/queue/remove", self.base_url);
        self.client
            .post(url)
            .json(&serde_json::json!({ "index": index }))
            .send()
            .await
            .map_err(|err| MeloError::Message(err.to_string()))?
            .error_for_status()
            .map_err(|err| MeloError::Message(err.to_string()))?
            .json()
            .await
            .map_err(|err| MeloError::Message(err.to_string()))
    }

    /// 移动远端队列中的某一项。
    ///
    /// # 参数
    /// - `from`：源索引
    /// - `to`：目标索引
    ///
    /// # 返回值
    /// - `MeloResult<PlayerSnapshot>`：最新快照
    pub async fn queue_move(&self, from: usize, to: usize) -> MeloResult<PlayerSnapshot> {
        let url = format!("{}/api/queue/move", self.base_url);
        self.client
            .post(url)
            .json(&serde_json::json!({ "from": from, "to": to }))
            .send()
            .await
            .map_err(|err| MeloError::Message(err.to_string()))?
            .error_for_status()
            .map_err(|err| MeloError::Message(err.to_string()))?
            .json()
            .await
            .map_err(|err| MeloError::Message(err.to_string()))
    }

    /// 设置播放器音量。
    ///
    /// # 参数
    /// - `value`：目标音量百分比
    ///
    /// # 返回值
    /// - `MeloResult<PlayerSnapshot>`：最新快照
    pub async fn player_volume(&self, value: u8) -> MeloResult<PlayerSnapshot> {
        self.post_json_with_body(
            "/api/player/volume",
            serde_json::json!({ "volume_percent": value }),
        )
        .await
    }

    /// 将播放器切换到静音状态。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - `MeloResult<PlayerSnapshot>`：最新快照
    pub async fn player_mute(&self) -> MeloResult<PlayerSnapshot> {
        self.post_json("/api/player/mute").await
    }

    /// 取消播放器静音。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - `MeloResult<PlayerSnapshot>`：最新快照
    pub async fn player_unmute(&self) -> MeloResult<PlayerSnapshot> {
        self.post_json("/api/player/unmute").await
    }

    /// 读取播放器模式快照。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - `MeloResult<PlayerSnapshot>`：最新快照
    pub async fn player_mode_show(&self) -> MeloResult<PlayerSnapshot> {
        self.status().await
    }

    /// 设置播放器循环模式。
    ///
    /// # 参数
    /// - `mode`：目标循环模式
    ///
    /// # 返回值
    /// - `MeloResult<PlayerSnapshot>`：最新快照
    pub async fn player_mode_repeat(&self, mode: &str) -> MeloResult<PlayerSnapshot> {
        self.post_json_with_body(
            "/api/player/mode",
            serde_json::json!({ "repeat_mode": mode }),
        )
        .await
    }

    /// 设置播放器随机播放开关。
    ///
    /// # 参数
    /// - `enabled`：目标随机播放状态
    ///
    /// # 返回值
    /// - `MeloResult<PlayerSnapshot>`：最新快照
    pub async fn player_mode_shuffle(&self, enabled: bool) -> MeloResult<PlayerSnapshot> {
        self.post_json_with_body(
            "/api/player/mode",
            serde_json::json!({ "shuffle_enabled": enabled }),
        )
        .await
    }
}

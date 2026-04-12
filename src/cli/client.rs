use crate::api::response::ApiResponse;
use crate::api::system::{DaemonStatusResponse, HealthResponse};
use crate::core::error::{MeloError, MeloResult};
use crate::core::model::player::PlayerSnapshot;
use crate::domain::open::service::OpenResponse;

const DAEMON_PROBE_TIMEOUT_MS: u64 = 500;

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
            client: build_client(None),
            base_url,
        }
    }

    /// 创建用于 daemon 探测的短超时客户端。
    ///
    /// # 参数
    /// - `base_url`：daemon 基础地址
    ///
    /// # 返回
    /// - `Self`：带短超时保护的客户端实例
    pub fn new_probe(base_url: String) -> Self {
        Self {
            client: build_client(Some(std::time::Duration::from_millis(
                DAEMON_PROBE_TIMEOUT_MS,
            ))),
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

    /// 从环境变量、daemon 注册文件和默认配置中发现客户端目标。
    ///
    /// # 参数
    /// - `settings`：当前配置
    ///
    /// # 返回值
    /// - `MeloResult<Self>`：发现后的客户端实例
    pub async fn from_discovery(
        settings: &crate::core::config::settings::Settings,
    ) -> MeloResult<Self> {
        let base_url = crate::daemon::process::resolve_base_url(settings).await?;
        Ok(Self::new(base_url))
    }

    /// 返回客户端当前绑定的基础 URL。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - `&str`：基础 URL
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// 发送请求并统一解包 API 响应壳。
    ///
    /// # 参数
    /// - `request`：已配置好的请求构造器
    ///
    /// # 返回值
    /// - `MeloResult<T>`：解包后的业务数据
    async fn send_and_decode<T>(&self, request: reqwest::RequestBuilder) -> MeloResult<T>
    where
        T: serde::de::DeserializeOwned,
    {
        let response = request
            .send()
            .await
            .map_err(|err| MeloError::Message(err.to_string()))?;
        let status = response.status();
        let envelope: ApiResponse<T> = response
            .json()
            .await
            .map_err(|err| MeloError::Message(err.to_string()))?;

        if envelope.code != 0 {
            return Err(MeloError::Message(format!(
                "api_error:{}:{}",
                envelope.code, envelope.msg
            )));
        }

        envelope.data.ok_or_else(|| {
            MeloError::Message(format!("api_error:missing_data:status={}", status.as_u16()))
        })
    }

    /// 发送请求并验证空数据成功响应。
    ///
    /// # 参数
    /// - `request`：已配置好的请求构造器
    ///
    /// # 返回值
    /// - `MeloResult<()>`：调用结果
    async fn send_and_decode_empty(&self, request: reqwest::RequestBuilder) -> MeloResult<()> {
        let response = request
            .send()
            .await
            .map_err(|err| MeloError::Message(err.to_string()))?;
        let envelope: ApiResponse<serde_json::Value> = response
            .json()
            .await
            .map_err(|err| MeloError::Message(err.to_string()))?;

        if envelope.code != 0 {
            return Err(MeloError::Message(format!(
                "api_error:{}:{}",
                envelope.code, envelope.msg
            )));
        }

        Ok(())
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
        self.send_and_decode(self.client.get(url)).await
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
        self.send_and_decode_empty(self.client.get(url)).await
    }

    /// 读取 daemon 健康响应。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - `MeloResult<HealthResponse>`：健康响应
    pub async fn health_status(&self) -> MeloResult<HealthResponse> {
        let url = format!("{}/api/system/health", self.base_url);
        self.send_and_decode(self.client.get(url)).await
    }

    /// 读取 daemon 系统状态响应。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - `MeloResult<DaemonStatusResponse>`：系统状态响应
    pub async fn daemon_status(&self) -> MeloResult<DaemonStatusResponse> {
        let url = format!("{}/api/system/status", self.base_url);
        self.send_and_decode(self.client.get(url)).await
    }

    /// 请求 daemon 直接打开一个目标。
    ///
    /// # 参数
    /// - `target`：目标路径
    /// - `mode`：触发模式
    ///
    /// # 返回
    /// - `MeloResult<OpenResponse>`：打开结果
    pub async fn open_target(&self, target: String, mode: &str) -> MeloResult<OpenResponse> {
        let url = format!("{}/api/open", self.base_url);
        self.send_and_decode(
            self.client
                .post(url)
                .json(&serde_json::json!({ "target": target, "mode": mode })),
        )
        .await
    }

    /// 获取 TUI 首页聚合快照。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - `MeloResult<crate::core::model::tui::TuiSnapshot>`：TUI 首页快照
    pub async fn tui_home(&self) -> MeloResult<crate::core::model::tui::TuiSnapshot> {
        let url = format!("{}/api/tui/home", self.base_url);
        self.send_and_decode(self.client.get(url)).await
    }

    /// 预览指定歌单。
    ///
    /// # 参数
    /// - `name`：歌单名
    ///
    /// # 返回值
    /// - `MeloResult<crate::api::playlist::PlaylistPreviewResponse>`：歌单预览结果
    pub async fn playlist_preview(
        &self,
        name: &str,
    ) -> MeloResult<crate::api::playlist::PlaylistPreviewResponse> {
        let mut url = reqwest::Url::parse(&format!("{}/api/playlists/preview", self.base_url))
            .map_err(|err| MeloError::Message(err.to_string()))?;
        url.query_pairs_mut().append_pair("name", name);
        self.send_and_decode(self.client.get(url)).await
    }

    /// 播放指定歌单，并从给定索引起播。
    ///
    /// # 参数
    /// - `name`：歌单名
    /// - `start_index`：起播索引
    ///
    /// # 返回值
    /// - `MeloResult<crate::core::model::tui::TuiSnapshot>`：新的 TUI 聚合快照
    pub async fn playlist_play(
        &self,
        name: &str,
        start_index: usize,
    ) -> MeloResult<crate::core::model::tui::TuiSnapshot> {
        let url = format!("{}/api/playlists/play", self.base_url);
        self.send_and_decode(
            self.client
                .post(url)
                .json(&serde_json::json!({ "name": name, "start_index": start_index })),
        )
        .await
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
        self.send_and_decode_empty(self.client.post(url)).await
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
        self.send_and_decode(self.client.post(url)).await
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
        self.send_and_decode(self.client.post(url).json(&body))
            .await
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
        self.send_and_decode(self.client.get(url)).await
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
        self.send_and_decode(self.client.post(url)).await
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
        self.send_and_decode(
            self.client
                .post(url)
                .json(&serde_json::json!({ "index": index })),
        )
        .await
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
        self.send_and_decode(
            self.client
                .post(url)
                .json(&serde_json::json!({ "index": index })),
        )
        .await
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
        self.send_and_decode(
            self.client
                .post(url)
                .json(&serde_json::json!({ "from": from, "to": to })),
        )
        .await
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

/// 根据可选超时构建 reqwest 客户端。
///
/// # 参数
/// - `timeout`：可选的请求超时；为 `None` 时表示使用 reqwest 默认行为
///
/// # 返回值
/// - `reqwest::Client`：构建好的 HTTP 客户端
fn build_client(timeout: Option<std::time::Duration>) -> reqwest::Client {
    let mut builder = reqwest::Client::builder();
    if let Some(timeout) = timeout {
        builder = builder.timeout(timeout);
    }

    builder.build().unwrap_or_else(|_| reqwest::Client::new())
}

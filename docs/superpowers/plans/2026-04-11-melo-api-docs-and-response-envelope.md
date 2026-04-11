# Melo API 文档与统一响应封装 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 为 Melo daemon HTTP API 增加 OpenAPI/在线文档能力，并把 HTTP 响应统一为 `code/msg/data` 契约，同时更新 Rust CLI 客户端与文档导出脚本。

**Architecture:** 在 `src/api/` 边界新增统一响应壳和 API 错误映射，让 handler 只返回业务结果或 API 错误，再由 Axum 统一落成 `HTTP status + ApiResponse<T>`。OpenAPI 继续使用 `utoipa`，通过单独文档模块收口 schema、路由注册和导出入口，再由 `package.json` 调用仓库内脚本完成导出与校验。

**Tech Stack:** Rust, Axum, Reqwest, Serde, Utoipa, pnpm, Vitest

---

## File Structure

- Create: `src/api/response.rs`
  - 统一定义 `ApiResponse<T>`、成功包装辅助函数，以及 OpenAPI 需要的通用响应 schema。
- Create: `src/api/error.rs`
  - 定义 API 业务错误码、错误响应模型、`IntoResponse` 映射，以及从现有 `MeloError`/参数错误到 API 错误的转换。
- Create: `src/api/docs.rs`
  - 集中定义 `utoipa::OpenApi` 聚合、导出函数和在线文档所需的辅助入口。
- Modify: `src/api/mod.rs`
  - 导出新增的 `response`、`error`、`docs` 模块。
- Modify: `src/api/system.rs`
  - 将系统接口改成统一响应壳并补 schema 注解。
- Modify: `src/api/player.rs`
  - 将播放器接口改成统一响应壳、去掉 `unwrap()`、补 request/response schema 注解。
- Modify: `src/api/queue.rs`
  - 将队列接口改成统一响应壳、去掉 `unwrap()`、补 request/response schema 注解。
- Modify: `src/api/open.rs`
  - 将打开目标接口切到统一 API 错误与响应壳。
- Modify: `src/api/ws.rs`
  - 最小补充 WebSocket 接口文档说明，不改变消息协议。
- Modify: `src/core/error.rs`
  - 增加更可映射的错误变体或最小辅助方法，避免 API 层只能处理裸字符串。
- Modify: `src/daemon/server.rs`
  - 注册 `/api/openapi.json` 与 `/api/docs` 路由。
- Modify: `src/cli/client.rs`
  - 增加统一 `reqwest` 响应解包方法，所有 HTTP 方法改走统一解包逻辑。
- Create: `src/cli/client/tests.rs`
  - 为统一响应解包和错误转换补单元测试。
- Modify: `src/cli/mod.rs`
  - 挂载 `client/tests.rs` 测试模块声明。
- Modify: `tests/api_server.rs`
  - 更新已有 API 集成测试以断言统一响应壳、错误码和文档入口。
- Modify: `tests/cli_remote.rs`
  - 更新 CLI 集成测试以覆盖统一响应壳下的客户端行为。
- Create: `src/bin/export_openapi.rs`
  - 提供导出 OpenAPI JSON 的 Rust 入口，供 `package.json` 调用。
- Create: `scripts/api-docs/check-openapi.cjs`
  - 校验仓库中导出的 OpenAPI 文件是否与当前代码生成结果一致。
- Create: `docs/openapi/melo.openapi.json`
  - 作为静态导出的 OpenAPI 文件落点。
- Modify: `package.json`
  - 增加 `docs:api`、`docs:api:check`、`docs:api:serve` 等脚本。
- Modify: `tests/dev-cli/package-json.test.mts`
  - 断言新增脚本存在且命令字符串正确。

### Task 1: 建立统一响应壳与 API 错误映射

**Files:**
- Create: `src/api/response.rs`
- Create: `src/api/error.rs`
- Modify: `src/api/mod.rs`
- Modify: `src/core/error.rs`
- Test: `tests/api_server.rs`

- [ ] **Step 1: 写失败的 API 集成测试，先锁定成功响应壳与错误响应壳**

```rust
#[tokio::test]
async fn system_status_endpoint_wraps_payload_in_api_response() {
    let app = melo::daemon::app::test_router().await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/system/status")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(payload["code"], 0);
    assert_eq!(payload["msg"], "ok");
    assert_eq!(payload["data"]["backend"], "noop");
}

#[tokio::test]
async fn open_endpoint_returns_structured_error_body() {
    let app = melo::daemon::app::test_router().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/open")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"target":"cover.jpg","mode":"replace"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(payload["code"], 1302);
    assert!(payload["msg"].as_str().unwrap().contains("unsupported"));
    assert!(payload["data"].is_null());
}
```

- [ ] **Step 2: 运行失败测试，确认当前返回仍是裸对象或原始字符串**

Run: `rtk cargo test -q --test api_server system_status_endpoint_wraps_payload_in_api_response open_endpoint_returns_structured_error_body`

Expected: FAIL，提示 `code` 字段缺失或反序列化结构不匹配。

- [ ] **Step 3: 最小实现统一响应壳与 API 错误类型**

```rust
// src/api/response.rs
use serde::Serialize;

/// HTTP API 统一成功/失败响应壳。
#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct ApiResponse<T>
where
    T: Serialize,
{
    /// 业务错误码，`0` 表示成功。
    pub code: i32,
    /// 对调用方稳定的文本消息。
    pub msg: String,
    /// 实际业务数据。
    pub data: Option<T>,
}

impl<T> ApiResponse<T>
where
    T: Serialize,
{
    /// 包装成功响应。
    ///
    /// # 参数
    /// - `data`：业务数据
    ///
    /// # 返回值
    /// - `Self`：成功响应壳
    pub fn ok(data: T) -> Self {
        Self {
            code: 0,
            msg: "ok".to_string(),
            data: Some(data),
        }
    }

    /// 包装无数据成功响应。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - `ApiResponse<serde_json::Value>`：空数据成功响应
    pub fn ok_empty() -> ApiResponse<serde_json::Value> {
        ApiResponse {
            code: 0,
            msg: "ok".to_string(),
            data: None,
        }
    }
}
```

```rust
// src/api/error.rs
use axum::{Json, http::StatusCode, response::{IntoResponse, Response}};
use serde::Serialize;

use crate::api::response::ApiResponse;

/// HTTP API 统一错误。
#[derive(Debug, Clone)]
pub struct ApiError {
    pub status: StatusCode,
    pub code: i32,
    pub msg: String,
}

impl ApiError {
    pub fn invalid_request(msg: impl Into<String>) -> Self {
        Self { status: StatusCode::BAD_REQUEST, code: 1001, msg: msg.into() }
    }

    pub fn invalid_json(msg: impl Into<String>) -> Self {
        Self { status: StatusCode::BAD_REQUEST, code: 1002, msg: msg.into() }
    }

    pub fn unsupported_target(msg: impl Into<String>) -> Self {
        Self { status: StatusCode::BAD_REQUEST, code: 1302, msg: msg.into() }
    }

    pub fn internal(msg: impl Into<String>) -> Self {
        Self { status: StatusCode::INTERNAL_SERVER_ERROR, code: 1599, msg: msg.into() }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = Json(ApiResponse::<serde_json::Value> {
            code: self.code,
            msg: self.msg,
            data: None,
        });
        (self.status, body).into_response()
    }
}
```

- [ ] **Step 4: 把 `system` 与 `open` 先切到新壳，保证测试能过**

```rust
// src/api/system.rs
pub async fn status(
    State(state): State<AppState>,
) -> Json<ApiResponse<DaemonStatusResponse>> {
    Json(ApiResponse::ok(state.system_status()))
}
```

```rust
// src/api/open.rs
pub async fn open(
    State(state): State<AppState>,
    Json(request): Json<OpenRequest>,
) -> Result<Json<ApiResponse<OpenResponse>>, ApiError> {
    let response = state
        .open_target(request)
        .await
        .map_err(|err| ApiError::unsupported_target(err.to_string()))?;
    Ok(Json(ApiResponse::ok(response)))
}
```

- [ ] **Step 5: 运行测试确认通过**

Run: `rtk cargo test -q --test api_server system_status_endpoint_wraps_payload_in_api_response open_endpoint_returns_structured_error_body`

Expected: PASS

- [ ] **Step 6: 提交第一批响应壳基础设施**

```bash
rtk git add src/api/response.rs src/api/error.rs src/api/mod.rs src/core/error.rs src/api/system.rs src/api/open.rs tests/api_server.rs
rtk git commit -m "feat(api): add unified response envelope"
```

### Task 2: 将 player/queue/system 全量切换到统一响应并收掉 `unwrap()`

**Files:**
- Modify: `src/api/player.rs`
- Modify: `src/api/queue.rs`
- Modify: `src/api/system.rs`
- Modify: `src/core/error.rs`
- Test: `tests/api_server.rs`

- [ ] **Step 1: 写失败测试，锁定 player/queue 成功与错误路径的统一格式**

```rust
#[tokio::test]
async fn player_volume_endpoint_returns_wrapped_snapshot() {
    let app = melo::daemon::app::test_router().await;
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/player/volume")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"volume_percent":55}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(payload["code"], 0);
    assert_eq!(payload["data"]["volume_percent"], 55);
}

#[tokio::test]
async fn queue_play_endpoint_returns_business_error_when_index_is_invalid() {
    let app = melo::daemon::app::test_router().await;
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/queue/play")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"index":99}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(payload["code"], 1102);
}
```

- [ ] **Step 2: 运行失败测试，确认 `player`/`queue` 还在返回裸快照或 panic 路径**

Run: `rtk cargo test -q --test api_server player_volume_endpoint_returns_wrapped_snapshot queue_play_endpoint_returns_business_error_when_index_is_invalid`

Expected: FAIL，返回结构不符合或请求直接 panic。

- [ ] **Step 3: 为 `MeloError` 增加可映射的最小错误变体**

```rust
// src/core/error.rs
#[derive(Debug, Error)]
pub enum MeloError {
    #[error("{0}")]
    Message(String),
    #[error("invalid_queue_index:{0}")]
    InvalidQueueIndex(usize),
    #[error("invalid_volume:{0}")]
    InvalidVolume(u8),
    #[error("queue_empty")]
    QueueEmpty,
}
```

- [ ] **Step 4: 将 `player` 与 `queue` handler 改为 `Result<Json<ApiResponse<_>>, ApiError>`**

```rust
pub async fn set_volume(
    State(state): State<AppState>,
    Json(request): Json<PlayerVolumeRequest>,
) -> Result<Json<ApiResponse<PlayerSnapshot>>, ApiError> {
    let snapshot = state
        .player
        .set_volume_percent(request.volume_percent)
        .await
        .map_err(api_error_from_melo_error)?;
    Ok(Json(ApiResponse::ok(snapshot)))
}
```

```rust
pub async fn play_index(
    State(state): State<AppState>,
    Json(request): Json<QueueIndexRequest>,
) -> Result<Json<ApiResponse<PlayerSnapshot>>, ApiError> {
    let snapshot = state
        .player
        .play_index(request.index)
        .await
        .map_err(api_error_from_melo_error)?;
    Ok(Json(ApiResponse::ok(snapshot)))
}
```

- [ ] **Step 5: 增加 `MeloError -> ApiError` 映射**

```rust
pub fn api_error_from_melo_error(err: MeloError) -> ApiError {
    match err {
        MeloError::InvalidQueueIndex(_) => ApiError {
            status: StatusCode::BAD_REQUEST,
            code: 1102,
            msg: "invalid_queue_index".to_string(),
        },
        MeloError::InvalidVolume(_) => ApiError {
            status: StatusCode::BAD_REQUEST,
            code: 1101,
            msg: "invalid_volume".to_string(),
        },
        MeloError::QueueEmpty => ApiError {
            status: StatusCode::CONFLICT,
            code: 1201,
            msg: "queue_empty".to_string(),
        },
        MeloError::Message(message) => ApiError::internal(message),
    }
}
```

- [ ] **Step 6: 运行集成测试，确认 HTTP 格式与状态码稳定**

Run: `rtk cargo test -q --test api_server`

Expected: PASS，且 `player`、`queue`、`system` 相关断言全部适配新响应壳。

- [ ] **Step 7: 提交全量 HTTP 响应统一改造**

```bash
rtk git add src/api/player.rs src/api/queue.rs src/api/system.rs src/core/error.rs tests/api_server.rs
rtk git commit -m "feat(api): unify player and queue responses"
```

### Task 3: 为 HTTP API 补 OpenAPI 与在线文档入口

**Files:**
- Create: `src/api/docs.rs`
- Modify: `src/api/mod.rs`
- Modify: `src/api/system.rs`
- Modify: `src/api/player.rs`
- Modify: `src/api/queue.rs`
- Modify: `src/api/open.rs`
- Modify: `src/api/ws.rs`
- Modify: `src/daemon/server.rs`
- Test: `tests/api_server.rs`

- [ ] **Step 1: 写失败测试，先锁定文档 JSON 与页面入口**

```rust
#[tokio::test]
async fn openapi_json_endpoint_is_available() {
    let app = melo::daemon::app::test_router().await;
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/openapi.json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(payload["openapi"], "3.1.0");
    assert!(payload["paths"]["/api/player/status"].is_object());
}
```

```rust
#[tokio::test]
async fn docs_page_endpoint_is_available() {
    let app = melo::daemon::app::test_router().await;
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/docs")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}
```

- [ ] **Step 2: 运行失败测试，确认当前文档路由尚未暴露**

Run: `rtk cargo test -q --test api_server openapi_json_endpoint_is_available docs_page_endpoint_is_available`

Expected: FAIL，返回 `404 Not Found`。

- [ ] **Step 3: 新增 OpenAPI 聚合模块与导出函数**

```rust
// src/api/docs.rs
#[derive(utoipa::OpenApi)]
#[openapi(
    paths(
        crate::api::system::health,
        crate::api::system::status,
        crate::api::system::shutdown,
        crate::api::player::status,
        crate::api::player::play,
        crate::api::player::pause,
        crate::api::player::toggle,
        crate::api::player::stop,
        crate::api::player::next,
        crate::api::player::prev,
        crate::api::player::set_volume,
        crate::api::player::mute,
        crate::api::player::unmute,
        crate::api::player::set_mode,
        crate::api::queue::add,
        crate::api::queue::insert,
        crate::api::queue::clear,
        crate::api::queue::play_index,
        crate::api::queue::remove,
        crate::api::queue::move_item,
        crate::api::open::open,
        crate::api::ws::player_updates
    ),
    components(
        schemas(
            crate::api::response::ApiResponse<crate::api::system::DaemonStatusResponse>,
            crate::api::response::ApiResponse<crate::api::system::HealthResponse>,
            crate::api::response::ApiResponse<crate::core::model::player::PlayerSnapshot>,
            crate::api::response::ApiResponse<crate::domain::open::service::OpenResponse>
        )
    )
)]
pub struct MeloOpenApi;

pub fn openapi_json() -> String {
    MeloOpenApi::openapi().to_pretty_json().unwrap()
}
```

- [ ] **Step 4: 在 handler 上补 `utoipa::path` 注解并注册 `/api/openapi.json` 与 `/api/docs`**

```rust
// src/api/system.rs
#[utoipa::path(
    get,
    path = "/api/system/status",
    responses(
        (status = 200, description = "daemon 状态", body = ApiResponse<DaemonStatusResponse>)
    )
)]
pub async fn status(...) -> ... { ... }
```

```rust
// src/daemon/server.rs
Router::new()
    .route("/api/openapi.json", axum::routing::get(|| async {
        (
            [(axum::http::header::CONTENT_TYPE, "application/json")],
            crate::api::docs::openapi_json(),
        )
    }))
    .route("/api/docs", utoipa_swagger_ui::SwaggerUi::new("/api/docs").url("/api/openapi.json", crate::api::docs::MeloOpenApi::openapi()))
```

- [ ] **Step 5: 运行 API 测试，确认文档入口和主接口 schema 都能生成**

Run: `rtk cargo test -q --test api_server`

Expected: PASS，且新增 `/api/openapi.json`、`/api/docs` 断言通过。

- [ ] **Step 6: 提交 OpenAPI 与在线文档接入**

```bash
rtk git add src/api/docs.rs src/api/mod.rs src/api/system.rs src/api/player.rs src/api/queue.rs src/api/open.rs src/api/ws.rs src/daemon/server.rs tests/api_server.rs Cargo.toml
rtk git commit -m "feat(api): add openapi docs endpoints"
```

### Task 4: 更新 Rust CLI 客户端统一解包响应壳

**Files:**
- Modify: `src/cli/client.rs`
- Create: `src/cli/client/tests.rs`
- Modify: `src/cli/mod.rs`
- Modify: `tests/cli_remote.rs`

- [ ] **Step 1: 写失败测试，锁定客户端能解包成功响应并识别业务错误**

```rust
#[tokio::test]
async fn api_client_status_unwraps_api_response() {
    let app = melo::daemon::app::test_router().await;
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let client = melo::cli::client::ApiClient::new(format!("http://{addr}"));
    let snapshot = client.status().await.unwrap();
    assert!(snapshot.playback_state == "stopped" || snapshot.playback_state == "idle");
}
```

```rust
#[tokio::test]
async fn explicit_open_command_prints_wrapped_error_message() {
    let app = melo::daemon::app::test_router().await;
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let mut cmd = Command::cargo_bin("melo").unwrap();
    cmd.env("MELO_BASE_URL", format!("http://{addr}"));
    cmd.arg("cover.jpg");

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("1302"))
        .stderr(predicate::str::contains("unsupported"));
}
```

- [ ] **Step 2: 运行失败测试，确认旧客户端无法自动解包新响应壳**

Run: `rtk cargo test -q --test cli_remote explicit_open_command_prints_wrapped_error_message status_command_prints_json_snapshot`

Expected: FAIL，CLI 输出拿到的是整个响应壳或无法正确解析。

- [ ] **Step 3: 在 `ApiClient` 增加统一 `send_and_decode` 方法**

```rust
async fn send_and_decode<T>(
    &self,
    request: reqwest::RequestBuilder,
) -> MeloResult<T>
where
    T: serde::de::DeserializeOwned,
{
    let response = request
        .send()
        .await
        .map_err(|err| MeloError::Message(err.to_string()))?;
    let status = response.status();
    let body: crate::api::response::ApiResponse<T> = response
        .json()
        .await
        .map_err(|err| MeloError::Message(err.to_string()))?;

    if body.code != 0 {
        return Err(MeloError::Message(format!("api_error:{}:{}", body.code, body.msg)));
    }

    body.data.ok_or_else(|| {
        MeloError::Message(format!("api_error:{}:missing_data_for_status_{status}", body.code))
    })
}
```

- [ ] **Step 4: 将现有 `status`、`health_status`、`daemon_status`、`post_json`、`open_target` 等全部切到统一解包**

```rust
pub async fn status(&self) -> MeloResult<PlayerSnapshot> {
    let url = format!("{}/api/player/status", self.base_url);
    self.send_and_decode(self.client.get(url)).await
}

pub async fn open_target(&self, target: String, mode: &str) -> MeloResult<OpenResponse> {
    let url = format!("{}/api/open", self.base_url);
    self.send_and_decode(
        self.client
            .post(url)
            .json(&serde_json::json!({ "target": target, "mode": mode })),
    )
    .await
}
```

- [ ] **Step 5: 运行 CLI 与客户端测试，确认行为恢复**

Run: `rtk cargo test -q --test cli_remote`

Expected: PASS，CLI 远程命令仍能输出业务信息，错误路径能看到稳定业务码/消息。

- [ ] **Step 6: 提交客户端响应解包改造**

```bash
rtk git add src/cli/client.rs src/cli/client/tests.rs src/cli/mod.rs tests/cli_remote.rs
rtk git commit -m "feat(cli): decode api response envelope"
```

### Task 5: 增加 OpenAPI 导出入口、`package.json` 脚本与校验

**Files:**
- Create: `src/bin/export_openapi.rs`
- Create: `scripts/api-docs/check-openapi.cjs`
- Create: `docs/openapi/melo.openapi.json`
- Modify: `package.json`
- Modify: `tests/dev-cli/package-json.test.mts`

- [ ] **Step 1: 写失败测试，锁定 `package.json` 中的新脚本名称**

```ts
it('defines the API docs scripts', () => {
  expect(packageJson.scripts['docs:api']).toBe(
    'cargo run --quiet --bin export_openapi -- docs/openapi/melo.openapi.json',
  )
  expect(packageJson.scripts['docs:api:check']).toBe(
    'node ./scripts/api-docs/check-openapi.cjs',
  )
  expect(packageJson.scripts['docs:api:serve']).toBe(
    'node ./scripts/api-docs/check-openapi.cjs --print-url',
  )
})
```

- [ ] **Step 2: 运行失败测试，确认脚本尚未定义**

Run: `pnpm test:dev-cli`

Expected: FAIL，提示 `docs:api` 等脚本缺失。

- [ ] **Step 3: 增加 Rust 导出入口与 Node 校验脚本**

```rust
// src/bin/export_openapi.rs
fn main() {
    let output = std::env::args()
        .nth(1)
        .expect("missing output path");
    let json = melo::api::docs::openapi_json();
    std::fs::create_dir_all(std::path::Path::new(&output).parent().unwrap()).unwrap();
    std::fs::write(output, json).unwrap();
}
```

```js
// scripts/api-docs/check-openapi.cjs
const fs = require('node:fs')
const path = require('node:path')
const { execFileSync } = require('node:child_process')

const output = path.resolve(process.cwd(), 'docs/openapi/melo.openapi.json')
const generated = execFileSync(
  'cargo',
  ['run', '--quiet', '--bin', 'export_openapi', '--', output + '.tmp'],
  { encoding: 'utf8' },
)

if (process.argv.includes('--print-url')) {
  console.log('http://127.0.0.1:8080/api/docs')
  process.exit(0)
}

const current = fs.readFileSync(output, 'utf8')
const next = fs.readFileSync(output + '.tmp', 'utf8')
fs.rmSync(output + '.tmp')
if (current !== next) {
  console.error('openapi spec is outdated')
  process.exit(1)
}
```

- [ ] **Step 4: 更新 `package.json` 并生成一次静态 OpenAPI**

```json
{
  "scripts": {
    "docs:api": "cargo run --quiet --bin export_openapi -- docs/openapi/melo.openapi.json",
    "docs:api:check": "node ./scripts/api-docs/check-openapi.cjs",
    "docs:api:serve": "node ./scripts/api-docs/check-openapi.cjs --print-url"
  }
}
```

- [ ] **Step 5: 运行脚本与测试，确认导出和校验都可用**

Run: `pnpm docs:api`
Expected: `docs/openapi/melo.openapi.json` 被生成或刷新。

Run: `pnpm docs:api:check`
Expected: exit code `0`，无 “outdated” 报错。

Run: `pnpm test:dev-cli`
Expected: PASS

- [ ] **Step 6: 提交 API 文档脚本与导出文件**

```bash
rtk git add src/bin/export_openapi.rs scripts/api-docs/check-openapi.cjs docs/openapi/melo.openapi.json package.json tests/dev-cli/package-json.test.mts
rtk git commit -m "feat(docs): add openapi export scripts"
```

### Task 6: 全量验证并收尾

**Files:**
- Modify: `package.json`
- Modify: `tests/api_server.rs`
- Modify: `tests/cli_remote.rs`
- Modify: `docs/openapi/melo.openapi.json`

- [ ] **Step 1: 运行 Rust API 与 CLI 相关测试**

Run: `rtk cargo test -q --test api_server --test cli_remote`

Expected: PASS

- [ ] **Step 2: 运行前端脚本测试与 OpenAPI 校验**

Run: `pnpm test:dev-cli`

Expected: PASS

Run: `pnpm docs:api:check`

Expected: PASS

- [ ] **Step 3: 运行项目总体验证**

Run: `pnpm qa`

Expected: PASS，包含 format、lint、test 全部通过。

- [ ] **Step 4: 生成最终变更摘要并提交**

```bash
rtk git status --short
rtk git add src/api src/cli src/bin/export_openapi.rs scripts/api-docs package.json docs/openapi tests/api_server.rs tests/cli_remote.rs tests/dev-cli/package-json.test.mts
rtk git commit -m "feat(api): document and unify daemon http responses"
```

## Self-Review

- Spec coverage:
  - 统一响应壳与 `code=0` 成功约定：Task 1、Task 2
  - 错误码分层与 HTTP 状态映射：Task 1、Task 2
  - 去掉 handler `unwrap()`：Task 2
  - OpenAPI 与在线文档：Task 3
  - `package.json` 导出与校验脚本：Task 5
  - Rust CLI 统一解包：Task 4
  - 全量验证与 `pnpm qa`：Task 6
- Placeholder scan:
  - 已避免使用 `TBD`、`TODO`、`later` 等占位表达。
- Type consistency:
  - 统一使用 `ApiResponse<T>`、`ApiError`、`openapi_json()`、`send_and_decode()` 作为后续任务中的核心名字。

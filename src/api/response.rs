use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// HTTP API 统一响应壳。
///
/// # 参数
/// - `T`：实际业务数据类型
///
/// # 返回值
/// - 无
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ApiResponse<T> {
    /// 业务错误码，`0` 表示成功。
    pub code: i32,
    /// 对调用方稳定的文本消息。
    pub msg: String,
    /// 实际业务数据。
    pub data: Option<T>,
}

impl<T> ApiResponse<T> {
    /// 包装成功响应。
    ///
    /// # 参数
    /// - `data`：业务数据
    ///
    /// # 返回值
    /// - `Self`：统一成功响应
    pub fn ok(data: T) -> Self {
        Self {
            code: 0,
            msg: "ok".to_string(),
            data: Some(data),
        }
    }
}

impl ApiResponse<serde_json::Value> {
    /// 包装无数据成功响应。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - `Self`：无数据成功响应
    pub fn ok_empty() -> Self {
        Self {
            code: 0,
            msg: "ok".to_string(),
            data: None,
        }
    }
}

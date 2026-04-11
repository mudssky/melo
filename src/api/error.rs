use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};

use crate::api::response::ApiResponse;
use crate::core::error::MeloError;

/// HTTP API 统一错误。
#[derive(Debug, Clone)]
pub struct ApiError {
    /// 对应的 HTTP 状态码。
    pub status: StatusCode,
    /// 稳定业务错误码。
    pub code: i32,
    /// 对调用方稳定的错误消息。
    pub msg: String,
}

impl ApiError {
    /// 创建请求无效错误。
    ///
    /// # 参数
    /// - `msg`：错误消息
    ///
    /// # 返回值
    /// - `Self`：API 错误
    pub fn invalid_request(msg: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            code: 1001,
            msg: msg.into(),
        }
    }

    /// 创建目标类型不支持错误。
    ///
    /// # 参数
    /// - `msg`：错误消息
    ///
    /// # 返回值
    /// - `Self`：API 错误
    pub fn unsupported_target(msg: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            code: 1302,
            msg: msg.into(),
        }
    }

    /// 创建内部错误。
    ///
    /// # 参数
    /// - `msg`：错误消息
    ///
    /// # 返回值
    /// - `Self`：API 错误
    pub fn internal(msg: impl Into<String>) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            code: 1599,
            msg: msg.into(),
        }
    }
}

impl From<MeloError> for ApiError {
    fn from(value: MeloError) -> Self {
        match value {
            MeloError::Message(message) if message == "unsupported_open_format" => {
                Self::unsupported_target(message)
            }
            MeloError::Message(message) if message == "open_target_empty" => Self {
                status: StatusCode::CONFLICT,
                code: 1201,
                msg: message,
            },
            MeloError::Message(message) if message == "queue index out of range" => Self {
                status: StatusCode::BAD_REQUEST,
                code: 1102,
                msg: "invalid_queue_index".to_string(),
            },
            MeloError::Message(message) if message == "queue is empty" => Self {
                status: StatusCode::CONFLICT,
                code: 1201,
                msg: "queue_empty".to_string(),
            },
            MeloError::Message(message) if message == "queue has no next item" => Self {
                status: StatusCode::CONFLICT,
                code: 1202,
                msg: "queue_no_next".to_string(),
            },
            MeloError::Message(message) if message == "queue has no previous item" => Self {
                status: StatusCode::CONFLICT,
                code: 1202,
                msg: "queue_no_prev".to_string(),
            },
            MeloError::Message(message) => Self::internal(message),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(ApiResponse::<serde_json::Value> {
                code: self.code,
                msg: self.msg,
                data: None,
            }),
        )
            .into_response()
    }
}

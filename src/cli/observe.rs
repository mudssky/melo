use crate::cli::client::ApiClient;
use crate::core::error::{MeloError, MeloResult};

/// 只读命令观察到的 daemon 状态。
pub enum ObservedDaemon {
    /// daemon 可用，可继续发起只读请求。
    Running {
        client: ApiClient,
        base_url: String,
        docs_url: String,
        openapi_url: String,
    },
    /// daemon 不可用，应给出友好提示。
    Unavailable { reason: String, hint: String },
}

/// 观察只读命令所依赖的 daemon 是否可用。
///
/// # 参数
/// - 无
///
/// # 返回值
/// - `MeloResult<ObservedDaemon>`：观察结果
pub async fn observe_read_only_daemon() -> MeloResult<ObservedDaemon> {
    let client = if let Ok(base_url) = std::env::var("MELO_BASE_URL") {
        ApiClient::new(base_url)
    } else if let Some(registration) = crate::daemon::registry::load_registration().await? {
        ApiClient::new(registration.base_url)
    } else {
        return Ok(unavailable_daemon());
    };

    if client.health().await.is_err() {
        return Ok(unavailable_daemon());
    }

    let base_url = client.base_url().trim_end_matches('/').to_string();
    Ok(ObservedDaemon::Running {
        docs_url: format!("{base_url}/api/docs/"),
        openapi_url: format!("{base_url}/api/openapi.json"),
        client,
        base_url,
    })
}

/// 打印友好提示并返回统一错误。
///
/// # 参数
/// - `reason`：失败原因
/// - `hint`：下一步提示
///
/// # 返回值
/// - `MeloError`：统一错误对象
pub fn print_unavailable_and_error(reason: &str, hint: &str) -> MeloError {
    eprintln!("{reason}");
    eprintln!("{hint}");
    MeloError::Message("daemon_unavailable".to_string())
}

/// 生成统一的 daemon 不可用结果。
///
/// # 参数
/// - 无
///
/// # 返回值
/// - `ObservedDaemon`：不可用状态
fn unavailable_daemon() -> ObservedDaemon {
    ObservedDaemon::Unavailable {
        reason: "daemon is unavailable".to_string(),
        hint: "run `melo daemon start`".to_string(),
    }
}

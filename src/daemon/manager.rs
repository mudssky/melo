use std::time::Duration;

use crate::core::config::settings::Settings;
use crate::core::error::{MeloError, MeloResult};
use crate::daemon::observe::{DaemonObservation, DaemonState, observe_with_paths};
use crate::daemon::registry::DaemonPaths;

/// daemon 启动结果。
#[derive(Debug, Clone, serde::Serialize)]
pub struct StartResult {
    /// 执行动作。
    pub action: &'static str,
    /// 最新观察结果。
    pub observation: DaemonObservation,
}

/// daemon 停止结果。
#[derive(Debug, Clone, serde::Serialize)]
pub struct StopResult {
    /// 执行动作。
    pub action: &'static str,
    /// 最新观察结果。
    pub observation: DaemonObservation,
}

/// daemon 重启结果。
#[derive(Debug, Clone, serde::Serialize)]
pub struct RestartResult {
    /// 执行动作。
    pub action: &'static str,
    /// 重启前观察结果。
    pub previous: DaemonObservation,
    /// 重启后观察结果。
    pub current: DaemonObservation,
}

/// 启动 daemon，并等待到可观测状态稳定。
///
/// # 参数
/// - `settings`：当前配置
///
/// # 返回值
/// - `MeloResult<StartResult>`：启动结果
pub async fn start(settings: &Settings) -> MeloResult<StartResult> {
    let paths = crate::daemon::registry::runtime_paths()?;
    start_with_paths(
        settings,
        &paths,
        crate::daemon::process::spawn_background_daemon,
    )
    .await
}

/// 带注入 spawn 行为地启动 daemon，便于单测。
///
/// # 参数
/// - `settings`：当前配置
/// - `paths`：运行期文件路径
/// - `spawn`：实际启动逻辑
///
/// # 返回值
/// - `MeloResult<StartResult>`：启动结果
pub async fn start_with_paths<F>(
    settings: &Settings,
    paths: &DaemonPaths,
    spawn: F,
) -> MeloResult<StartResult>
where
    F: FnOnce() -> MeloResult<()>,
{
    let current = observe_with_paths(settings, paths).await?;
    if current.state == DaemonState::Running {
        return Ok(StartResult {
            action: "already_running",
            observation: current,
        });
    }

    if current.registration_exists && current.state != DaemonState::Running {
        crate::daemon::registry::clear_registration_from(&paths.state_file).await?;
    }

    spawn()?;
    let observation =
        wait_for_state(settings, paths, |value| value.state == DaemonState::Running).await?;

    Ok(StartResult {
        action: "started",
        observation,
    })
}

/// 停止 daemon。
///
/// # 参数
/// - `settings`：当前配置
///
/// # 返回值
/// - `MeloResult<StopResult>`：停止结果
pub async fn stop(settings: &Settings) -> MeloResult<StopResult> {
    let paths = crate::daemon::registry::runtime_paths()?;
    stop_with_paths(settings, &paths).await
}

/// 按指定路径集合停止 daemon。
///
/// # 参数
/// - `settings`：当前配置
/// - `paths`：运行期文件路径
///
/// # 返回值
/// - `MeloResult<StopResult>`：停止结果
pub async fn stop_with_paths(settings: &Settings, paths: &DaemonPaths) -> MeloResult<StopResult> {
    let observation = observe_with_paths(settings, paths).await?;
    match observation.state {
        DaemonState::NotRunning => Ok(StopResult {
            action: "not_running",
            observation,
        }),
        DaemonState::RegisteredButUnhealthy => {
            crate::daemon::registry::clear_registration_from(&paths.state_file).await?;
            let current = observe_with_paths(settings, paths).await?;
            Ok(StopResult {
                action: "stale_registration_cleared",
                observation: current,
            })
        }
        DaemonState::Running | DaemonState::Stopping => {
            if let Some(base_url) = observation.base_url.clone() {
                let _ = crate::cli::client::ApiClient::new(base_url)
                    .post_no_body("/api/system/shutdown")
                    .await;
            }

            let current = wait_for_state(settings, paths, |value| {
                matches!(
                    value.state,
                    DaemonState::NotRunning | DaemonState::RegisteredButUnhealthy
                )
            })
            .await?;
            crate::daemon::registry::clear_registration_from(&paths.state_file).await?;
            Ok(StopResult {
                action: "stopped",
                observation: current,
            })
        }
    }
}

/// 重启 daemon，并确认旧实例退出、新实例健康。
///
/// # 参数
/// - `settings`：当前配置
///
/// # 返回值
/// - `MeloResult<RestartResult>`：重启结果
pub async fn restart(settings: &Settings) -> MeloResult<RestartResult> {
    let paths = crate::daemon::registry::runtime_paths()?;
    restart_with_paths(
        settings,
        &paths,
        crate::daemon::process::spawn_background_daemon,
    )
    .await
}

/// 带注入 spawn 行为地重启 daemon。
///
/// # 参数
/// - `settings`：当前配置
/// - `paths`：运行期文件路径
/// - `spawn`：实际启动逻辑
///
/// # 返回值
/// - `MeloResult<RestartResult>`：重启结果
pub async fn restart_with_paths<F>(
    settings: &Settings,
    paths: &DaemonPaths,
    spawn: F,
) -> MeloResult<RestartResult>
where
    F: FnOnce() -> MeloResult<()>,
{
    let previous = observe_with_paths(settings, paths).await?;
    let _ = stop_with_paths(settings, paths).await?;
    let current = start_with_paths(settings, paths, spawn).await?.observation;
    Ok(RestartResult {
        action: "restarted",
        previous,
        current,
    })
}

/// 确保 daemon 已可访问，供带副作用命令复用。
///
/// # 参数
/// - `settings`：当前配置
///
/// # 返回值
/// - `MeloResult<String>`：健康 daemon 的 base URL
pub async fn ensure_running(settings: &Settings) -> MeloResult<String> {
    let observation = start(settings).await?.observation;
    observation
        .base_url
        .ok_or_else(|| MeloError::Message("daemon_not_running".to_string()))
}

/// 读取日志文件尾部。
///
/// # 参数
/// - `paths`：运行期文件路径
/// - `tail`：保留末尾行数
///
/// # 返回值
/// - `MeloResult<String>`：日志尾部文本
pub async fn read_logs_with_paths(paths: &DaemonPaths, tail: usize) -> MeloResult<String> {
    let contents = tokio::fs::read_to_string(&paths.log_file)
        .await
        .map_err(|err| MeloError::Message(err.to_string()))?;
    Ok(tail_lines(&contents, tail))
}

/// 保留文本的最后 N 行。
///
/// # 参数
/// - `contents`：原始文本
/// - `tail`：目标行数
///
/// # 返回值
/// - `String`：截断后的文本
pub fn tail_lines(contents: &str, tail: usize) -> String {
    let lines = contents.lines().collect::<Vec<_>>();
    let start = lines.len().saturating_sub(tail);
    lines[start..].join("\n")
}

/// 等待观察结果满足给定条件。
///
/// # 参数
/// - `settings`：当前配置
/// - `paths`：运行期文件路径
/// - `predicate`：目标条件
///
/// # 返回值
/// - `MeloResult<DaemonObservation>`：满足条件时的观察结果
async fn wait_for_state<F>(
    settings: &Settings,
    paths: &DaemonPaths,
    predicate: F,
) -> MeloResult<DaemonObservation>
where
    F: Fn(&DaemonObservation) -> bool,
{
    for _ in 0..40 {
        let observation = observe_with_paths(settings, paths).await?;
        if predicate(&observation) {
            return Ok(observation);
        }
        tokio::time::sleep(Duration::from_millis(150)).await;
    }

    Err(MeloError::Message(
        "daemon_state_transition_timeout".to_string(),
    ))
}

#[cfg(test)]
mod tests;

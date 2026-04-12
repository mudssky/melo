use std::path::Path;
use std::time::Duration;

use sysinfo::{Pid, System};

use crate::api::system::DaemonStatusResponse;
use crate::core::config::settings::Settings;
use crate::core::error::MeloResult;
use crate::daemon::registry::{DaemonPaths, DaemonRegistration};

/// daemon 生命周期状态。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub enum DaemonState {
    /// 没有可用的受管 daemon。
    NotRunning,
    /// 注册存在，但健康或进程侧不可信。
    RegisteredButUnhealthy,
    /// daemon 运行正常。
    Running,
    /// daemon 已收到关闭请求，正在退出。
    Stopping,
}

/// daemon 统一观察结果。
#[derive(Debug, Clone, serde::Serialize)]
pub struct DaemonObservation {
    /// 当前统一状态。
    pub state: DaemonState,
    /// 注册文件是否存在。
    pub registration_exists: bool,
    /// 注册文件路径。
    pub registration_path: String,
    /// 注册中的基础地址。
    pub base_url: Option<String>,
    /// 注册中的实例 ID。
    pub instance_id: Option<String>,
    /// 注册中的 pid。
    pub pid: Option<u32>,
    /// 注册中的启动时间。
    pub started_at: Option<String>,
    /// 注册中的后端名。
    pub backend: Option<String>,
    /// 注册中的 host。
    pub host: Option<String>,
    /// 注册中的 port。
    pub port: Option<u16>,
    /// 进程是否存在。
    pub process_exists: bool,
    /// 进程启动时间是否与注册匹配。
    pub process_start_time_matches: bool,
    /// 实际探测到的 pid。
    pub actual_pid: Option<u32>,
    /// 实际探测到的进程路径。
    pub actual_process_path: Option<String>,
    /// HTTP 探测是否成功。
    pub health_ok: bool,
    /// HTTP 实例 ID 是否与注册一致。
    pub http_instance_id_matches: Option<bool>,
    /// daemon 是否已收到关闭请求。
    pub shutdown_requested: bool,
    /// 日志文件路径。
    pub log_path: Option<String>,
    /// 日志文件是否可读。
    pub log_readable: bool,
}

/// doctor 检查级别。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub enum DoctorLevel {
    OK,
    WARN,
    FAIL,
}

/// 单条 doctor 检查项。
#[derive(Debug, Clone, serde::Serialize)]
pub struct DoctorCheck {
    /// 检查项编码。
    pub code: &'static str,
    /// 检查等级。
    pub level: DoctorLevel,
    /// 人类可读摘要。
    pub summary: String,
    /// 证据文本。
    pub evidence: String,
}

/// daemon 诊断报告。
#[derive(Debug, Clone, serde::Serialize)]
pub struct DoctorReport {
    /// 总结论。
    pub conclusion: DoctorLevel,
    /// 检查项列表。
    pub checks: Vec<DoctorCheck>,
    /// 生成报告时使用的观察结果。
    pub observation: DaemonObservation,
}

/// 计算统一生命周期状态。
///
/// # 参数
/// - `process_exists`：进程是否存在
/// - `process_start_time_matches`：启动时间是否匹配
/// - `http_instance_id_matches`：HTTP 实例 ID 是否匹配
/// - `shutdown_requested`：daemon 是否正在关闭
///
/// # 返回值
/// - `DaemonState`：统一状态
pub fn classify_state(
    process_exists: bool,
    process_start_time_matches: bool,
    http_instance_id_matches: Option<bool>,
    shutdown_requested: bool,
) -> DaemonState {
    match http_instance_id_matches {
        Some(true) if process_exists && process_start_time_matches && shutdown_requested => {
            DaemonState::Stopping
        }
        Some(true) if process_exists && process_start_time_matches => DaemonState::Running,
        _ if process_exists || http_instance_id_matches.is_some() => {
            DaemonState::RegisteredButUnhealthy
        }
        _ => DaemonState::RegisteredButUnhealthy,
    }
}

/// 观测指定路径集合下的 daemon 状态。
///
/// # 参数
/// - `_settings`：当前配置
/// - `paths`：daemon 运行期文件路径
///
/// # 返回值
/// - `MeloResult<DaemonObservation>`：观察结果
pub async fn observe_with_paths(
    _settings: &Settings,
    paths: &DaemonPaths,
) -> MeloResult<DaemonObservation> {
    let registration = crate::daemon::registry::load_registration_from(&paths.state_file).await?;
    let registration_path = paths.state_file.to_string_lossy().to_string();

    let Some(registration) = registration else {
        return Ok(DaemonObservation {
            state: DaemonState::NotRunning,
            registration_exists: false,
            registration_path,
            base_url: None,
            instance_id: None,
            pid: None,
            started_at: None,
            backend: None,
            host: None,
            port: None,
            process_exists: false,
            process_start_time_matches: false,
            actual_pid: None,
            actual_process_path: None,
            health_ok: false,
            http_instance_id_matches: None,
            shutdown_requested: false,
            log_path: Some(paths.log_file.to_string_lossy().to_string()),
            log_readable: log_is_readable(&paths.log_file).await,
        });
    };

    let process = observe_process(&registration);
    let daemon_status = probe_http_status(&registration).await;
    let http_instance_id_matches = daemon_status
        .as_ref()
        .map(|status| status.instance_id == registration.instance_id);
    let health_ok = daemon_status.is_some();
    let shutdown_requested = daemon_status
        .as_ref()
        .map(|status| status.shutdown_requested)
        .unwrap_or(false);
    let log_path = registration.log_path.clone();

    Ok(DaemonObservation {
        state: classify_state(
            process.process_exists,
            process.process_start_time_matches,
            http_instance_id_matches,
            shutdown_requested,
        ),
        registration_exists: true,
        registration_path,
        base_url: Some(registration.base_url.clone()),
        instance_id: Some(registration.instance_id.clone()),
        pid: Some(registration.pid),
        started_at: Some(registration.started_at.clone()),
        backend: Some(registration.backend.clone()),
        host: Some(registration.host.clone()),
        port: Some(registration.port),
        process_exists: process.process_exists,
        process_start_time_matches: process.process_start_time_matches,
        actual_pid: process.actual_pid,
        actual_process_path: process.actual_process_path,
        health_ok,
        http_instance_id_matches,
        shutdown_requested,
        log_path: Some(log_path.clone()),
        log_readable: log_is_readable(Path::new(&log_path)).await,
    })
}

/// 根据观察结果构建 doctor 报告。
///
/// # 参数
/// - `observation`：统一观察结果
///
/// # 返回值
/// - `DoctorReport`：诊断报告
pub fn build_doctor_report(observation: &DaemonObservation) -> DoctorReport {
    let checks = vec![
        DoctorCheck {
            code: "registration",
            level: if observation.registration_exists {
                DoctorLevel::OK
            } else {
                DoctorLevel::FAIL
            },
            summary: "daemon 注册文件".to_string(),
            evidence: observation.registration_path.clone(),
        },
        DoctorCheck {
            code: "pid",
            level: if observation.process_exists {
                DoctorLevel::OK
            } else {
                DoctorLevel::FAIL
            },
            summary: "注册 pid 是否存活".to_string(),
            evidence: format!("registered_pid={:?}", observation.pid),
        },
        DoctorCheck {
            code: "started_at",
            level: if observation.process_start_time_matches {
                DoctorLevel::OK
            } else {
                DoctorLevel::FAIL
            },
            summary: "进程启动时间是否匹配".to_string(),
            evidence: format!("started_at={:?}", observation.started_at),
        },
        DoctorCheck {
            code: "health",
            level: if observation.health_ok {
                DoctorLevel::OK
            } else {
                DoctorLevel::FAIL
            },
            summary: "health/status 探测".to_string(),
            evidence: format!("base_url={:?}", observation.base_url),
        },
        DoctorCheck {
            code: "instance_id",
            level: match observation.http_instance_id_matches {
                Some(true) => DoctorLevel::OK,
                Some(false) => DoctorLevel::FAIL,
                None => DoctorLevel::WARN,
            },
            summary: "HTTP 实例 ID 是否与注册一致".to_string(),
            evidence: format!("instance_id={:?}", observation.instance_id),
        },
        DoctorCheck {
            code: "log_file",
            level: if observation.log_readable {
                DoctorLevel::OK
            } else {
                DoctorLevel::FAIL
            },
            summary: "日志文件是否存在且可读".to_string(),
            evidence: observation.log_path.clone().unwrap_or_default(),
        },
    ];

    let conclusion = checks
        .iter()
        .map(|check| check.level.clone())
        .max_by_key(|level| match level {
            DoctorLevel::OK => 0,
            DoctorLevel::WARN => 1,
            DoctorLevel::FAIL => 2,
        })
        .unwrap_or(DoctorLevel::OK);

    DoctorReport {
        conclusion,
        checks,
        observation: observation.clone(),
    }
}

#[derive(Debug)]
struct ProcessObservation {
    process_exists: bool,
    process_start_time_matches: bool,
    actual_pid: Option<u32>,
    actual_process_path: Option<String>,
}

async fn probe_http_status(registration: &DaemonRegistration) -> Option<DaemonStatusResponse> {
    tokio::time::timeout(
        Duration::from_millis(500),
        crate::cli::client::ApiClient::new_probe(registration.base_url.clone()).daemon_status(),
    )
    .await
    .ok()
    .and_then(Result::ok)
}

fn observe_process(registration: &DaemonRegistration) -> ProcessObservation {
    let system = System::new_all();
    let pid = Pid::from_u32(registration.pid);
    let Some(process) = system.process(pid) else {
        return ProcessObservation {
            process_exists: false,
            process_start_time_matches: false,
            actual_pid: None,
            actual_process_path: None,
        };
    };

    let started_at =
        crate::daemon::registry::started_at_text_from_unix_seconds(process.start_time()).ok();

    ProcessObservation {
        process_exists: true,
        process_start_time_matches: started_at
            .as_deref()
            .map(|value| value == registration.started_at)
            .unwrap_or(false),
        actual_pid: Some(process.pid().as_u32()),
        actual_process_path: process.exe().map(|path| path.to_string_lossy().to_string()),
    }
}

async fn log_is_readable(path: &Path) -> bool {
    tokio::fs::metadata(path)
        .await
        .map(|metadata| metadata.is_file())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests;

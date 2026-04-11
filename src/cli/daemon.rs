use crate::cli::args::DaemonCommand;
use crate::core::config::settings::Settings;
use crate::core::error::MeloResult;
use crate::daemon::manager;
use crate::daemon::observe::{DaemonObservation, DoctorReport};

/// 执行 daemon 管理子命令。
///
/// # 参数
/// - `command`：daemon 子命令；`None` 时等价于 `start`
///
/// # 返回值
/// - `MeloResult<()>`：执行结果
pub async fn run_daemon_command(command: Option<DaemonCommand>) -> MeloResult<()> {
    let settings = Settings::load()?;
    let paths = crate::daemon::registry::runtime_paths()?;

    match command {
        Some(DaemonCommand::Status { json, verbose }) => {
            let observation = crate::daemon::observe::observe_with_paths(&settings, &paths).await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&observation).unwrap());
            } else {
                println!("{}", format_status_human(&observation, verbose));
            }
        }
        Some(DaemonCommand::Logs { tail }) => {
            let output = manager::read_logs_with_paths(&paths, tail).await?;
            println!("{output}");
        }
        Some(DaemonCommand::Doctor { json }) => {
            let observation = crate::daemon::observe::observe_with_paths(&settings, &paths).await?;
            let report = crate::daemon::observe::build_doctor_report(&observation);
            if json {
                println!("{}", serde_json::to_string_pretty(&report).unwrap());
            } else {
                println!("{}", format_doctor_human(&report));
            }
        }
        Some(DaemonCommand::Ps) => {
            let observation = crate::daemon::observe::observe_with_paths(&settings, &paths).await?;
            println!("{}", format_ps_human(&observation));
        }
        Some(DaemonCommand::Start) | None => {
            let result = manager::start(&settings).await?;
            println!("action: {}", result.action);
            println!("{}", format_status_human(&result.observation, true));
        }
        Some(DaemonCommand::Stop) => {
            let result = manager::stop(&settings).await?;
            println!("action: {}", result.action);
            println!("{}", format_status_human(&result.observation, true));
        }
        Some(DaemonCommand::Restart) => {
            let result = manager::restart(&settings).await?;
            println!("action: {}", result.action);
            println!("previous_instance: {:?}", result.previous.instance_id);
            println!("{}", format_status_human(&result.current, true));
        }
        Some(DaemonCommand::Run) => unreachable!("`daemon run` 由 src/cli/run.rs 直接接管"),
    }

    Ok(())
}

/// 格式化人类可读的状态输出。
///
/// # 参数
/// - `observation`：统一观察结果
/// - `verbose`：是否显示更多字段
///
/// # 返回值
/// - `String`：渲染后的文本
pub fn format_status_human(observation: &DaemonObservation, verbose: bool) -> String {
    let mut lines = vec![
        format!("state: {:?}", observation.state),
        format!("pid: {:?}", observation.pid),
        format!("base_url: {:?}", observation.base_url),
        format!("backend: {:?}", observation.backend),
        format!("started_at: {:?}", observation.started_at),
        format!(
            "health: {}",
            if observation.health_ok {
                "healthy"
            } else {
                "unhealthy"
            }
        ),
    ];

    if verbose {
        lines.push(format!(
            "registration_path: {}",
            observation.registration_path
        ));
        lines.push(format!("instance_id: {:?}", observation.instance_id));
        lines.push(format!("process_exists: {}", observation.process_exists));
        lines.push(format!(
            "process_start_time_matches: {}",
            observation.process_start_time_matches
        ));
        lines.push(format!("log_path: {:?}", observation.log_path));
    }

    lines.join("\n")
}

/// 格式化人类可读的 doctor 输出。
///
/// # 参数
/// - `report`：诊断报告
///
/// # 返回值
/// - `String`：渲染后的文本
pub fn format_doctor_human(report: &DoctorReport) -> String {
    let mut lines = vec![format!("conclusion: {:?}", report.conclusion)];
    for check in &report.checks {
        lines.push(format!(
            "[{:?}] {}: {} ({})",
            check.level, check.code, check.summary, check.evidence
        ));
    }
    lines.join("\n")
}

/// 格式化 `daemon ps` 输出。
///
/// # 参数
/// - `observation`：统一观察结果
///
/// # 返回值
/// - `String`：渲染后的文本
pub fn format_ps_human(observation: &DaemonObservation) -> String {
    [
        format!("registered_pid: {:?}", observation.pid),
        format!("actual_pid: {:?}", observation.actual_pid),
        format!("process_path: {:?}", observation.actual_process_path),
        format!("instance_id: {:?}", observation.instance_id),
        format!(
            "process_start_time_matches: {}",
            observation.process_start_time_matches
        ),
    ]
    .join("\n")
}

#[cfg(test)]
mod tests;

#[tokio::main]
async fn main() {
    let raw_args = std::env::args_os().collect::<Vec<_>>();
    let mut prepared = melo::cli::global_flags::prepare_args(&raw_args).unwrap_or_else(|err| {
        eprintln!("{err}");
        std::process::exit(1);
    });
    let settings = melo::core::config::settings::Settings::load().unwrap_or_default();
    let component = if matches!(
        prepared.clap_args.get(1).and_then(|arg| arg.to_str()),
        Some("daemon")
    ) && matches!(
        prepared.clap_args.get(2).and_then(|arg| arg.to_str()),
        Some("run")
    ) {
        melo::core::logging::LogComponent::Daemon
    } else {
        melo::core::logging::LogComponent::Cli
    };
    if matches!(component, melo::core::logging::LogComponent::Daemon)
        && prepared.logging.daemon_log_level.is_none()
    {
        prepared.logging.daemon_log_level = std::env::var("MELO_DAEMON_LOG_LEVEL_OVERRIDE")
            .ok()
            .and_then(|value| value.parse().ok());
    }
    let command_id =
        std::env::var("MELO_COMMAND_ID").unwrap_or_else(|_| uuid::Uuid::new_v4().to_string());
    unsafe {
        std::env::set_var("MELO_COMMAND_ID", &command_id);
    }
    melo::core::logging::init_tracing(
        &settings,
        component,
        &prepared.logging,
        melo::core::logging::RuntimeLogContext {
            session_id: uuid::Uuid::new_v4().to_string(),
            command_id,
        },
    );

    if let Err(err) = melo::cli::run::run_prepared(prepared).await {
        tracing::error!(error = %err, "command_failed");
        eprintln!("{err}");
        std::process::exit(1);
    }
}

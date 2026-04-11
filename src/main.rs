use std::fs::OpenOptions;
use std::sync::Arc;

use tracing_subscriber::fmt::writer::MakeWriterExt;

/// 判断当前是否以隐藏的 daemon 子进程模式运行。
///
/// # 参数
/// - `raw_args`：命令行参数
///
/// # 返回值
/// - `bool`：是否为 `melo daemon run`
fn daemon_run_requested(raw_args: &[String]) -> bool {
    matches!(raw_args.get(1).map(String::as_str), Some("daemon"))
        && matches!(raw_args.get(2).map(String::as_str), Some("run"))
}

/// 初始化 tracing，并在隐藏 daemon 运行模式下把日志同时写到 stdout 与固定日志文件。
///
/// # 参数
/// - `raw_args`：命令行参数
///
/// # 返回值
/// - 无
fn init_tracing(raw_args: &[String]) {
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));

    if daemon_run_requested(raw_args)
        && let Ok(paths) = melo::daemon::registry::runtime_paths()
    {
        if let Some(parent) = paths.log_file.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        if let Ok(file) = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&paths.log_file)
        {
            let writer = std::io::stdout.and(Arc::new(file));
            let _ = tracing_subscriber::fmt()
                .with_env_filter(env_filter)
                .with_target(false)
                .with_writer(writer)
                .try_init();
            return;
        }
    }

    let _ = tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_target(false)
        .try_init();
}

#[tokio::main]
async fn main() {
    let raw_args = std::env::args().collect::<Vec<_>>();
    init_tracing(&raw_args);

    if let Err(err) = melo::cli::run().await {
        eprintln!("{err}");
        std::process::exit(1);
    }
}

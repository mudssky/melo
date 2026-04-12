use clap::Parser;

use crate::cli::args::{
    CliArgs, Command, DaemonCommand, DbCommand, PlayerCommand, PlayerModeCommand, PlaylistCommand,
    QueueCommand,
};
use crate::cli::observe::{ObservedDaemon, observe_read_only_daemon, print_unavailable_and_error};
use crate::core::error::MeloResult;

/// 解析命令行参数并交给后续子命令实现。
///
/// # 参数
/// - 无
///
/// # 返回
/// - `MeloResult<()>`：解析成功时返回 `Ok(())`
pub async fn run() -> MeloResult<()> {
    let raw_args = std::env::args_os().collect::<Vec<_>>();
    let prepared = crate::cli::global_flags::prepare_args(&raw_args)?;
    run_prepared(prepared).await
}

/// 执行预解析后的 CLI 参数。
///
/// # 参数
/// - `prepared`：预解析后的参数与日志覆盖项
///
/// # 返回
/// - `MeloResult<()>`：执行结果
pub async fn run_prepared(prepared: crate::cli::global_flags::PreparedArgs) -> MeloResult<()> {
    tracing::info!(target: "melo::cli::startup", "loading_settings");
    let raw_args = prepared.clap_args.clone();
    match crate::cli::dispatch::dispatch_args(&prepared.dispatch_args) {
        crate::cli::dispatch::Dispatch::DefaultLaunch => {
            let settings = crate::core::config::settings::Settings::load()?;
            let resolved_cli = crate::core::logging::resolve_logging_options(
                &settings,
                crate::core::logging::LogComponent::Cli,
                &prepared.logging,
            );
            let renderer = crate::core::runtime_templates::RuntimeTemplateRenderer::default();
            let launch_cwd = std::env::current_dir()
                .map_err(|err| crate::core::error::MeloError::Message(err.to_string()))?;
            let launch_cwd_text = launch_cwd_text(&launch_cwd);
            let (base_url, source_label, startup_notice) = {
                let _mirror = if prepared.logging.verbose {
                    Some(crate::core::logging::attach_daemon_log_mirror(
                        crate::core::logging::daemon_log_path(&settings),
                        resolved_cli.prefix_enabled,
                        settings.logging.daemon_prefix.clone(),
                    ))
                } else {
                    None
                };
                tracing::info!(target: "melo::cli::startup", "resolving_base_url");
                let ensured = crate::daemon::manager::ensure_running_with_logging(
                    &settings,
                    &prepared.logging,
                )
                .await?;
                report_daemon_override_notice(
                    &settings,
                    &prepared.logging,
                    ensured.already_running,
                );
                let base_url = ensured.base_url;
                let home = crate::cli::client::ApiClient::new(base_url.clone())
                    .tui_home()
                    .await?;
                let decision =
                    crate::cli::launch::choose_default_launch_decision(&launch_cwd, &home);
                let (source_label, startup_notice) = match decision {
                    crate::cli::launch::DefaultLaunchDecision::PreserveCurrentSession {
                        ..
                    } => (None, Some("Continuing current playback".to_string())),
                    crate::cli::launch::DefaultLaunchDecision::OpenLaunchCwd { launch_cwd } => {
                        if let Some(line) =
                            render_scan_cli_lines(&renderer, &settings, &launch_cwd).first()
                        {
                            println!("{line}");
                        }
                        let opened = crate::cli::client::ApiClient::new(base_url.clone())
                            .open_target(launch_cwd.clone(), "cwd_dir")
                            .await?;
                        if let Some(line) =
                            render_scan_cli_lines(&renderer, &settings, &opened.source_label).get(1)
                        {
                            println!("{line}");
                        }
                        (Some(opened.source_label), None)
                    }
                };
                (base_url, source_label, startup_notice)
            };

            return crate::tui::run::start(
                base_url,
                crate::tui::run::LaunchContext {
                    launch_cwd: Some(launch_cwd_text),
                    source_label,
                    startup_notice,
                    footer_hints_enabled: settings.tui.show_footer_hints,
                },
            )
            .await;
        }
        crate::cli::dispatch::Dispatch::DirectOpen(target) => {
            let settings = crate::core::config::settings::Settings::load()?;
            let resolved_cli = crate::core::logging::resolve_logging_options(
                &settings,
                crate::core::logging::LogComponent::Cli,
                &prepared.logging,
            );
            let _mirror = if prepared.logging.verbose {
                Some(crate::core::logging::attach_daemon_log_mirror(
                    crate::core::logging::daemon_log_path(&settings),
                    resolved_cli.prefix_enabled,
                    settings.logging.daemon_prefix.clone(),
                ))
            } else {
                None
            };
            tracing::info!(target: "melo::cli::startup", target = %target, "opening_explicit_target");
            let ensured =
                crate::daemon::manager::ensure_running_with_logging(&settings, &prepared.logging)
                    .await?;
            report_daemon_override_notice(&settings, &prepared.logging, ensured.already_running);
            let base_url = ensured.base_url;
            let renderer = crate::core::runtime_templates::RuntimeTemplateRenderer::default();
            let mode = if std::path::Path::new(&target).is_dir() {
                "path_dir"
            } else {
                "path_file"
            };
            if mode == "path_dir"
                && let Some(line) = render_scan_cli_lines(&renderer, &settings, &target).first()
            {
                println!("{line}");
            }
            let opened = crate::cli::client::ApiClient::new(base_url.clone())
                .open_target(target, mode)
                .await?;
            if mode == "path_dir"
                && let Some(line) =
                    render_scan_cli_lines(&renderer, &settings, &opened.source_label).get(1)
            {
                println!("{line}");
            }
            return crate::tui::run::start(
                base_url,
                crate::tui::run::LaunchContext {
                    launch_cwd: None,
                    source_label: Some(opened.source_label),
                    startup_notice: None,
                    footer_hints_enabled: settings.tui.show_footer_hints,
                },
            )
            .await;
        }
        crate::cli::dispatch::Dispatch::Clap => {}
    }

    let args = CliArgs::parse_from(raw_args);
    run_clap(args, &prepared.logging).await
}

/// 执行标准 Clap 子命令分发。
///
/// # 参数
/// - `args`：解析后的 CLI 参数
/// - `logging`：当前命令的日志覆盖项
///
/// # 返回
/// - `MeloResult<()>`：执行结果
async fn run_clap(
    args: CliArgs,
    logging: &crate::core::logging::CliLogOverrides,
) -> MeloResult<()> {
    match args.command {
        Some(Command::Status) => match observe_read_only_daemon().await? {
            ObservedDaemon::Running {
                client, docs_url, ..
            } => {
                let snapshot = client.status().await?;
                println!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::json!({
                        "snapshot": snapshot,
                        "docs": docs_url,
                    }))
                    .unwrap()
                );
            }
            ObservedDaemon::Unavailable { reason, hint } => {
                return Err(print_unavailable_and_error(&reason, &hint));
            }
        },
        Some(Command::Play) => {
            let snapshot = daemon_client_with_autostart(logging)
                .await?
                .post_json("/api/player/play")
                .await?;
            println!("{}", serde_json::to_string_pretty(&snapshot).unwrap());
        }
        Some(Command::Pause) => {
            let snapshot = daemon_client()
                .await?
                .post_json("/api/player/pause")
                .await?;
            println!("{}", serde_json::to_string_pretty(&snapshot).unwrap());
        }
        Some(Command::Toggle) => {
            let snapshot = daemon_client()
                .await?
                .post_json("/api/player/toggle")
                .await?;
            println!("{}", serde_json::to_string_pretty(&snapshot).unwrap());
        }
        Some(Command::Next) => {
            let snapshot = daemon_client().await?.post_json("/api/player/next").await?;
            println!("{}", serde_json::to_string_pretty(&snapshot).unwrap());
        }
        Some(Command::Prev) => {
            let snapshot = daemon_client().await?.post_json("/api/player/prev").await?;
            println!("{}", serde_json::to_string_pretty(&snapshot).unwrap());
        }
        Some(Command::Stop) => {
            let snapshot = daemon_client().await?.post_json("/api/player/stop").await?;
            println!("{}", serde_json::to_string_pretty(&snapshot).unwrap());
        }
        Some(Command::Tui) => {
            let settings = crate::core::config::settings::Settings::load()?;
            let ensured =
                crate::daemon::manager::ensure_running_with_logging(&settings, logging).await?;
            report_daemon_override_notice(&settings, logging, ensured.already_running);
            let base_url = ensured.base_url;
            crate::tui::run::start(
                base_url,
                crate::tui::run::LaunchContext {
                    launch_cwd: None,
                    source_label: None,
                    startup_notice: None,
                    footer_hints_enabled: settings.tui.show_footer_hints,
                },
            )
            .await?;
        }
        Some(Command::Daemon {
            command: Some(DaemonCommand::Run),
        }) => {
            run_daemon_server().await?;
        }
        Some(Command::Daemon { command }) => {
            crate::cli::daemon::run_daemon_command(command).await?;
        }
        Some(Command::Player {
            command: PlayerCommand::Volume { value },
        }) => {
            let snapshot = daemon_client().await?.player_volume(value).await?;
            println!("{}", serde_json::to_string_pretty(&snapshot).unwrap());
        }
        Some(Command::Player {
            command: PlayerCommand::Mute,
        }) => {
            let snapshot = daemon_client().await?.player_mute().await?;
            println!("{}", serde_json::to_string_pretty(&snapshot).unwrap());
        }
        Some(Command::Player {
            command: PlayerCommand::Unmute,
        }) => {
            let snapshot = daemon_client().await?.player_unmute().await?;
            println!("{}", serde_json::to_string_pretty(&snapshot).unwrap());
        }
        Some(Command::Player {
            command:
                PlayerCommand::Mode {
                    command: PlayerModeCommand::Show,
                },
        }) => match observe_read_only_daemon().await? {
            ObservedDaemon::Running { client, .. } => {
                let snapshot = client.player_mode_show().await?;
                println!("{}", serde_json::to_string_pretty(&snapshot).unwrap());
            }
            ObservedDaemon::Unavailable { reason, hint } => {
                return Err(print_unavailable_and_error(&reason, &hint));
            }
        },
        Some(Command::Player {
            command:
                PlayerCommand::Mode {
                    command: PlayerModeCommand::Repeat { mode },
                },
        }) => {
            let snapshot = daemon_client().await?.player_mode_repeat(&mode).await?;
            println!("{}", serde_json::to_string_pretty(&snapshot).unwrap());
        }
        Some(Command::Player {
            command:
                PlayerCommand::Mode {
                    command: PlayerModeCommand::Shuffle { enabled },
                },
        }) => {
            let snapshot = daemon_client()
                .await?
                .player_mode_shuffle(parse_bool_flag(&enabled))
                .await?;
            println!("{}", serde_json::to_string_pretty(&snapshot).unwrap());
        }
        Some(Command::Queue {
            command: QueueCommand::Show,
        }) => match observe_read_only_daemon().await? {
            ObservedDaemon::Running { client, .. } => {
                let snapshot = client.queue_show().await?;
                println!("{}", serde_json::to_string_pretty(&snapshot).unwrap());
            }
            ObservedDaemon::Unavailable { reason, hint } => {
                return Err(print_unavailable_and_error(&reason, &hint));
            }
        },
        Some(Command::Queue {
            command: QueueCommand::Remove { index },
        }) => {
            let snapshot = daemon_client().await?.queue_remove(index).await?;
            println!("{}", serde_json::to_string_pretty(&snapshot).unwrap());
        }
        Some(Command::Queue {
            command: QueueCommand::Move { from, to },
        }) => {
            let snapshot = daemon_client().await?.queue_move(from, to).await?;
            println!("{}", serde_json::to_string_pretty(&snapshot).unwrap());
        }
        Some(Command::Queue {
            command: QueueCommand::Clear,
        }) => {
            let snapshot = daemon_client().await?.queue_clear().await?;
            println!("{}", serde_json::to_string_pretty(&snapshot).unwrap());
        }
        Some(Command::Queue {
            command: QueueCommand::Play { index },
        }) => {
            let snapshot = daemon_client().await?.queue_play_index(index).await?;
            println!("{}", serde_json::to_string_pretty(&snapshot).unwrap());
        }
        Some(Command::Playlist {
            command:
                PlaylistCommand::Promote {
                    source_key,
                    new_name,
                },
        }) => {
            let settings = crate::core::config::settings::Settings::load()?;
            crate::domain::playlist::service::PlaylistService::new(settings)
                .promote_ephemeral(&source_key, &new_name)
                .await?;
            println!("{new_name}");
        }
        Some(Command::Playlist {
            command: PlaylistCommand::Cleanup { expired },
        }) => {
            if expired {
                let settings = crate::core::config::settings::Settings::load()?;
                let deleted = crate::domain::playlist::service::PlaylistService::new(settings)
                    .cleanup_expired(&crate::core::db::now_text())
                    .await?;
                println!("{deleted}");
            } else {
                println!("0");
            }
        }
        Some(Command::Db {
            command: DbCommand::Path,
        }) => {
            let settings = crate::core::config::settings::Settings::load()?;
            println!("{}", settings.database.path);
        }
        Some(Command::Db {
            command: DbCommand::Vacuum,
        }) => {
            let settings = crate::core::config::settings::Settings::load()?;
            crate::core::db::maintenance::vacuum(settings.database.path.as_std_path()).await?;
        }
        Some(Command::Db {
            command: DbCommand::Backup { dest },
        }) => {
            let settings = crate::core::config::settings::Settings::load()?;
            let dest = dest.unwrap_or_else(|| format!("{}.backup", settings.database.path));
            crate::core::db::maintenance::backup(
                settings.database.path.as_std_path(),
                std::path::Path::new(&dest),
            )?;
            println!("{dest}");
        }
        _ => {}
    }

    Ok(())
}

/// 生成目录扫描启动阶段要输出给 CLI 的提示行。
///
/// # 参数
/// - `renderer`：运行时模板渲染器
/// - `settings`：全局配置
/// - `source_label`：当前扫描来源标签
///
/// # 返回值
/// - `Vec<String>`：按顺序返回启动提示与切入 TUI 提示
fn render_scan_cli_lines(
    renderer: &crate::core::runtime_templates::RuntimeTemplateRenderer,
    settings: &crate::core::config::settings::Settings,
    source_label: &str,
) -> Vec<String> {
    vec![
        renderer.render(
            settings,
            crate::core::runtime_templates::RuntimeTemplateKey::CliScanStart,
            serde_json::json!({ "source_label": source_label }),
        ),
        renderer.render(
            settings,
            crate::core::runtime_templates::RuntimeTemplateKey::CliScanHandoff,
            serde_json::json!({ "source_label": source_label }),
        ),
    ]
}

/// 将启动目录格式化为稳定的文本表示。
///
/// # 参数
/// - `path`：运行时捕获到的当前目录
///
/// # 返回值
/// - `String`：可传给 TUI 与测试断言的目录文本
pub(crate) fn launch_cwd_text(path: &std::path::Path) -> String {
    path.to_string_lossy().into_owned()
}

/// 构造一个基于发现逻辑的 daemon 客户端。
///
/// # 参数
/// - 无
///
/// # 返回值
/// - `MeloResult<crate::cli::client::ApiClient>`：发现后的客户端
async fn daemon_client() -> MeloResult<crate::cli::client::ApiClient> {
    let settings = crate::core::config::settings::Settings::load()?;
    crate::cli::client::ApiClient::from_discovery(&settings).await
}

/// 构造一个允许自动拉起 daemon 的客户端。
///
/// # 参数
/// - `logging`：当前命令的日志覆盖项
///
/// # 返回值
/// - `MeloResult<crate::cli::client::ApiClient>`：自动拉起后的客户端
async fn daemon_client_with_autostart(
    logging: &crate::core::logging::CliLogOverrides,
) -> MeloResult<crate::cli::client::ApiClient> {
    let settings = crate::core::config::settings::Settings::load()?;
    let ensured = crate::daemon::manager::ensure_running_with_logging(&settings, logging).await?;
    report_daemon_override_notice(&settings, logging, ensured.already_running);
    Ok(crate::cli::client::ApiClient::new(ensured.base_url))
}

/// 在当前命令链路里输出 daemon 覆盖范围受限提示。
///
/// # 参数
/// - `settings`：全局配置
/// - `logging`：当前命令的日志覆盖项
/// - `daemon_already_running`：当前命令是否复用了已有 daemon
///
/// # 返回值
/// - 无
fn report_daemon_override_notice(
    settings: &crate::core::config::settings::Settings,
    logging: &crate::core::logging::CliLogOverrides,
    daemon_already_running: bool,
) {
    if let Some(notice) =
        crate::core::logging::daemon_override_notice(settings, logging, daemon_already_running)
    {
        tracing::warn!(target: "melo::cli::startup", "{notice}");
    }
}

/// 以前台模式运行 daemon 服务端。
///
/// # 参数
/// - 无
///
/// # 返回值
/// - `MeloResult<()>`：运行结果
async fn run_daemon_server() -> MeloResult<()> {
    let settings = crate::core::config::settings::Settings::load()?;
    let bind_addr = if let Ok(base_url) = std::env::var("MELO_BASE_URL") {
        crate::daemon::process::daemon_bind_addr(&base_url)?
    } else {
        crate::daemon::process::next_bind_addr(
            &settings.daemon.host,
            settings.daemon.base_port,
            settings.daemon.port_search_limit,
        )
        .await?
    };
    let listener = tokio::net::TcpListener::bind(bind_addr)
        .await
        .map_err(|err| crate::core::error::MeloError::Message(err.to_string()))?;
    let listener_addr = listener
        .local_addr()
        .map_err(|err| crate::core::error::MeloError::Message(err.to_string()))?;
    let state = crate::daemon::app::AppState::new().await?;
    let shutdown_state = state.clone();
    crate::daemon::registry::store_registration(&state.daemon_registration(listener_addr)).await?;
    let serve_result = axum::serve(listener, crate::daemon::server::router(state))
        .with_graceful_shutdown(async move {
            shutdown_state.wait_for_shutdown().await;
        })
        .await
        .map_err(|err| crate::core::error::MeloError::Message(err.to_string()));
    let clear_result = crate::daemon::registry::clear_registration().await;
    serve_result?;
    clear_result?;
    Ok(())
}

/// 解析 CLI 中的布尔开关值。
///
/// # 参数
/// - `value`：原始字符串
///
/// # 返回
/// - `bool`：解析后的布尔值
fn parse_bool_flag(value: &str) -> bool {
    matches!(value, "1" | "true" | "on" | "yes")
}

#[cfg(test)]
mod tests;

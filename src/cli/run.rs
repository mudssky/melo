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
    match crate::cli::dispatch::dispatch_args(&raw_args) {
        crate::cli::dispatch::Dispatch::DefaultLaunch => {
            let settings = crate::core::config::settings::Settings::load()?;
            let base_url = crate::daemon::process::ensure_running(&settings).await?;
            let renderer = crate::core::runtime_templates::RuntimeTemplateRenderer::default();
            let (source_label, startup_notice) = if settings.open.scan_current_dir {
                let cwd = std::env::current_dir()
                    .map_err(|err| crate::core::error::MeloError::Message(err.to_string()))?;
                if let Some(line) =
                    render_scan_cli_lines(&renderer, &settings, &cwd.to_string_lossy()).first()
                {
                    println!("{line}");
                }
                match crate::cli::client::ApiClient::new(base_url.clone())
                    .open_target(cwd.to_string_lossy().into_owned(), "cwd_dir")
                    .await
                {
                    Ok(opened) => {
                        if let Some(line) =
                            render_scan_cli_lines(&renderer, &settings, &opened.source_label).get(1)
                        {
                            println!("{line}");
                        }
                        (Some(opened.source_label), None)
                    }
                    Err(err) => (None, Some(err.to_string())),
                }
            } else {
                (None, None)
            };

            return crate::tui::run::start(
                base_url,
                crate::tui::run::LaunchContext {
                    source_label,
                    startup_notice,
                    footer_hints_enabled: settings.tui.show_footer_hints,
                },
            )
            .await;
        }
        crate::cli::dispatch::Dispatch::DirectOpen(target) => {
            let settings = crate::core::config::settings::Settings::load()?;
            let base_url = crate::daemon::process::ensure_running(&settings).await?;
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
    run_clap(args).await
}

/// 执行标准 Clap 子命令分发。
///
/// # 参数
/// - `args`：解析后的 CLI 参数
///
/// # 返回
/// - `MeloResult<()>`：执行结果
async fn run_clap(args: CliArgs) -> MeloResult<()> {
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
            let snapshot = daemon_client_with_autostart()
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
            let base_url = crate::daemon::process::ensure_running(&settings).await?;
            crate::tui::run::start(
                base_url,
                crate::tui::run::LaunchContext {
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
/// - 无
///
/// # 返回值
/// - `MeloResult<crate::cli::client::ApiClient>`：自动拉起后的客户端
async fn daemon_client_with_autostart() -> MeloResult<crate::cli::client::ApiClient> {
    if let Ok(base_url) = std::env::var("MELO_BASE_URL") {
        let client = crate::cli::client::ApiClient::new(base_url.clone());
        if client.health().await.is_ok() {
            return Ok(client);
        }
    }

    let settings = crate::core::config::settings::Settings::load()?;
    let base_url = crate::daemon::manager::ensure_running(&settings).await?;
    Ok(crate::cli::client::ApiClient::new(base_url))
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

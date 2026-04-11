use clap::Parser;

use crate::cli::args::{
    CliArgs, Command, DbCommand, PlayerCommand, PlayerModeCommand, PlaylistCommand, QueueCommand,
};
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
            let (source_label, startup_notice) = if settings.open.scan_current_dir {
                let cwd = std::env::current_dir()
                    .map_err(|err| crate::core::error::MeloError::Message(err.to_string()))?;
                match crate::cli::client::ApiClient::new(base_url.clone())
                    .open_target(cwd.to_string_lossy().into_owned(), "cwd_dir")
                    .await
                {
                    Ok(opened) => (Some(opened.source_label), None),
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
            let mode = if std::path::Path::new(&target).is_dir() {
                "path_dir"
            } else {
                "path_file"
            };
            let opened = crate::cli::client::ApiClient::new(base_url.clone())
                .open_target(target, mode)
                .await?;
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
        Some(Command::Status) => {
            let client = daemon_client().await?;
            let snapshot = client.status().await?;
            println!("{}", serde_json::to_string_pretty(&snapshot).unwrap());
        }
        Some(Command::Play) => {
            let snapshot = daemon_client().await?.post_json("/api/player/play").await?;
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
        Some(Command::Daemon) => {
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
            let state = crate::daemon::app::AppState::new()?;
            let backend_name = state.player.snapshot().await.backend_name;
            crate::daemon::registry::store_registration(
                &crate::daemon::registry::DaemonRegistration {
                    base_url: format!("http://{listener_addr}"),
                    pid: std::process::id(),
                    started_at: daemon_started_at_text(),
                    version: env!("CARGO_PKG_VERSION").to_string(),
                    backend: backend_name,
                    host: listener_addr.ip().to_string(),
                    port: listener_addr.port(),
                },
            )
            .await?;
            let serve_result = axum::serve(listener, crate::daemon::server::router(state))
                .await
                .map_err(|err| crate::core::error::MeloError::Message(err.to_string()));
            let clear_result = crate::daemon::registry::clear_registration().await;
            serve_result?;
            clear_result?;
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
        }) => {
            let snapshot = daemon_client().await?.player_mode_show().await?;
            println!("{}", serde_json::to_string_pretty(&snapshot).unwrap());
        }
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
        }) => {
            let snapshot = daemon_client().await?.queue_show().await?;
            println!("{}", serde_json::to_string_pretty(&snapshot).unwrap());
        }
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

/// 生成 daemon 注册信息里的启动时间字符串。
///
/// # 参数
/// - 无
///
/// # 返回值
/// - `String`：当前 UTC 秒级时间戳字符串
fn daemon_started_at_text() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs().to_string())
        .unwrap_or_else(|_| "0".to_string())
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

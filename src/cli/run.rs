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
            let base_url = daemon_base_url();
            crate::daemon::process::ensure_running(&base_url).await?;

            let settings = crate::core::config::settings::Settings::load()?;
            let source_label = if settings.open.scan_current_dir {
                let cwd = std::env::current_dir()
                    .map_err(|err| crate::core::error::MeloError::Message(err.to_string()))?;
                crate::cli::client::ApiClient::new(base_url.clone())
                    .open_target(cwd.to_string_lossy().into_owned(), "cwd_dir")
                    .await
                    .ok()
                    .map(|opened| opened.source_label)
            } else {
                None
            };

            return crate::tui::run::start(base_url, source_label).await;
        }
        crate::cli::dispatch::Dispatch::DirectOpen(target) => {
            let base_url = daemon_base_url();
            crate::daemon::process::ensure_running(&base_url).await?;
            let mode = if std::path::Path::new(&target).is_dir() {
                "path_dir"
            } else {
                "path_file"
            };
            let opened = crate::cli::client::ApiClient::new(base_url.clone())
                .open_target(target, mode)
                .await?;
            return crate::tui::run::start(base_url, Some(opened.source_label)).await;
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
            let client = crate::cli::client::ApiClient::from_env();
            let snapshot = client.status().await?;
            println!("{}", serde_json::to_string_pretty(&snapshot).unwrap());
        }
        Some(Command::Play) => {
            let snapshot = crate::cli::client::ApiClient::from_env()
                .post_json("/api/player/play")
                .await?;
            println!("{}", serde_json::to_string_pretty(&snapshot).unwrap());
        }
        Some(Command::Pause) => {
            let snapshot = crate::cli::client::ApiClient::from_env()
                .post_json("/api/player/pause")
                .await?;
            println!("{}", serde_json::to_string_pretty(&snapshot).unwrap());
        }
        Some(Command::Toggle) => {
            let snapshot = crate::cli::client::ApiClient::from_env()
                .post_json("/api/player/toggle")
                .await?;
            println!("{}", serde_json::to_string_pretty(&snapshot).unwrap());
        }
        Some(Command::Next) => {
            let snapshot = crate::cli::client::ApiClient::from_env()
                .post_json("/api/player/next")
                .await?;
            println!("{}", serde_json::to_string_pretty(&snapshot).unwrap());
        }
        Some(Command::Prev) => {
            let snapshot = crate::cli::client::ApiClient::from_env()
                .post_json("/api/player/prev")
                .await?;
            println!("{}", serde_json::to_string_pretty(&snapshot).unwrap());
        }
        Some(Command::Stop) => {
            let snapshot = crate::cli::client::ApiClient::from_env()
                .post_json("/api/player/stop")
                .await?;
            println!("{}", serde_json::to_string_pretty(&snapshot).unwrap());
        }
        Some(Command::Tui) => {
            let base_url = daemon_base_url();
            crate::daemon::process::ensure_running(&base_url).await?;
            crate::tui::run::start(base_url, None).await?;
        }
        Some(Command::Daemon) => {
            let base_url = daemon_base_url();
            let listener =
                tokio::net::TcpListener::bind(crate::daemon::process::daemon_bind_addr(&base_url)?)
                    .await
                    .map_err(|err| crate::core::error::MeloError::Message(err.to_string()))?;
            let state = crate::daemon::app::AppState::new()?;
            axum::serve(listener, crate::daemon::server::router(state))
                .await
                .map_err(|err| crate::core::error::MeloError::Message(err.to_string()))?;
        }
        Some(Command::Player {
            command: PlayerCommand::Volume { value },
        }) => {
            let snapshot = crate::cli::client::ApiClient::from_env()
                .player_volume(value)
                .await?;
            println!("{}", serde_json::to_string_pretty(&snapshot).unwrap());
        }
        Some(Command::Player {
            command: PlayerCommand::Mute,
        }) => {
            let snapshot = crate::cli::client::ApiClient::from_env()
                .player_mute()
                .await?;
            println!("{}", serde_json::to_string_pretty(&snapshot).unwrap());
        }
        Some(Command::Player {
            command: PlayerCommand::Unmute,
        }) => {
            let snapshot = crate::cli::client::ApiClient::from_env()
                .player_unmute()
                .await?;
            println!("{}", serde_json::to_string_pretty(&snapshot).unwrap());
        }
        Some(Command::Player {
            command:
                PlayerCommand::Mode {
                    command: PlayerModeCommand::Show,
                },
        }) => {
            let snapshot = crate::cli::client::ApiClient::from_env()
                .player_mode_show()
                .await?;
            println!("{}", serde_json::to_string_pretty(&snapshot).unwrap());
        }
        Some(Command::Player {
            command:
                PlayerCommand::Mode {
                    command: PlayerModeCommand::Repeat { mode },
                },
        }) => {
            let snapshot = crate::cli::client::ApiClient::from_env()
                .player_mode_repeat(&mode)
                .await?;
            println!("{}", serde_json::to_string_pretty(&snapshot).unwrap());
        }
        Some(Command::Player {
            command:
                PlayerCommand::Mode {
                    command: PlayerModeCommand::Shuffle { enabled },
                },
        }) => {
            let snapshot = crate::cli::client::ApiClient::from_env()
                .player_mode_shuffle(parse_bool_flag(&enabled))
                .await?;
            println!("{}", serde_json::to_string_pretty(&snapshot).unwrap());
        }
        Some(Command::Queue {
            command: QueueCommand::Show,
        }) => {
            let snapshot = crate::cli::client::ApiClient::from_env()
                .queue_show()
                .await?;
            println!("{}", serde_json::to_string_pretty(&snapshot).unwrap());
        }
        Some(Command::Queue {
            command: QueueCommand::Remove { index },
        }) => {
            let snapshot = crate::cli::client::ApiClient::from_env()
                .queue_remove(index)
                .await?;
            println!("{}", serde_json::to_string_pretty(&snapshot).unwrap());
        }
        Some(Command::Queue {
            command: QueueCommand::Move { from, to },
        }) => {
            let snapshot = crate::cli::client::ApiClient::from_env()
                .queue_move(from, to)
                .await?;
            println!("{}", serde_json::to_string_pretty(&snapshot).unwrap());
        }
        Some(Command::Queue {
            command: QueueCommand::Clear,
        }) => {
            let snapshot = crate::cli::client::ApiClient::from_env()
                .queue_clear()
                .await?;
            println!("{}", serde_json::to_string_pretty(&snapshot).unwrap());
        }
        Some(Command::Queue {
            command: QueueCommand::Play { index },
        }) => {
            let snapshot = crate::cli::client::ApiClient::from_env()
                .queue_play_index(index)
                .await?;
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

/// 返回 daemon HTTP 基地址。
///
/// # 参数
/// - 无
///
/// # 返回
/// - `String`：daemon HTTP 基地址
fn daemon_base_url() -> String {
    std::env::var("MELO_BASE_URL").unwrap_or_else(|_| "http://127.0.0.1:8080".to_string())
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

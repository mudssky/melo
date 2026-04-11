use clap::Parser;

use crate::cli::args::{
    CliArgs, Command, DbCommand, PlayerCommand, PlayerModeCommand, QueueCommand,
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
    let args = CliArgs::parse();

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
        Some(Command::Daemon) => {
            let base_url = std::env::var("MELO_BASE_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:8080".to_string());
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

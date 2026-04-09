use clap::Parser;

use crate::cli::args::{CliArgs, Command, DbCommand, QueueCommand};
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

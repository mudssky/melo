use clap::Parser;

use crate::cli::args::{CliArgs, Command, DbCommand};
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
            crate::cli::client::ApiClient::from_env()
                .post_no_body("/api/player/play")
                .await?;
        }
        Some(Command::Pause) => {
            crate::cli::client::ApiClient::from_env()
                .post_no_body("/api/player/pause")
                .await?;
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
            crate::core::db::maintenance::vacuum(settings.database.path.as_std_path())?;
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

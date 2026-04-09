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
pub fn run() -> MeloResult<()> {
    let args = CliArgs::parse();

    if matches!(
        args.command,
        Some(Command::Db {
            command: DbCommand::Path,
        })
    ) {
        let settings = crate::core::config::settings::Settings::load()?;
        println!("{}", settings.database.path);
    }

    Ok(())
}

use clap::Parser;

use crate::cli::args::CliArgs;
use crate::core::error::MeloResult;

/// 解析命令行参数并交给后续子命令实现。
///
/// # 参数
/// - 无
///
/// # 返回
/// - `MeloResult<()>`：解析成功时返回 `Ok(())`
pub fn run() -> MeloResult<()> {
    let _ = CliArgs::parse();
    Ok(())
}

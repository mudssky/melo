use clap::{Parser, Subcommand};

/// Melo 顶层命令行参数。
#[derive(Debug, Parser)]
#[command(
    name = "melo",
    version,
    about = "Local music library manager and daemon player"
)]
pub struct CliArgs {
    /// 顶层子命令。
    #[command(subcommand)]
    pub command: Option<Command>,
}

/// Melo 第一层命令定义。
#[derive(Debug, Subcommand)]
pub enum Command {
    Play,
    Pause,
    Toggle,
    Next,
    Prev,
    Stop,
    Status,
    Tui,
    Daemon,
    Library,
    Queue,
    Playlist,
    Db {
        #[command(subcommand)]
        command: DbCommand,
    },
    Config,
}

/// 数据库维护子命令。
#[derive(Debug, Subcommand)]
pub enum DbCommand {
    Path,
    Init,
    Migrate,
    Doctor,
    Vacuum,
    Backup { dest: Option<String> },
}

use clap::{Parser, Subcommand};

/// Melo 顶层命令行参数。
#[derive(Debug, Parser)]
#[command(
    name = "melo",
    version,
    about = "Daemon-backed local music library manager",
    long_about = "Daemon-backed local music library manager for scanning, organizing, querying, and remotely controlling local playback.",
    after_help = "Examples:\n  melo daemon\n  melo status\n  melo library scan D:/Music\n  melo playlist preview aimer"
)]
pub struct CliArgs {
    /// 顶层子命令。
    #[command(subcommand)]
    pub command: Option<Command>,
}

/// 播放队列相关子命令。
#[derive(Debug, Subcommand)]
pub enum QueueCommand {
    #[command(about = "Print the current queue snapshot")]
    Show,
    #[command(about = "Remove a queue item by index")]
    Remove { index: usize },
    #[command(about = "Move a queue item to a new index")]
    Move { from: usize, to: usize },
    #[command(about = "Clear the entire queue")]
    Clear,
    #[command(about = "Play a queue item by index")]
    Play { index: usize },
}

/// Melo 第一层命令定义。
#[derive(Debug, Subcommand)]
pub enum Command {
    #[command(about = "Ask the daemon to start playback")]
    Play,
    #[command(about = "Ask the daemon to pause playback")]
    Pause,
    #[command(about = "Toggle the current playback state")]
    Toggle,
    #[command(about = "Skip to the next queue item")]
    Next,
    #[command(about = "Return to the previous queue item")]
    Prev,
    #[command(about = "Stop the active playback session")]
    Stop,
    #[command(about = "Fetch the current player snapshot from the daemon")]
    Status,
    #[command(about = "Launch the terminal UI client")]
    Tui,
    #[command(about = "Run or manage the Melo daemon")]
    Daemon,
    #[command(
        about = "Scan, inspect, and organize library content",
        long_about = "Scan, inspect, and organize library content without mixing one-off ingest operations with file organization previews.",
        after_help = "Examples:\n  melo library scan D:/Music\n  melo library organize --preview"
    )]
    Library,
    #[command(
        about = "Inspect or manipulate the in-memory playback queue",
        long_about = "Inspect or manipulate the in-memory playback queue exposed by the daemon."
    )]
    Queue {
        #[command(subcommand)]
        command: QueueCommand,
    },
    #[command(
        about = "Manage static playlists and preview smart playlist results",
        long_about = "Manage static playlists and preview smart playlist results so users can distinguish saved membership changes from query-only previews.",
        after_help = "Examples:\n  melo playlist add Favorites 12 42\n  melo playlist preview aimer"
    )]
    Playlist,
    #[command(
        about = "Inspect and maintain the Melo database",
        long_about = "Inspect and maintain the Melo database, including path discovery, health checks, backups, and low-level maintenance tasks.",
        after_help = "Examples:\n  melo db doctor\n  melo db backup ./backup/melo.db"
    )]
    Db {
        #[command(subcommand)]
        command: DbCommand,
    },
    #[command(about = "Print or inspect effective configuration")]
    Config,
}

/// 数据库维护子命令。
#[derive(Debug, Subcommand)]
pub enum DbCommand {
    #[command(about = "Print the current SQLite database path")]
    Path,
    #[command(about = "Initialize the database schema")]
    Init,
    #[command(about = "Apply pending database migrations")]
    Migrate,
    #[command(about = "Run database health checks")]
    Doctor,
    #[command(about = "Run SQLite VACUUM")]
    Vacuum,
    #[command(about = "Create a database backup at the given destination")]
    Backup { dest: Option<String> },
}

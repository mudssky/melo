use clap::{Parser, Subcommand};

/// Melo 顶层命令行参数。
#[derive(Debug, Parser)]
#[command(
    name = "melo",
    version,
    about = "Daemon-backed local music library manager",
    long_about = "Daemon-backed local music library manager for scanning, organizing, querying, and remotely controlling local playback.",
    after_help = "Examples:\n  melo\n  melo D:/Music\n  melo daemon\n  melo status\n  melo playlist cleanup"
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

/// 播放模式相关子命令。
#[derive(Debug, Subcommand)]
pub enum PlayerModeCommand {
    #[command(about = "Print the current repeat/shuffle mode snapshot")]
    Show,
    #[command(about = "Set the repeat mode to off/one/all")]
    Repeat { mode: String },
    #[command(about = "Enable or disable shuffle with true/false or on/off")]
    Shuffle { enabled: String },
}

/// 播放器控制相关子命令。
#[derive(Debug, Subcommand)]
pub enum PlayerCommand {
    #[command(about = "Set the player volume percentage")]
    Volume { value: u8 },
    #[command(about = "Mute the player")]
    Mute,
    #[command(about = "Unmute the player")]
    Unmute,
    #[command(about = "Inspect or update repeat/shuffle mode")]
    Mode {
        #[command(subcommand)]
        command: PlayerModeCommand,
    },
}

/// 播放列表维护子命令。
#[derive(Debug, Subcommand)]
pub enum PlaylistCommand {
    #[command(about = "Promote an ephemeral playlist into a visible static playlist")]
    Promote {
        source_key: String,
        new_name: String,
    },
    #[command(about = "Delete expired ephemeral playlists")]
    Cleanup {
        #[arg(long, default_value_t = true)]
        expired: bool,
    },
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
        about = "Inspect or control advanced player state",
        long_about = "Inspect or control advanced player state such as volume, mute, repeat, and shuffle."
    )]
    Player {
        #[command(subcommand)]
        command: PlayerCommand,
    },
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
        about = "Maintain direct-open ephemeral playlists",
        long_about = "Maintain direct-open ephemeral playlists, including promotion into visible static playlists and cleanup of expired ephemeral records.",
        after_help = "Examples:\n  melo playlist promote D:/Music/blue-bird.mp3 Favorites\n  melo playlist cleanup"
    )]
    Playlist {
        #[command(subcommand)]
        command: PlaylistCommand,
    },
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

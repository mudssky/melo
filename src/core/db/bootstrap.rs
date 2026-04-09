use std::fs;
use std::path::PathBuf;

use rusqlite::Connection;

use crate::core::config::settings::Settings;
use crate::core::error::{MeloError, MeloResult};

/// 数据库初始化器，负责创建一期所需的基础表结构。
pub struct DatabaseBootstrap<'a> {
    settings: &'a Settings,
}

impl<'a> DatabaseBootstrap<'a> {
    /// 创建新的数据库初始化器。
    ///
    /// # 参数
    /// - `settings`：全局配置
    ///
    /// # 返回
    /// - `Self`：数据库初始化器
    pub fn new(settings: &'a Settings) -> Self {
        Self { settings }
    }

    /// 初始化数据库表结构。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回
    /// - `MeloResult<()>`：初始化结果
    pub async fn init(&self) -> MeloResult<()> {
        let path = self.settings.database.path.as_std_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|err| MeloError::Message(err.to_string()))?;
        }

        let conn = Connection::open(path).map_err(|err| MeloError::Message(err.to_string()))?;
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS migrations (
                name TEXT PRIMARY KEY
            );

            CREATE TABLE IF NOT EXISTS artists (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                sort_name TEXT,
                search_name TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS albums (
                id INTEGER PRIMARY KEY,
                title TEXT NOT NULL,
                album_artist_id INTEGER,
                year INTEGER,
                source_dir TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS songs (
                id INTEGER PRIMARY KEY,
                path TEXT NOT NULL UNIQUE,
                title TEXT NOT NULL,
                artist_id INTEGER,
                album_id INTEGER,
                track_no INTEGER,
                disc_no INTEGER,
                duration_seconds REAL,
                genre TEXT,
                lyrics TEXT,
                lyrics_source_kind TEXT NOT NULL DEFAULT 'none',
                lyrics_source_path TEXT,
                lyrics_format TEXT,
                lyrics_updated_at TEXT,
                format TEXT,
                bitrate INTEGER,
                sample_rate INTEGER,
                bit_depth INTEGER,
                channels INTEGER,
                file_size INTEGER NOT NULL,
                file_mtime INTEGER NOT NULL,
                added_at TEXT NOT NULL,
                scanned_at TEXT NOT NULL,
                organized_at TEXT,
                last_organize_rule TEXT,
                updated_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS playlists (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL UNIQUE,
                description TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS playlist_entries (
                id INTEGER PRIMARY KEY,
                playlist_id INTEGER NOT NULL,
                song_id INTEGER NOT NULL,
                position INTEGER NOT NULL,
                added_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS artwork_refs (
                id INTEGER PRIMARY KEY,
                owner_kind TEXT NOT NULL,
                owner_id INTEGER NOT NULL,
                source_kind TEXT NOT NULL,
                source_path TEXT,
                embedded_song_id INTEGER,
                mime TEXT,
                cache_path TEXT,
                hash TEXT,
                updated_at TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_songs_artist_id ON songs(artist_id);
            CREATE INDEX IF NOT EXISTS idx_songs_album_track ON songs(album_id, disc_no, track_no);
            CREATE INDEX IF NOT EXISTS idx_artists_search_name ON artists(search_name);
            CREATE INDEX IF NOT EXISTS idx_albums_artist_title ON albums(album_artist_id, title);
            CREATE INDEX IF NOT EXISTS idx_playlist_entries_position ON playlist_entries(playlist_id, position);
            "#,
        )
        .map_err(|err| MeloError::Message(err.to_string()))?;

        Ok(())
    }

    /// 读取当前数据库中的表名列表。
    ///
    /// # 参数
    /// - `path`：数据库路径
    ///
    /// # 返回
    /// - `rusqlite::Result<Vec<String>>`：排序后的表名列表
    pub fn table_names(path: PathBuf) -> rusqlite::Result<Vec<String>> {
        let conn = Connection::open(path)?;
        let mut stmt =
            conn.prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        rows.collect()
    }
}

use crate::core::config::settings::Settings;
use crate::core::db::connection::connect;
use crate::core::error::{MeloError, MeloResult};

/// 静态歌单摘要。
#[derive(Debug, Clone)]
pub struct StaticPlaylistSummary {
    /// 歌单名称。
    pub name: String,
    /// 歌单内歌曲数量。
    pub count: usize,
}

/// 静态歌单仓储。
pub struct PlaylistRepository {
    settings: Settings,
}

impl PlaylistRepository {
    /// 创建新的静态歌单仓储。
    ///
    /// # 参数
    /// - `settings`：全局配置
    ///
    /// # 返回
    /// - `Self`：仓储对象
    pub fn new(settings: Settings) -> Self {
        Self { settings }
    }

    /// 创建静态歌单。
    ///
    /// # 参数
    /// - `name`：歌单名
    /// - `description`：可选描述
    ///
    /// # 返回
    /// - `MeloResult<()>`：创建结果
    pub async fn create_static(&self, name: &str, description: Option<&str>) -> MeloResult<()> {
        let conn = connect(&self.settings)?;
        conn.execute(
            "INSERT INTO playlists (name, description, created_at, updated_at) VALUES (?1, ?2, datetime('now'), datetime('now'))",
            rusqlite::params![name, description],
        )
        .map_err(|err| MeloError::Message(err.to_string()))?;
        Ok(())
    }

    /// 向静态歌单添加歌曲。
    ///
    /// # 参数
    /// - `name`：歌单名
    /// - `song_ids`：歌曲 ID 列表
    ///
    /// # 返回
    /// - `MeloResult<()>`：写入结果
    pub async fn add_songs(&self, name: &str, song_ids: &[i64]) -> MeloResult<()> {
        let conn = connect(&self.settings)?;
        let playlist_id: i64 = conn
            .query_row("SELECT id FROM playlists WHERE name = ?1", [name], |row| {
                row.get(0)
            })
            .map_err(|err| MeloError::Message(err.to_string()))?;

        let current_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM playlist_entries WHERE playlist_id = ?1",
                [playlist_id],
                |row| row.get(0),
            )
            .map_err(|err| MeloError::Message(err.to_string()))?;

        for (offset, song_id) in song_ids.iter().enumerate() {
            conn.execute(
                "INSERT INTO playlist_entries (playlist_id, song_id, position, added_at) VALUES (?1, ?2, ?3, datetime('now'))",
                rusqlite::params![playlist_id, song_id, current_count + offset as i64],
            )
            .map_err(|err| MeloError::Message(err.to_string()))?;
        }

        Ok(())
    }

    /// 列出静态歌单摘要。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回
    /// - `MeloResult<Vec<StaticPlaylistSummary>>`：静态歌单摘要列表
    pub async fn list_static(&self) -> MeloResult<Vec<StaticPlaylistSummary>> {
        let conn = connect(&self.settings)?;
        let mut stmt = conn
            .prepare(
                "SELECT playlists.name, COUNT(playlist_entries.id)
                 FROM playlists
                 LEFT JOIN playlist_entries ON playlist_entries.playlist_id = playlists.id
                 GROUP BY playlists.id
                 ORDER BY playlists.name ASC",
            )
            .map_err(|err| MeloError::Message(err.to_string()))?;

        let rows = stmt
            .query_map([], |row| {
                Ok(StaticPlaylistSummary {
                    name: row.get(0)?,
                    count: row.get::<_, i64>(1)? as usize,
                })
            })
            .map_err(|err| MeloError::Message(err.to_string()))?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|err| MeloError::Message(err.to_string()))
    }

    /// 预览静态歌单中的歌曲。
    ///
    /// # 参数
    /// - `name`：歌单名
    ///
    /// # 返回
    /// - `MeloResult<Vec<SongRecord>>`：歌单歌曲列表
    pub async fn preview_static(
        &self,
        name: &str,
    ) -> MeloResult<Vec<crate::domain::library::repository::SongRecord>> {
        let conn = connect(&self.settings)?;
        let mut stmt = conn
            .prepare(
                "SELECT songs.id, songs.title, songs.lyrics, songs.lyrics_source_kind
                 FROM playlists
                 JOIN playlist_entries ON playlist_entries.playlist_id = playlists.id
                 JOIN songs ON songs.id = playlist_entries.song_id
                 WHERE playlists.name = ?1
                 ORDER BY playlist_entries.position ASC",
            )
            .map_err(|err| MeloError::Message(err.to_string()))?;

        let rows = stmt
            .query_map([name], |row| {
                Ok(crate::domain::library::repository::SongRecord {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    lyrics: row.get(2)?,
                    lyrics_source_kind: row.get(3)?,
                })
            })
            .map_err(|err| MeloError::Message(err.to_string()))?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|err| MeloError::Message(err.to_string()))
    }
}

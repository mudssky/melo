use sea_orm::ActiveValue::Set;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseBackend, EntityTrait, PaginatorTrait,
    QueryFilter, Statement,
};

use crate::core::config::settings::Settings;
use crate::core::db::connection::connect;
use crate::core::db::entities::{playlist_entries, playlists};
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
    /// # 返回值
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
    /// # 返回值
    /// - `MeloResult<()>`：创建结果
    pub async fn create_static(&self, name: &str, description: Option<&str>) -> MeloResult<()> {
        let connection = connect(&self.settings).await?;
        let now = crate::core::db::now_text();
        playlists::ActiveModel {
            name: Set(name.to_string()),
            description: Set(description.map(ToString::to_string)),
            created_at: Set(now.clone()),
            updated_at: Set(now),
            ..Default::default()
        }
        .insert(&connection)
        .await
        .map_err(|err| MeloError::Message(err.to_string()))?;
        Ok(())
    }

    /// 向静态歌单添加歌曲。
    ///
    /// # 参数
    /// - `name`：歌单名
    /// - `song_ids`：歌曲 ID 列表
    ///
    /// # 返回值
    /// - `MeloResult<()>`：写入结果
    pub async fn add_songs(&self, name: &str, song_ids: &[i64]) -> MeloResult<()> {
        let connection = connect(&self.settings).await?;
        let playlist = playlists::Entity::find()
            .filter(playlists::Column::Name.eq(name))
            .one(&connection)
            .await
            .map_err(|err| MeloError::Message(err.to_string()))?
            .ok_or_else(|| MeloError::Message(format!("未找到歌单: {name}")))?;

        let current_count = playlist_entries::Entity::find()
            .filter(playlist_entries::Column::PlaylistId.eq(playlist.id))
            .count(&connection)
            .await
            .map_err(|err| MeloError::Message(err.to_string()))? as i64;
        let now = crate::core::db::now_text();

        for (offset, song_id) in song_ids.iter().enumerate() {
            playlist_entries::ActiveModel {
                playlist_id: Set(playlist.id),
                song_id: Set(*song_id),
                position: Set(current_count + offset as i64),
                added_at: Set(now.clone()),
                ..Default::default()
            }
            .insert(&connection)
            .await
            .map_err(|err| MeloError::Message(err.to_string()))?;
        }

        Ok(())
    }

    /// 列出静态歌单摘要。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - `MeloResult<Vec<StaticPlaylistSummary>>`：静态歌单摘要列表
    pub async fn list_static(&self) -> MeloResult<Vec<StaticPlaylistSummary>> {
        let connection = connect(&self.settings).await?;
        let rows = connection
            .query_all(Statement::from_string(
                DatabaseBackend::Sqlite,
                "SELECT playlists.name AS name, COUNT(playlist_entries.id) AS count
                 FROM playlists
                 LEFT JOIN playlist_entries ON playlist_entries.playlist_id = playlists.id
                 GROUP BY playlists.id
                 ORDER BY playlists.name ASC"
                    .to_string(),
            ))
            .await
            .map_err(|err| MeloError::Message(err.to_string()))?;

        rows.into_iter()
            .map(|row| {
                Ok(StaticPlaylistSummary {
                    name: row.try_get("", "name")?,
                    count: row.try_get::<i64>("", "count")? as usize,
                })
            })
            .collect::<Result<Vec<_>, sea_orm::DbErr>>()
            .map_err(|err| MeloError::Message(err.to_string()))
    }

    /// 预览静态歌单中的歌曲。
    ///
    /// # 参数
    /// - `name`：歌单名
    ///
    /// # 返回值
    /// - `MeloResult<Vec<SongRecord>>`：歌单歌曲列表
    pub async fn preview_static(
        &self,
        name: &str,
    ) -> MeloResult<Vec<crate::domain::library::repository::SongRecord>> {
        let connection = connect(&self.settings).await?;
        let rows = connection
            .query_all(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                "SELECT songs.id, songs.title, songs.lyrics, songs.lyrics_source_kind
                 FROM playlists
                 JOIN playlist_entries ON playlist_entries.playlist_id = playlists.id
                 JOIN songs ON songs.id = playlist_entries.song_id
                 WHERE playlists.name = ?
                 ORDER BY playlist_entries.position ASC"
                    .to_string(),
                [name.into()],
            ))
            .await
            .map_err(|err| MeloError::Message(err.to_string()))?;

        rows.into_iter()
            .map(|row| {
                Ok(crate::domain::library::repository::SongRecord {
                    id: row.try_get("", "id")?,
                    title: row.try_get("", "title")?,
                    lyrics: row.try_get("", "lyrics")?,
                    lyrics_source_kind: row.try_get("", "lyrics_source_kind")?,
                })
            })
            .collect::<Result<Vec<_>, sea_orm::DbErr>>()
            .map_err(|err| MeloError::Message(err.to_string()))
    }
}

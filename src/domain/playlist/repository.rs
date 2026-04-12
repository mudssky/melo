use sea_orm::ActiveValue::Set;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseBackend, EntityTrait, IntoActiveModel,
    PaginatorTrait, QueryFilter, Statement,
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

/// 已持久化歌单摘要。
#[derive(Debug, Clone)]
pub struct StoredPlaylistSummary {
    /// 歌单名称。
    pub name: String,
    /// 歌单类型。
    pub kind: String,
    /// 歌单内歌曲数量。
    pub count: usize,
}

/// 已持久化歌单记录。
#[derive(Debug, Clone)]
pub struct StoredPlaylist {
    /// 歌单 ID。
    pub id: i64,
    /// 歌单名称。
    pub name: String,
    /// 歌单类型。
    pub kind: String,
    /// 是否在常规列表中可见。
    pub visible: bool,
}

/// 静态歌单仓储。
#[derive(Clone)]
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
            kind: Set("static".to_string()),
            source_kind: Set(None),
            source_key: Set(None),
            visible: Set(true),
            expires_at: Set(None),
            last_activated_at: Set(None),
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
                 WHERE playlists.kind = 'static'
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

    /// 列出所有可见的已持久化歌单摘要。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - `MeloResult<Vec<StoredPlaylistSummary>>`：可见歌单摘要列表
    pub async fn list_visible(&self) -> MeloResult<Vec<StoredPlaylistSummary>> {
        let connection = connect(&self.settings).await?;
        let rows = connection
            .query_all(Statement::from_string(
                DatabaseBackend::Sqlite,
                "SELECT playlists.name AS name, playlists.kind AS kind, COUNT(playlist_entries.id) AS count
                 FROM playlists
                 LEFT JOIN playlist_entries ON playlist_entries.playlist_id = playlists.id
                 WHERE playlists.visible = 1
                 GROUP BY playlists.id
                 ORDER BY playlists.name ASC"
                    .to_string(),
            ))
            .await
            .map_err(|err| MeloError::Message(err.to_string()))?;

        rows.into_iter()
            .map(|row| {
                Ok(StoredPlaylistSummary {
                    name: row.try_get("", "name")?,
                    kind: row.try_get("", "kind")?,
                    count: row.try_get::<i64>("", "count")? as usize,
                })
            })
            .collect::<Result<Vec<_>, sea_orm::DbErr>>()
            .map_err(|err| MeloError::Message(err.to_string()))
    }

    /// 复用或创建临时歌单，并重建其成员关系。
    ///
    /// # 参数
    /// - `name`：歌单显示名
    /// - `source_kind`：来源类型
    /// - `source_key`：来源唯一键
    /// - `visible`：是否在常规列表中可见
    /// - `expires_at`：可选过期时间
    /// - `song_ids`：歌单成员歌曲 ID 列表
    ///
    /// # 返回值
    /// - `MeloResult<StoredPlaylist>`：写入后的歌单记录
    pub async fn upsert_ephemeral(
        &self,
        name: &str,
        source_kind: &str,
        source_key: &str,
        visible: bool,
        expires_at: Option<&str>,
        song_ids: &[i64],
    ) -> MeloResult<StoredPlaylist> {
        let connection = connect(&self.settings).await?;
        let now = crate::core::db::now_text();

        let existing = playlists::Entity::find()
            .filter(playlists::Column::Kind.eq("ephemeral"))
            .filter(playlists::Column::SourceKind.eq(source_kind))
            .filter(playlists::Column::SourceKey.eq(source_key))
            .one(&connection)
            .await
            .map_err(|err| MeloError::Message(err.to_string()))?;

        let playlist_id = if let Some(existing) = existing {
            let playlist_id = existing.id;
            let mut model: playlists::ActiveModel = existing.into_active_model();
            model.name = Set(name.to_string());
            model.description = Set(None);
            model.visible = Set(visible);
            model.expires_at = Set(expires_at.map(ToString::to_string));
            model.last_activated_at = Set(Some(now.clone()));
            model.updated_at = Set(now.clone());
            model
                .update(&connection)
                .await
                .map_err(|err| MeloError::Message(err.to_string()))?;

            playlist_entries::Entity::delete_many()
                .filter(playlist_entries::Column::PlaylistId.eq(playlist_id))
                .exec(&connection)
                .await
                .map_err(|err| MeloError::Message(err.to_string()))?;

            playlist_id
        } else {
            playlists::ActiveModel {
                name: Set(name.to_string()),
                description: Set(None),
                kind: Set("ephemeral".to_string()),
                source_kind: Set(Some(source_kind.to_string())),
                source_key: Set(Some(source_key.to_string())),
                visible: Set(visible),
                expires_at: Set(expires_at.map(ToString::to_string)),
                last_activated_at: Set(Some(now.clone())),
                created_at: Set(now.clone()),
                updated_at: Set(now.clone()),
                ..Default::default()
            }
            .insert(&connection)
            .await
            .map_err(|err| MeloError::Message(err.to_string()))?
            .id
        };

        for (position, song_id) in song_ids.iter().enumerate() {
            playlist_entries::ActiveModel {
                playlist_id: Set(playlist_id),
                song_id: Set(*song_id),
                position: Set(position as i64),
                added_at: Set(now.clone()),
                ..Default::default()
            }
            .insert(&connection)
            .await
            .map_err(|err| MeloError::Message(err.to_string()))?;
        }

        Ok(StoredPlaylist {
            id: playlist_id,
            name: name.to_string(),
            kind: "ephemeral".to_string(),
            visible,
        })
    }

    /// 将临时歌单提升为正式静态歌单。
    ///
    /// # 参数
    /// - `source_key`：来源唯一键
    /// - `new_name`：新的静态歌单名
    ///
    /// # 返回值
    /// - `MeloResult<()>`：提升结果
    pub async fn promote_ephemeral(&self, source_key: &str, new_name: &str) -> MeloResult<()> {
        let connection = connect(&self.settings).await?;
        let playlist = playlists::Entity::find()
            .filter(playlists::Column::Kind.eq("ephemeral"))
            .filter(playlists::Column::SourceKey.eq(source_key))
            .one(&connection)
            .await
            .map_err(|err| MeloError::Message(err.to_string()))?
            .ok_or_else(|| MeloError::Message(format!("未找到临时歌单: {source_key}")))?;

        let mut model: playlists::ActiveModel = playlist.into_active_model();
        model.name = Set(new_name.to_string());
        model.kind = Set("static".to_string());
        model.source_kind = Set(None);
        model.source_key = Set(None);
        model.visible = Set(true);
        model.expires_at = Set(None);
        model.last_activated_at = Set(None);
        model.updated_at = Set(crate::core::db::now_text());
        model
            .update(&connection)
            .await
            .map_err(|err| MeloError::Message(err.to_string()))?;
        Ok(())
    }

    /// 清理已经过期的临时歌单。
    ///
    /// # 参数
    /// - `now_text`：用于比较的当前时间文本
    ///
    /// # 返回值
    /// - `MeloResult<u64>`：删除的歌单数量
    pub async fn cleanup_expired(&self, now_text: &str) -> MeloResult<u64> {
        let connection = connect(&self.settings).await?;
        playlists::Entity::delete_many()
            .filter(playlists::Column::Kind.eq("ephemeral"))
            .filter(playlists::Column::ExpiresAt.is_not_null())
            .filter(playlists::Column::ExpiresAt.lte(now_text))
            .exec(&connection)
            .await
            .map(|result| result.rows_affected)
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

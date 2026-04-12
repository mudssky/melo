use sea_orm::ActiveValue::Set;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseBackend, EntityTrait, IntoActiveModel,
    QueryFilter, QueryOrder, Statement,
};

use crate::core::config::settings::Settings;
use crate::core::db::connection::connect;
use crate::core::db::entities::{albums, artists, artwork_refs, songs};
use crate::core::error::{MeloError, MeloResult};
use crate::core::model::player::QueueItem;
use crate::domain::library::metadata::SongMetadata;
use crate::domain::playlist::query::SmartQuery;

/// 扫描后返回给调用方的歌曲摘要。
#[derive(Debug, Clone)]
pub struct SongRecord {
    /// 歌曲 ID。
    pub id: i64,
    /// 标题。
    pub title: String,
    /// 歌词文本。
    pub lyrics: Option<String>,
    /// 歌词来源类型。
    pub lyrics_source_kind: String,
}

/// 封面引用记录。
#[derive(Debug, Clone)]
pub struct ArtworkRefRecord {
    /// 来源类型。
    pub source_kind: String,
    /// 来源路径。
    pub source_path: Option<String>,
}

/// 组织文件时使用的候选歌曲上下文。
#[derive(Debug, Clone)]
pub struct OrganizeCandidate {
    /// 歌曲 ID。
    pub song_id: i64,
    /// 原始文件路径。
    pub source_path: String,
    /// 标题。
    pub title: String,
    /// 艺术家。
    pub artist: Option<String>,
    /// 关联的静态歌单名称。
    pub static_playlists: Vec<String>,
}

/// 面向媒体库持久化的仓储。
#[derive(Clone)]
pub struct LibraryRepository {
    settings: Settings,
}

impl LibraryRepository {
    /// 创建新的仓储对象。
    ///
    /// # 参数
    /// - `settings`：全局配置
    ///
    /// # 返回值
    /// - `Self`：仓储对象
    pub fn new(settings: Settings) -> Self {
        Self { settings }
    }

    /// 确保艺术家记录存在。
    ///
    /// # 参数
    /// - `connection`：数据库连接
    /// - `artist_name`：艺术家名称
    ///
    /// # 返回值
    /// - `MeloResult<Option<i64>>`：存在时返回艺术家 ID
    async fn ensure_artist(
        connection: &sea_orm::DatabaseConnection,
        artist_name: Option<&str>,
    ) -> MeloResult<Option<i64>> {
        let Some(artist_name) = artist_name.filter(|name| !name.is_empty()) else {
            return Ok(None);
        };

        let existing = artists::Entity::find()
            .filter(artists::Column::Name.eq(artist_name))
            .one(connection)
            .await
            .map_err(|err| MeloError::Message(err.to_string()))?;

        if let Some(model) = existing {
            return Ok(Some(model.id));
        }

        let now = crate::core::db::now_text();
        let artist = artists::ActiveModel {
            name: Set(artist_name.to_string()),
            sort_name: Set(Some(artist_name.to_string())),
            search_name: Set(artist_name.to_lowercase()),
            created_at: Set(now.clone()),
            updated_at: Set(now),
            ..Default::default()
        }
        .insert(connection)
        .await
        .map_err(|err| MeloError::Message(err.to_string()))?;
        Ok(Some(artist.id))
    }

    /// 确保专辑记录存在。
    ///
    /// # 参数
    /// - `connection`：数据库连接
    /// - `album_title`：专辑标题
    /// - `artist_id`：专辑艺术家 ID
    ///
    /// # 返回值
    /// - `MeloResult<Option<i64>>`：存在时返回专辑 ID
    async fn ensure_album(
        connection: &sea_orm::DatabaseConnection,
        album_title: Option<&str>,
        artist_id: Option<i64>,
    ) -> MeloResult<Option<i64>> {
        let Some(album_title) = album_title.filter(|title| !title.is_empty()) else {
            return Ok(None);
        };

        let existing = albums::Entity::find()
            .filter(albums::Column::Title.eq(album_title))
            .one(connection)
            .await
            .map_err(|err| MeloError::Message(err.to_string()))?;

        if let Some(model) = existing {
            return Ok(Some(model.id));
        }

        let now = crate::core::db::now_text();
        let album = albums::ActiveModel {
            title: Set(album_title.to_string()),
            album_artist_id: Set(artist_id),
            year: Set(None),
            source_dir: Set(None),
            created_at: Set(now.clone()),
            updated_at: Set(now),
            ..Default::default()
        }
        .insert(connection)
        .await
        .map_err(|err| MeloError::Message(err.to_string()))?;
        Ok(Some(album.id))
    }

    /// 读取文件大小和修改时间。
    ///
    /// # 参数
    /// - `path`：音频文件路径
    ///
    /// # 返回值
    /// - `MeloResult<(i64, i64)>`：文件大小与修改时间
    fn file_stats(path: &std::path::Path) -> MeloResult<(i64, i64)> {
        let metadata =
            std::fs::metadata(path).map_err(|err| MeloError::Message(err.to_string()))?;
        let file_size =
            i64::try_from(metadata.len()).map_err(|err| MeloError::Message(err.to_string()))?;
        let file_mtime = metadata
            .modified()
            .ok()
            .and_then(|mtime| mtime.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|duration| duration.as_secs() as i64)
            .unwrap_or_default();
        Ok((file_size, file_mtime))
    }

    /// 按需替换歌曲封面引用。
    ///
    /// # 参数
    /// - `connection`：数据库连接
    /// - `song_id`：歌曲 ID
    /// - `metadata`：歌曲元数据
    /// - `cover_path`：外置封面路径
    ///
    /// # 返回值
    /// - `MeloResult<()>`：写入结果
    async fn replace_artwork_ref(
        connection: &sea_orm::DatabaseConnection,
        song_id: i64,
        metadata: &SongMetadata,
        cover_path: Option<&std::path::Path>,
    ) -> MeloResult<()> {
        artwork_refs::Entity::delete_many()
            .filter(artwork_refs::Column::OwnerKind.eq("song"))
            .filter(artwork_refs::Column::OwnerId.eq(song_id))
            .exec(connection)
            .await
            .map_err(|err| MeloError::Message(err.to_string()))?;

        let now = crate::core::db::now_text();
        if let Some(cover_path) = cover_path {
            let mime = mime_guess::from_path(cover_path)
                .first_raw()
                .map(|value| value.to_string());
            artwork_refs::ActiveModel {
                owner_kind: Set("song".to_string()),
                owner_id: Set(song_id),
                source_kind: Set("sidecar".to_string()),
                source_path: Set(Some(cover_path.to_string_lossy().into_owned())),
                embedded_song_id: Set(None),
                mime: Set(mime),
                cache_path: Set(None),
                hash: Set(None),
                updated_at: Set(now),
                ..Default::default()
            }
            .insert(connection)
            .await
            .map_err(|err| MeloError::Message(err.to_string()))?;
        } else if let Some(embedded_artwork) = &metadata.embedded_artwork {
            artwork_refs::ActiveModel {
                owner_kind: Set("song".to_string()),
                owner_id: Set(song_id),
                source_kind: Set("embedded".to_string()),
                source_path: Set(None),
                embedded_song_id: Set(Some(song_id)),
                mime: Set(embedded_artwork.mime.clone()),
                cache_path: Set(None),
                hash: Set(None),
                updated_at: Set(now),
                ..Default::default()
            }
            .insert(connection)
            .await
            .map_err(|err| MeloError::Message(err.to_string()))?;
        }

        Ok(())
    }

    /// 将扫描到的歌曲写入数据库。
    ///
    /// # 参数
    /// - `path`：音频文件路径
    /// - `metadata`：歌曲元数据
    /// - `lyrics_source_path`：歌词 sidecar 路径
    /// - `cover_path`：封面 sidecar 路径
    ///
    /// # 返回值
    /// - `MeloResult<i64>`：写入后的歌曲 ID
    pub async fn upsert_song(
        &self,
        path: &std::path::Path,
        metadata: &SongMetadata,
        lyrics_source_path: Option<&str>,
        cover_path: Option<&std::path::Path>,
    ) -> MeloResult<i64> {
        let connection = connect(&self.settings).await?;
        let artist_id = Self::ensure_artist(&connection, metadata.artist.as_deref()).await?;
        let album_id =
            Self::ensure_album(&connection, metadata.album.as_deref(), artist_id).await?;
        let (file_size, file_mtime) = Self::file_stats(path)?;
        let path_text = path.to_string_lossy().into_owned();
        let now = crate::core::db::now_text();
        let lyrics_updated_at = metadata.lyrics.as_ref().map(|_| now.clone());

        let existing = songs::Entity::find()
            .filter(songs::Column::Path.eq(path_text.clone()))
            .one(&connection)
            .await
            .map_err(|err| MeloError::Message(err.to_string()))?;

        let song_id = if let Some(existing) = existing {
            let mut active_model: songs::ActiveModel = existing.into_active_model();
            active_model.title = Set(metadata.title.clone());
            active_model.artist_id = Set(artist_id);
            active_model.album_id = Set(album_id);
            active_model.track_no = Set(metadata.track_no.map(i64::from));
            active_model.disc_no = Set(metadata.disc_no.map(i64::from));
            active_model.duration_seconds = Set(metadata.duration_seconds);
            active_model.genre = Set(metadata.genre.clone());
            active_model.lyrics = Set(metadata.lyrics.clone());
            active_model.lyrics_source_kind = Set(metadata.lyrics_source_kind.as_str().to_string());
            active_model.lyrics_source_path = Set(lyrics_source_path.map(ToString::to_string));
            active_model.lyrics_format = Set(metadata.lyrics_format.clone());
            active_model.lyrics_updated_at = Set(lyrics_updated_at.clone());
            active_model.format = Set(metadata.format.clone());
            active_model.bitrate = Set(metadata.bitrate.map(i64::from));
            active_model.sample_rate = Set(metadata.sample_rate.map(i64::from));
            active_model.bit_depth = Set(metadata.bit_depth.map(i64::from));
            active_model.channels = Set(metadata.channels.map(i64::from));
            active_model.file_size = Set(file_size);
            active_model.file_mtime = Set(file_mtime);
            active_model.scanned_at = Set(now.clone());
            active_model.updated_at = Set(now.clone());
            active_model
                .update(&connection)
                .await
                .map_err(|err| MeloError::Message(err.to_string()))?
                .id
        } else {
            songs::ActiveModel {
                path: Set(path_text),
                title: Set(metadata.title.clone()),
                artist_id: Set(artist_id),
                album_id: Set(album_id),
                track_no: Set(metadata.track_no.map(i64::from)),
                disc_no: Set(metadata.disc_no.map(i64::from)),
                duration_seconds: Set(metadata.duration_seconds),
                genre: Set(metadata.genre.clone()),
                lyrics: Set(metadata.lyrics.clone()),
                lyrics_source_kind: Set(metadata.lyrics_source_kind.as_str().to_string()),
                lyrics_source_path: Set(lyrics_source_path.map(ToString::to_string)),
                lyrics_format: Set(metadata.lyrics_format.clone()),
                lyrics_updated_at: Set(lyrics_updated_at),
                format: Set(metadata.format.clone()),
                bitrate: Set(metadata.bitrate.map(i64::from)),
                sample_rate: Set(metadata.sample_rate.map(i64::from)),
                bit_depth: Set(metadata.bit_depth.map(i64::from)),
                channels: Set(metadata.channels.map(i64::from)),
                file_size: Set(file_size),
                file_mtime: Set(file_mtime),
                added_at: Set(now.clone()),
                scanned_at: Set(now.clone()),
                organized_at: Set(None),
                last_organize_rule: Set(None),
                updated_at: Set(now.clone()),
                ..Default::default()
            }
            .insert(&connection)
            .await
            .map_err(|err| MeloError::Message(err.to_string()))?
            .id
        };

        if cover_path.is_some() || metadata.embedded_artwork.is_some() {
            Self::replace_artwork_ref(&connection, song_id, metadata, cover_path).await?;
        }

        Ok(song_id)
    }

    /// 列出全部歌曲摘要。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - `MeloResult<Vec<SongRecord>>`：歌曲列表
    pub async fn list_songs(&self) -> MeloResult<Vec<SongRecord>> {
        let connection = connect(&self.settings).await?;
        let models = songs::Entity::find()
            .order_by_asc(songs::Column::Id)
            .all(&connection)
            .await
            .map_err(|err| MeloError::Message(err.to_string()))?;

        Ok(models
            .into_iter()
            .map(|model| SongRecord {
                id: model.id,
                title: model.title,
                lyrics: model.lyrics,
                lyrics_source_kind: model.lyrics_source_kind,
            })
            .collect())
    }

    /// 按文件路径顺序返回对应歌曲 ID。
    ///
    /// # 参数
    /// - `paths`：文件路径列表
    ///
    /// # 返回值
    /// - `MeloResult<Vec<i64>>`：对应歌曲 ID 列表
    pub async fn song_ids_by_paths(&self, paths: &[std::path::PathBuf]) -> MeloResult<Vec<i64>> {
        let connection = connect(&self.settings).await?;
        let mut song_ids = Vec::with_capacity(paths.len());

        for path in paths {
            let path_text = path.to_string_lossy().into_owned();
            let song = songs::Entity::find()
                .filter(songs::Column::Path.eq(path_text.clone()))
                .one(&connection)
                .await
                .map_err(|err| MeloError::Message(err.to_string()))?
                .ok_or_else(|| MeloError::Message(format!("未找到歌曲: {path_text}")))?;
            song_ids.push(song.id);
        }

        Ok(song_ids)
    }

    /// 按歌曲 ID 顺序构造播放器队列项。
    ///
    /// # 参数
    /// - `song_ids`：歌曲 ID 列表
    ///
    /// # 返回值
    /// - `MeloResult<Vec<QueueItem>>`：播放器队列项列表
    pub async fn queue_items_by_song_ids(&self, song_ids: &[i64]) -> MeloResult<Vec<QueueItem>> {
        let connection = connect(&self.settings).await?;
        let mut items = Vec::with_capacity(song_ids.len());

        for song_id in song_ids {
            let song = songs::Entity::find_by_id(*song_id)
                .one(&connection)
                .await
                .map_err(|err| MeloError::Message(err.to_string()))?
                .ok_or_else(|| MeloError::Message(format!("未找到歌曲: {song_id}")))?;
            items.push(QueueItem {
                song_id: song.id,
                path: song.path,
                title: song.title,
                duration_seconds: song.duration_seconds,
            });
        }

        Ok(items)
    }

    /// 按歌曲 ID 查询封面引用。
    ///
    /// # 参数
    /// - `song_id`：歌曲 ID
    ///
    /// # 返回值
    /// - `MeloResult<Option<ArtworkRefRecord>>`：封面引用记录
    pub async fn artwork_for_song(&self, song_id: i64) -> MeloResult<Option<ArtworkRefRecord>> {
        let connection = connect(&self.settings).await?;
        let record = artwork_refs::Entity::find()
            .filter(artwork_refs::Column::OwnerKind.eq("song"))
            .filter(artwork_refs::Column::OwnerId.eq(song_id))
            .one(&connection)
            .await
            .map_err(|err| MeloError::Message(err.to_string()))?;

        Ok(record.map(|model| ArtworkRefRecord {
            source_kind: model.source_kind,
            source_path: model.source_path,
        }))
    }

    /// 按 smart query 统计歌曲数量。
    ///
    /// # 参数
    /// - `query`：结构化查询
    ///
    /// # 返回值
    /// - `MeloResult<usize>`：命中数量
    pub async fn count_by_query(&self, query: &SmartQuery) -> MeloResult<usize> {
        Ok(self.list_by_query(query).await?.len())
    }

    /// 按 smart query 列出歌曲。
    ///
    /// # 参数
    /// - `query`：结构化查询
    ///
    /// # 返回值
    /// - `MeloResult<Vec<SongRecord>>`：命中的歌曲
    pub async fn list_by_query(&self, query: &SmartQuery) -> MeloResult<Vec<SongRecord>> {
        let connection = connect(&self.settings).await?;
        let (where_sql, params) = crate::domain::library::query::build_song_search_sql(query);
        let sql = format!(
            "SELECT songs.id, songs.title, songs.lyrics, songs.lyrics_source_kind
             FROM songs
             LEFT JOIN artists ON artists.id = songs.artist_id
             LEFT JOIN albums ON albums.id = songs.album_id
             WHERE {where_sql}
             ORDER BY songs.id ASC"
        );

        let rows = connection
            .query_all(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                sql,
                params.into_iter().map(Into::into).collect::<Vec<_>>(),
            ))
            .await
            .map_err(|err| MeloError::Message(err.to_string()))?;

        rows.into_iter()
            .map(|row| {
                Ok(SongRecord {
                    id: row.try_get("", "id")?,
                    title: row.try_get("", "title")?,
                    lyrics: row.try_get("", "lyrics")?,
                    lyrics_source_kind: row.try_get("", "lyrics_source_kind")?,
                })
            })
            .collect::<Result<Vec<_>, sea_orm::DbErr>>()
            .map_err(|err| MeloError::Message(err.to_string()))
    }

    /// 读取 organize 候选歌曲。
    ///
    /// # 参数
    /// - `song_id`：可选歌曲 ID 过滤
    ///
    /// # 返回值
    /// - `MeloResult<Vec<OrganizeCandidate>>`：候选列表
    pub async fn organize_candidates(
        &self,
        song_id: Option<i64>,
    ) -> MeloResult<Vec<OrganizeCandidate>> {
        let connection = connect(&self.settings).await?;
        let base_sql = if song_id.is_some() {
            "SELECT songs.id, songs.path, songs.title, artists.name
             FROM songs
             LEFT JOIN artists ON artists.id = songs.artist_id
             WHERE songs.id = ?
             ORDER BY songs.id ASC"
        } else {
            "SELECT songs.id, songs.path, songs.title, artists.name
             FROM songs
             LEFT JOIN artists ON artists.id = songs.artist_id
             ORDER BY songs.id ASC"
        };

        let rows = if let Some(song_id) = song_id {
            connection
                .query_all(Statement::from_sql_and_values(
                    DatabaseBackend::Sqlite,
                    base_sql.to_string(),
                    [song_id.into()],
                ))
                .await
        } else {
            connection
                .query_all(Statement::from_string(
                    DatabaseBackend::Sqlite,
                    base_sql.to_string(),
                ))
                .await
        }
        .map_err(|err| MeloError::Message(err.to_string()))?;

        let mut candidates = Vec::new();
        for row in rows {
            let song_id: i64 = row
                .try_get("", "id")
                .map_err(|err| MeloError::Message(err.to_string()))?;
            let playlist_rows = connection
                .query_all(Statement::from_sql_and_values(
                    DatabaseBackend::Sqlite,
                    "SELECT playlists.name
                     FROM playlist_entries
                     JOIN playlists ON playlists.id = playlist_entries.playlist_id
                     WHERE playlist_entries.song_id = ?
                     ORDER BY playlist_entries.position ASC"
                        .to_string(),
                    [song_id.into()],
                ))
                .await
                .map_err(|err| MeloError::Message(err.to_string()))?;
            let static_playlists = playlist_rows
                .into_iter()
                .map(|playlist_row| playlist_row.try_get("", "name"))
                .collect::<Result<Vec<String>, _>>()
                .map_err(|err| MeloError::Message(err.to_string()))?;

            candidates.push(OrganizeCandidate {
                song_id,
                source_path: row
                    .try_get("", "path")
                    .map_err(|err| MeloError::Message(err.to_string()))?,
                title: row
                    .try_get("", "title")
                    .map_err(|err| MeloError::Message(err.to_string()))?,
                artist: row
                    .try_get("", "name")
                    .map_err(|err| MeloError::Message(err.to_string()))?,
                static_playlists,
            });
        }

        Ok(candidates)
    }

    /// 记录 organize 后的新路径与规则名。
    ///
    /// # 参数
    /// - `song_id`：歌曲 ID
    /// - `target_path`：目标路径
    /// - `rule_name`：命中的规则名
    ///
    /// # 返回值
    /// - `MeloResult<()>`：写入结果
    pub async fn record_organized_path(
        &self,
        song_id: i64,
        target_path: &str,
        rule_name: &str,
    ) -> MeloResult<()> {
        let connection = connect(&self.settings).await?;
        let song = songs::Entity::find_by_id(song_id)
            .one(&connection)
            .await
            .map_err(|err| MeloError::Message(err.to_string()))?
            .ok_or_else(|| MeloError::Message(format!("未找到歌曲: {song_id}")))?;
        let now = crate::core::db::now_text();
        let mut active_model: songs::ActiveModel = song.into_active_model();
        active_model.path = Set(target_path.to_string());
        active_model.last_organize_rule = Set(Some(rule_name.to_string()));
        active_model.organized_at = Set(Some(now.clone()));
        active_model.updated_at = Set(now);
        active_model
            .update(&connection)
            .await
            .map_err(|err| MeloError::Message(err.to_string()))?;
        Ok(())
    }
}

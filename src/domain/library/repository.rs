use rusqlite::OptionalExtension;

use crate::core::config::settings::Settings;
use crate::core::db::connection::connect;
use crate::core::error::{MeloError, MeloResult};
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

/// 面向 SQLite 的库仓储。
pub struct LibraryRepository {
    settings: Settings,
}

impl LibraryRepository {
    /// 创建新的仓储对象。
    ///
    /// # 参数
    /// - `settings`：全局配置
    ///
    /// # 返回
    /// - `Self`：仓储对象
    pub fn new(settings: Settings) -> Self {
        Self { settings }
    }

    fn ensure_artist(
        conn: &rusqlite::Connection,
        artist_name: Option<&str>,
    ) -> Result<Option<i64>, rusqlite::Error> {
        let Some(artist_name) = artist_name.filter(|name| !name.is_empty()) else {
            return Ok(None);
        };

        let existing = conn
            .query_row(
                "SELECT id FROM artists WHERE name = ?1 LIMIT 1",
                [artist_name],
                |row| row.get::<_, i64>(0),
            )
            .optional()?;

        if let Some(id) = existing {
            return Ok(Some(id));
        }

        conn.execute(
            "INSERT INTO artists (name, sort_name, search_name, created_at, updated_at) VALUES (?1, ?1, lower(?1), datetime('now'), datetime('now'))",
            [artist_name],
        )?;

        Ok(Some(conn.last_insert_rowid()))
    }

    fn ensure_album(
        conn: &rusqlite::Connection,
        album_title: Option<&str>,
        artist_id: Option<i64>,
    ) -> Result<Option<i64>, rusqlite::Error> {
        let Some(album_title) = album_title.filter(|title| !title.is_empty()) else {
            return Ok(None);
        };

        let existing = conn
            .query_row(
                "SELECT id FROM albums WHERE title = ?1 LIMIT 1",
                [album_title],
                |row| row.get::<_, i64>(0),
            )
            .optional()?;

        if let Some(id) = existing {
            return Ok(Some(id));
        }

        conn.execute(
            "INSERT INTO albums (title, album_artist_id, year, source_dir, created_at, updated_at) VALUES (?1, ?2, NULL, NULL, datetime('now'), datetime('now'))",
            rusqlite::params![album_title, artist_id],
        )?;

        Ok(Some(conn.last_insert_rowid()))
    }

    /// 将扫描到的歌曲写入数据库。
    ///
    /// # 参数
    /// - `path`：音频文件路径
    /// - `metadata`：歌曲元数据
    /// - `lyrics_source_path`：歌词 sidecar 路径
    /// - `cover_path`：封面 sidecar 路径
    ///
    /// # 返回
    /// - `MeloResult<i64>`：写入后的歌曲 ID
    pub async fn upsert_song(
        &self,
        path: &std::path::Path,
        metadata: &SongMetadata,
        lyrics_source_path: Option<&str>,
        cover_path: Option<&std::path::Path>,
    ) -> MeloResult<i64> {
        let conn = connect(&self.settings)?;
        let artist_id = Self::ensure_artist(&conn, metadata.artist.as_deref())
            .map_err(|err| MeloError::Message(err.to_string()))?;
        let album_id = Self::ensure_album(&conn, metadata.album.as_deref(), artist_id)
            .map_err(|err| MeloError::Message(err.to_string()))?;

        let file_meta =
            std::fs::metadata(path).map_err(|err| MeloError::Message(err.to_string()))?;
        let file_size =
            i64::try_from(file_meta.len()).map_err(|err| MeloError::Message(err.to_string()))?;
        let file_mtime = file_meta
            .modified()
            .ok()
            .and_then(|mtime| mtime.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|duration| duration.as_secs() as i64)
            .unwrap_or_default();

        conn.execute(
            "INSERT INTO songs (
                path, title, artist_id, album_id, track_no, disc_no, duration_seconds, genre,
                lyrics, lyrics_source_kind, lyrics_source_path, lyrics_format, lyrics_updated_at,
                format, bitrate, sample_rate, bit_depth, channels, file_size, file_mtime,
                added_at, scanned_at, updated_at
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8,
                ?9, ?10, ?11, ?12, datetime('now'),
                ?13, ?14, ?15, ?16, ?17, ?18, ?19,
                datetime('now'), datetime('now'), datetime('now')
            )",
            rusqlite::params![
                path.to_string_lossy().to_string(),
                metadata.title,
                artist_id,
                album_id,
                metadata.track_no.map(i64::from),
                metadata.disc_no.map(i64::from),
                metadata.duration_seconds,
                metadata.genre,
                metadata.lyrics,
                metadata.lyrics_source_kind.as_str(),
                lyrics_source_path,
                metadata.lyrics_format,
                metadata.format,
                metadata.bitrate.map(i64::from),
                metadata.sample_rate.map(i64::from),
                metadata.bit_depth.map(i64::from),
                metadata.channels.map(i64::from),
                file_size,
                file_mtime,
            ],
        )
        .map_err(|err| MeloError::Message(err.to_string()))?;

        let song_id = conn.last_insert_rowid();

        if let Some(cover_path) = cover_path {
            conn.execute(
                "INSERT INTO artwork_refs (owner_kind, owner_id, source_kind, source_path, embedded_song_id, mime, cache_path, hash, updated_at)
                 VALUES ('song', ?1, 'sidecar', ?2, NULL, NULL, NULL, NULL, datetime('now'))",
                rusqlite::params![song_id, cover_path.to_string_lossy().to_string()],
            )
            .map_err(|err| MeloError::Message(err.to_string()))?;
        } else if let Some(embedded_artwork) = &metadata.embedded_artwork {
            conn.execute(
                "INSERT INTO artwork_refs (owner_kind, owner_id, source_kind, source_path, embedded_song_id, mime, cache_path, hash, updated_at)
                 VALUES ('song', ?1, 'embedded', NULL, ?1, ?2, NULL, NULL, datetime('now'))",
                rusqlite::params![song_id, embedded_artwork.mime.clone()],
            )
            .map_err(|err| MeloError::Message(err.to_string()))?;
        }

        Ok(song_id)
    }

    /// 列出全部歌曲摘要。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回
    /// - `MeloResult<Vec<SongRecord>>`：歌曲列表
    pub async fn list_songs(&self) -> MeloResult<Vec<SongRecord>> {
        let conn = connect(&self.settings)?;
        let mut stmt = conn
            .prepare("SELECT id, title, lyrics, lyrics_source_kind FROM songs ORDER BY id ASC")
            .map_err(|err| MeloError::Message(err.to_string()))?;
        let rows = stmt
            .query_map([], |row| {
                Ok(SongRecord {
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

    /// 按歌曲 ID 查询封面引用。
    ///
    /// # 参数
    /// - `song_id`：歌曲 ID
    ///
    /// # 返回
    /// - `MeloResult<Option<ArtworkRefRecord>>`：封面引用记录
    pub async fn artwork_for_song(&self, song_id: i64) -> MeloResult<Option<ArtworkRefRecord>> {
        let conn = connect(&self.settings)?;
        conn.query_row(
            "SELECT source_kind, source_path FROM artwork_refs WHERE owner_kind = 'song' AND owner_id = ?1 LIMIT 1",
            [song_id],
            |row| {
                Ok(ArtworkRefRecord {
                    source_kind: row.get(0)?,
                    source_path: row.get(1)?,
                })
            },
        )
        .optional()
        .map_err(|err| MeloError::Message(err.to_string()))
    }

    /// 按 smart query 统计歌曲数量。
    ///
    /// # 参数
    /// - `query`：结构化查询
    ///
    /// # 返回
    /// - `MeloResult<usize>`：命中数量
    pub async fn count_by_query(&self, query: &SmartQuery) -> MeloResult<usize> {
        Ok(self.list_by_query(query).await?.len())
    }

    /// 按 smart query 列出歌曲。
    ///
    /// # 参数
    /// - `query`：结构化查询
    ///
    /// # 返回
    /// - `MeloResult<Vec<SongRecord>>`：命中歌曲
    pub async fn list_by_query(&self, query: &SmartQuery) -> MeloResult<Vec<SongRecord>> {
        let conn = connect(&self.settings)?;
        let (where_sql, params) = crate::domain::library::query::build_song_search_sql(query);
        let sql = format!(
            "SELECT songs.id, songs.title, songs.lyrics, songs.lyrics_source_kind
             FROM songs
             LEFT JOIN artists ON artists.id = songs.artist_id
             LEFT JOIN albums ON albums.id = songs.album_id
             WHERE {where_sql}
             ORDER BY songs.id ASC"
        );

        let mut stmt = conn
            .prepare(&sql)
            .map_err(|err| MeloError::Message(err.to_string()))?;
        let params = rusqlite::params_from_iter(params.iter());
        let rows = stmt
            .query_map(params, |row| {
                Ok(SongRecord {
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

    /// 读取 organize 候选歌曲。
    ///
    /// # 参数
    /// - `song_id`：可选歌曲 ID 过滤
    ///
    /// # 返回
    /// - `MeloResult<Vec<OrganizeCandidate>>`：候选列表
    pub async fn organize_candidates(
        &self,
        song_id: Option<i64>,
    ) -> MeloResult<Vec<OrganizeCandidate>> {
        let conn = connect(&self.settings)?;
        let sql = if song_id.is_some() {
            "SELECT songs.id, songs.path, songs.title, artists.name
             FROM songs
             LEFT JOIN artists ON artists.id = songs.artist_id
             WHERE songs.id = ?1
             ORDER BY songs.id ASC"
        } else {
            "SELECT songs.id, songs.path, songs.title, artists.name
             FROM songs
             LEFT JOIN artists ON artists.id = songs.artist_id
             ORDER BY songs.id ASC"
        };
        let mut stmt = conn
            .prepare(sql)
            .map_err(|err| MeloError::Message(err.to_string()))?;
        let mapper = |row: &rusqlite::Row<'_>| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
            ))
        };

        let rows = if let Some(song_id) = song_id {
            stmt.query_map([song_id], mapper)
        } else {
            stmt.query_map([], mapper)
        }
        .map_err(|err| MeloError::Message(err.to_string()))?;

        let mut candidates = Vec::new();
        for row in rows {
            let (song_id, source_path, title, artist) =
                row.map_err(|err| MeloError::Message(err.to_string()))?;
            let mut playlist_stmt = conn
                .prepare(
                    "SELECT playlists.name
                     FROM playlist_entries
                     JOIN playlists ON playlists.id = playlist_entries.playlist_id
                     WHERE playlist_entries.song_id = ?1
                     ORDER BY playlist_entries.position ASC",
                )
                .map_err(|err| MeloError::Message(err.to_string()))?;
            let playlist_rows = playlist_stmt
                .query_map([song_id], |playlist_row| playlist_row.get::<_, String>(0))
                .map_err(|err| MeloError::Message(err.to_string()))?;
            let static_playlists = playlist_rows
                .collect::<Result<Vec<_>, _>>()
                .map_err(|err| MeloError::Message(err.to_string()))?;

            candidates.push(OrganizeCandidate {
                song_id,
                source_path,
                title,
                artist,
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
    /// # 返回
    /// - `MeloResult<()>`：写入结果
    pub async fn record_organized_path(
        &self,
        song_id: i64,
        target_path: &str,
        rule_name: &str,
    ) -> MeloResult<()> {
        let conn = connect(&self.settings)?;
        conn.execute(
            "UPDATE songs
             SET path = ?1, last_organize_rule = ?2, organized_at = datetime('now'), updated_at = datetime('now')
             WHERE id = ?3",
            rusqlite::params![target_path, rule_name, song_id],
        )
        .map_err(|err| MeloError::Message(err.to_string()))?;
        Ok(())
    }
}

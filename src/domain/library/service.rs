use std::sync::Arc;

use crate::core::config::settings::Settings;
use crate::core::error::{MeloError, MeloResult};
use crate::core::model::player::QueueItem;
use crate::core::model::track_content::{ArtworkSummary, TrackContentSnapshot};
use crate::domain::library::lofty_reader::LoftyMetadataReader;
use crate::domain::library::lyrics::parse_lyrics_timeline;
use crate::domain::library::metadata::{LyricsSourceKind, MetadataReader, NullMetadataReader};
use crate::domain::library::organize::OrganizePreviewRow;
use crate::domain::library::repository::{
    ArtworkRefRecord, LibraryRepository, SongRecord, TrackContentRecord,
};

/// 媒体库服务，负责目录扫描与数据查询。
#[derive(Clone)]
pub struct LibraryService {
    settings: Settings,
    reader: Arc<dyn MetadataReader>,
    repository: LibraryRepository,
}

impl LibraryService {
    /// 创建新的媒体库服务。
    ///
    /// # 参数
    /// - `settings`：全局配置
    /// - `reader`：元数据读取器
    ///
    /// # 返回值
    /// - `Self`：媒体库服务
    pub fn new(settings: Settings, reader: Arc<dyn MetadataReader>) -> Self {
        let repository = LibraryRepository::new(settings.clone());
        Self {
            settings,
            reader,
            repository,
        }
    }

    /// 创建默认使用 `Lofty` 的媒体库服务。
    ///
    /// # 参数
    /// - `settings`：全局配置
    ///
    /// # 返回值
    /// - `Self`：默认生产用媒体库服务
    pub fn with_lofty(settings: Settings) -> Self {
        Self::new(settings, Arc::new(LoftyMetadataReader))
    }

    /// 构造一个仅用于测试的服务。
    ///
    /// # 参数
    /// - `settings`：全局配置
    ///
    /// # 返回值
    /// - `Self`：测试用服务
    pub fn for_test(settings: Settings) -> Self {
        Self::new(settings, Arc::new(NullMetadataReader))
    }

    /// 扫描给定目录列表并写入数据库。
    ///
    /// # 参数
    /// - `roots`：待扫描目录列表
    ///
    /// # 返回值
    /// - `MeloResult<()>`：扫描结果
    pub async fn scan_paths(&self, roots: &[std::path::PathBuf]) -> MeloResult<()> {
        let _ = &self.settings;

        for root in roots {
            for entry in walkdir::WalkDir::new(root)
                .into_iter()
                .filter_map(Result::ok)
                .filter(|entry| entry.file_type().is_file())
            {
                let path = entry.path();
                if !crate::domain::open::formats::is_supported_audio_path(path) {
                    continue;
                }

                self.ensure_song_id_for_path(path).await?;
            }
        }

        Ok(())
    }

    /// 确保给定路径已经被扫描并返回对应歌曲 ID。
    ///
    /// # 参数
    /// - `audio_paths`：待确保存在的音频路径列表
    /// - `prewarm_limit`：同步预热上限
    ///
    /// # 返回值
    /// - `MeloResult<Vec<i64>>`：对应歌曲 ID 列表
    pub async fn ensure_scanned_paths(
        &self,
        audio_paths: &[std::path::PathBuf],
        prewarm_limit: usize,
    ) -> MeloResult<Vec<i64>> {
        let split_at = prewarm_limit.min(audio_paths.len());
        if split_at > 0 {
            self.scan_paths(&audio_paths[..split_at]).await?;
        }
        if split_at < audio_paths.len() {
            self.scan_paths(&audio_paths[split_at..]).await?;
        }

        self.repository.song_ids_by_paths(audio_paths).await
    }

    /// 确保单个音频路径已经扫描入库，并返回对应歌曲 ID。
    ///
    /// # 参数
    /// - `path`：音频文件路径
    ///
    /// # 返回值
    /// - `MeloResult<i64>`：对应歌曲 ID
    pub async fn ensure_song_id_for_path(&self, path: &std::path::Path) -> MeloResult<i64> {
        let mut metadata = self.reader.read(path)?;
        let mut lyrics_source_path = None;
        if let Some(resolved_lyrics) =
            crate::domain::library::assets::resolve_lyrics(path, &metadata)
        {
            metadata.lyrics = Some(resolved_lyrics.text);
            metadata.lyrics_source_kind = resolved_lyrics.source_kind;
            metadata.lyrics_format = Some(resolved_lyrics.format);
            lyrics_source_path = resolved_lyrics.source_path;
        } else {
            metadata.lyrics = None;
            metadata.lyrics_source_kind = LyricsSourceKind::None;
            metadata.lyrics_format = None;
        }

        let cover_path = crate::domain::library::assets::find_cover(path);
        self.repository
            .upsert_song(
                path,
                &metadata,
                lyrics_source_path.as_deref(),
                cover_path.as_deref(),
            )
            .await
    }

    /// 按歌曲 ID 顺序构造播放器队列项。
    ///
    /// # 参数
    /// - `song_ids`：歌曲 ID 列表
    ///
    /// # 返回值
    /// - `MeloResult<Vec<QueueItem>>`：播放器队列项列表
    pub async fn queue_items_for_song_ids(&self, song_ids: &[i64]) -> MeloResult<Vec<QueueItem>> {
        self.repository.queue_items_by_song_ids(song_ids).await
    }

    /// 列出库中的歌曲摘要。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - `MeloResult<Vec<SongRecord>>`：歌曲列表
    pub async fn list_songs(&self) -> MeloResult<Vec<SongRecord>> {
        self.repository.list_songs().await
    }

    /// 查询某首歌的封面引用。
    ///
    /// # 参数
    /// - `song_id`：歌曲 ID
    ///
    /// # 返回值
    /// - `MeloResult<Option<ArtworkRefRecord>>`：封面引用记录
    pub async fn artwork_for_song(&self, song_id: i64) -> MeloResult<Option<ArtworkRefRecord>> {
        self.repository.artwork_for_song(song_id).await
    }

    /// 返回指定歌曲的低频内容快照。
    ///
    /// # 参数
    /// - `song_id`：歌曲 ID
    ///
    /// # 返回值
    /// - `MeloResult<TrackContentSnapshot>`：歌曲内容快照
    pub async fn track_content(&self, song_id: i64) -> MeloResult<TrackContentSnapshot> {
        let record = self
            .repository
            .track_content(song_id)
            .await?
            .ok_or_else(|| MeloError::Message(format!("未找到歌曲: {song_id}")))?;
        self.build_track_content_snapshot(record).await
    }

    /// 刷新并返回指定歌曲的低频内容快照。
    ///
    /// # 参数
    /// - `song_id`：歌曲 ID
    ///
    /// # 返回值
    /// - `MeloResult<TrackContentSnapshot>`：刷新后的歌曲内容快照
    pub async fn refresh_track_content(&self, song_id: i64) -> MeloResult<TrackContentSnapshot> {
        let record = self
            .repository
            .track_content(song_id)
            .await?
            .ok_or_else(|| MeloError::Message(format!("未找到歌曲: {song_id}")))?;
        self.ensure_song_id_for_path(std::path::Path::new(&record.path))
            .await?;
        self.track_content(song_id).await
    }

    /// 预览 organize 结果。
    ///
    /// # 参数
    /// - `song_id`：可选歌曲 ID 过滤
    ///
    /// # 返回值
    /// - `MeloResult<Vec<OrganizePreviewRow>>`：预览结果
    pub async fn preview_organize(
        &self,
        song_id: Option<i64>,
    ) -> MeloResult<Vec<OrganizePreviewRow>> {
        let settings = Settings::load().unwrap_or_else(|_| self.settings.clone());
        let candidates = self.repository.organize_candidates(song_id).await?;
        let mut rows = Vec::new();

        for candidate in candidates {
            let Some(rule) = crate::domain::library::organize::choose_rule(
                &settings.library.organize.rules,
                &candidate,
            ) else {
                continue;
            };
            let target_path =
                crate::domain::library::organize::render_target_path(&settings, rule, &candidate)
                    .map_err(|err| MeloError::Message(err.to_string()))?;
            rows.push(OrganizePreviewRow {
                song_id: candidate.song_id,
                rule_name: rule.name.clone(),
                source_path: candidate.source_path,
                target_path,
            });
        }

        Ok(rows)
    }

    /// 执行 organize，并同步移动同名歌词 sidecar。
    ///
    /// # 参数
    /// - `song_id`：可选歌曲 ID 过滤
    ///
    /// # 返回值
    /// - `MeloResult<()>`：执行结果
    pub async fn apply_organize(&self, song_id: Option<i64>) -> MeloResult<()> {
        for row in self.preview_organize(song_id).await? {
            let source = std::path::Path::new(&row.source_path);
            let target = std::path::Path::new(&row.target_path);
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|err| MeloError::Message(err.to_string()))?;
            }
            std::fs::rename(source, target).map_err(|err| MeloError::Message(err.to_string()))?;

            for (sidecar_source, sidecar_target) in
                crate::domain::library::organize::sidecar_targets(source, target)
            {
                if let Some(parent) = sidecar_target.parent() {
                    std::fs::create_dir_all(parent)
                        .map_err(|err| MeloError::Message(err.to_string()))?;
                }
                std::fs::rename(sidecar_source, sidecar_target)
                    .map_err(|err| MeloError::Message(err.to_string()))?;
            }

            self.repository
                .record_organized_path(row.song_id, &row.target_path, &row.rule_name)
                .await?;
        }

        Ok(())
    }

    /// 将数据库歌曲详情记录转换为曲目内容快照。
    ///
    /// # 参数
    /// - `record`：歌曲详情记录
    ///
    /// # 返回值
    /// - `MeloResult<TrackContentSnapshot>`：构造后的曲目内容快照
    async fn build_track_content_snapshot(
        &self,
        record: TrackContentRecord,
    ) -> MeloResult<TrackContentSnapshot> {
        let artwork = self
            .repository
            .artwork_for_song(record.song_id)
            .await?
            .map(|artwork| ArtworkSummary {
                terminal_summary: format!("Cover: {}", artwork.source_kind),
                source_kind: artwork.source_kind,
                source_path: artwork.source_path,
            });
        let refresh_token = format!(
            "song-{}-{}-{}-{}",
            record.song_id, record.file_mtime, record.updated_at, record.lyrics_source_kind
        );

        Ok(TrackContentSnapshot {
            song_id: record.song_id,
            title: record.title,
            duration_seconds: record.duration_seconds,
            artwork,
            lyrics: record
                .lyrics
                .as_deref()
                .map(parse_lyrics_timeline)
                .unwrap_or_default(),
            refresh_token,
        })
    }
}

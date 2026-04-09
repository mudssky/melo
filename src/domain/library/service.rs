use std::sync::Arc;

use crate::core::config::settings::Settings;
use crate::core::error::MeloResult;
use crate::domain::library::metadata::{LyricsSourceKind, MetadataReader};
use crate::domain::library::repository::{ArtworkRefRecord, LibraryRepository, SongRecord};

/// 媒体库服务，负责目录扫描与数据查询。
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
    /// # 返回
    /// - `Self`：媒体库服务
    pub fn new(settings: Settings, reader: Arc<dyn MetadataReader>) -> Self {
        let repository = LibraryRepository::new(settings.clone());
        Self {
            settings,
            reader,
            repository,
        }
    }

    /// 扫描给定目录列表并写入数据库。
    ///
    /// # 参数
    /// - `roots`：待扫描目录列表
    ///
    /// # 返回
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
                if !matches!(
                    path.extension().and_then(|ext| ext.to_str()),
                    Some("flac" | "mp3" | "ogg" | "wav")
                ) {
                    continue;
                }

                let mut metadata = self.reader.read(path)?;
                let mut lyrics_source_path = None;
                if let Some((source_path, lyrics, format)) =
                    crate::domain::library::assets::find_sidecar_lyrics(path)
                {
                    metadata.lyrics = Some(lyrics);
                    metadata.lyrics_source_kind = LyricsSourceKind::Sidecar;
                    metadata.lyrics_format = Some(format);
                    lyrics_source_path = Some(source_path);
                }

                let cover_path = crate::domain::library::assets::find_cover(path);
                self.repository
                    .upsert_song(
                        path,
                        &metadata,
                        lyrics_source_path.as_deref(),
                        cover_path.as_deref(),
                    )
                    .await?;
            }
        }

        Ok(())
    }

    /// 列出库中的歌曲摘要。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回
    /// - `MeloResult<Vec<SongRecord>>`：歌曲列表
    pub async fn list_songs(&self) -> MeloResult<Vec<SongRecord>> {
        self.repository.list_songs().await
    }

    /// 查询某首歌的封面引用。
    ///
    /// # 参数
    /// - `song_id`：歌曲 ID
    ///
    /// # 返回
    /// - `MeloResult<Option<ArtworkRefRecord>>`：封面引用记录
    pub async fn artwork_for_song(&self, song_id: i64) -> MeloResult<Option<ArtworkRefRecord>> {
        self.repository.artwork_for_song(song_id).await
    }
}

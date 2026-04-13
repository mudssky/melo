use crate::core::config::settings::Settings;
use crate::core::error::MeloResult;
use crate::core::model::player::QueueItem;
use crate::domain::library::repository::{ArtworkRefRecord, LibraryRepository, SongRecord};
use crate::domain::playlist::query::SmartQuery;
use crate::domain::playlist::repository::{PlaylistRepository, StoredPlaylist};

/// 统一后的歌单摘要。
#[derive(Debug, Clone)]
pub struct PlaylistSummary {
    /// 歌单名称。
    pub name: String,
    /// 歌单类型。
    pub kind: String,
    /// 歌曲数。
    pub count: usize,
}

/// 歌单服务，统一处理 static 与 smart 两种来源。
#[derive(Clone)]
pub struct PlaylistService {
    settings: Settings,
    repository: PlaylistRepository,
    library_repository: LibraryRepository,
}

impl PlaylistService {
    /// 创建新的歌单服务。
    ///
    /// # 参数
    /// - `settings`：全局配置
    ///
    /// # 返回
    /// - `Self`：歌单服务
    pub fn new(settings: Settings) -> Self {
        let repository = PlaylistRepository::new(settings.clone());
        let library_repository = LibraryRepository::new(settings.clone());
        Self {
            settings,
            repository,
            library_repository,
        }
    }

    fn current_settings(&self) -> MeloResult<Settings> {
        match Settings::load() {
            Ok(settings) => Ok(settings),
            Err(_) => Ok(self.settings.clone()),
        }
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
        self.repository.create_static(name, description).await
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
        self.repository.add_songs(name, song_ids).await
    }

    /// 复用或创建临时歌单。
    ///
    /// # 参数
    /// - `name`：歌单显示名
    /// - `source_kind`：来源类型
    /// - `source_key`：来源唯一键
    /// - `visible`：是否在常规列表中可见
    /// - `expires_at`：可选过期时间
    /// - `song_ids`：歌单成员歌曲 ID 列表
    ///
    /// # 返回
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
        self.repository
            .upsert_ephemeral(name, source_kind, source_key, visible, expires_at, song_ids)
            .await
    }

    /// 列出 static + visible ephemeral + smart 统一视图。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回
    /// - `MeloResult<Vec<PlaylistSummary>>`：统一歌单摘要
    pub async fn list_all(&self) -> MeloResult<Vec<PlaylistSummary>> {
        let mut playlists = self
            .repository
            .list_visible()
            .await?
            .into_iter()
            .map(|playlist| PlaylistSummary {
                name: playlist.name,
                kind: playlist.kind,
                count: playlist.count,
            })
            .collect::<Vec<_>>();

        let settings = self.current_settings()?;
        for (name, definition) in settings.playlists.smart {
            let query = SmartQuery::parse(&definition.query)?;
            playlists.push(PlaylistSummary {
                name,
                kind: "smart".to_string(),
                count: self.library_repository.count_by_query(&query).await?,
            });
        }

        Ok(playlists)
    }

    /// 列出所有在常规列表中可见的已持久化歌单。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回
    /// - `MeloResult<Vec<PlaylistSummary>>`：可见歌单摘要
    pub async fn list_visible(&self) -> MeloResult<Vec<PlaylistSummary>> {
        self.repository.list_visible().await.map(|playlists| {
            playlists
                .into_iter()
                .map(|playlist| PlaylistSummary {
                    name: playlist.name,
                    kind: playlist.kind,
                    count: playlist.count,
                })
                .collect()
        })
    }

    /// 将临时歌单提升为正式静态歌单。
    ///
    /// # 参数
    /// - `source_key`：来源唯一键
    /// - `new_name`：新的静态歌单名
    ///
    /// # 返回
    /// - `MeloResult<()>`：提升结果
    pub async fn promote_ephemeral(&self, source_key: &str, new_name: &str) -> MeloResult<()> {
        self.repository
            .promote_ephemeral(source_key, new_name)
            .await
    }

    /// 清理已经过期的临时歌单。
    ///
    /// # 参数
    /// - `now_text`：当前时间文本
    ///
    /// # 返回
    /// - `MeloResult<u64>`：删除数量
    pub async fn cleanup_expired(&self, now_text: &str) -> MeloResult<u64> {
        self.repository.cleanup_expired(now_text).await
    }

    /// 预览歌单内容。
    ///
    /// # 参数
    /// - `name`：歌单名
    ///
    /// # 返回
    /// - `MeloResult<Vec<SongRecord>>`：歌单歌曲列表
    pub async fn preview(&self, name: &str) -> MeloResult<Vec<SongRecord>> {
        let settings = self.current_settings()?;
        if let Some(definition) = settings.playlists.smart.get(name) {
            let query = SmartQuery::parse(&definition.query)?;
            return self.library_repository.list_by_query(&query).await;
        }

        self.repository.preview_static(name).await
    }

    /// 按歌曲 ID 读取一条歌曲记录。
    ///
    /// # 参数
    /// - `song_id`：歌曲 ID
    ///
    /// # 返回
    /// - `MeloResult<Option<SongRecord>>`：命中的歌曲记录
    pub async fn song_record(&self, song_id: i64) -> MeloResult<Option<SongRecord>> {
        Ok(self
            .library_repository
            .list_songs()
            .await?
            .into_iter()
            .find(|song| song.id == song_id))
    }

    /// 按歌曲 ID 读取封面引用。
    ///
    /// # 参数
    /// - `song_id`：歌曲 ID
    ///
    /// # 返回
    /// - `MeloResult<Option<ArtworkRefRecord>>`：封面引用记录
    pub async fn artwork_for_song(&self, song_id: i64) -> MeloResult<Option<ArtworkRefRecord>> {
        self.library_repository.artwork_for_song(song_id).await
    }

    /// 将歌单内容转换成播放器队列项。
    ///
    /// # 参数
    /// - `name`：歌单名
    ///
    /// # 返回
    /// - `MeloResult<Vec<QueueItem>>`：播放器队列项列表
    pub async fn queue_items(&self, name: &str) -> MeloResult<Vec<QueueItem>> {
        let settings = self.current_settings()?;
        let song_ids = if let Some(definition) = settings.playlists.smart.get(name) {
            let query = SmartQuery::parse(&definition.query)?;
            self.library_repository
                .list_by_query(&query)
                .await?
                .into_iter()
                .map(|song| song.id)
                .collect::<Vec<_>>()
        } else {
            self.repository
                .preview_static(name)
                .await?
                .into_iter()
                .map(|song| song.id)
                .collect::<Vec<_>>()
        };

        self.library_repository
            .queue_items_by_song_ids(&song_ids)
            .await
    }
}

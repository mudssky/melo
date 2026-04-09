use crate::core::config::settings::Settings;
use crate::core::error::MeloResult;
use crate::domain::library::repository::{LibraryRepository, SongRecord};
use crate::domain::playlist::query::SmartQuery;
use crate::domain::playlist::repository::PlaylistRepository;

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

    /// 列出 static + smart 统一视图。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回
    /// - `MeloResult<Vec<PlaylistSummary>>`：统一歌单摘要
    pub async fn list_all(&self) -> MeloResult<Vec<PlaylistSummary>> {
        let mut playlists = self
            .repository
            .list_static()
            .await?
            .into_iter()
            .map(|playlist| PlaylistSummary {
                name: playlist.name,
                kind: "static".to_string(),
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
}

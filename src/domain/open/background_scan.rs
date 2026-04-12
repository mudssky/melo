use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use futures_util::stream::{self, StreamExt};

use crate::core::config::settings::Settings;
use crate::core::error::{MeloError, MeloResult};
use crate::daemon::tasks::RuntimeTaskHandle;
use crate::domain::library::service::LibraryService;
use crate::domain::player::service::PlayerService;
use crate::domain::playlist::service::PlaylistService;

/// 目录直开后台补扫协调器。
///
/// 它负责并发读取剩余文件，但始终按发现顺序提交到歌单和播放队列里，
/// 这样前台可以更快进入 TUI，同时又不会打乱用户看到的曲目顺序。
#[derive(Clone)]
pub(crate) struct BackgroundScanCoordinator {
    settings: Settings,
    library: LibraryService,
    playlists: PlaylistService,
    player: Arc<PlayerService>,
}

/// 后台补扫任务所需的静态上下文。
pub(crate) struct BackgroundScanRequest {
    pub(crate) source_name: String,
    pub(crate) source_kind: String,
    pub(crate) source_key: String,
    pub(crate) visible: bool,
    pub(crate) expires_at: Option<String>,
    pub(crate) initial_song_ids: Vec<i64>,
    pub(crate) remaining_paths: Vec<PathBuf>,
}

impl BackgroundScanCoordinator {
    /// 创建新的后台补扫协调器。
    ///
    /// # 参数
    /// - `settings`：全局配置
    /// - `library`：媒体库服务
    /// - `playlists`：歌单服务
    /// - `player`：播放器服务
    ///
    /// # 返回值
    /// - `Self`：后台补扫协调器
    pub(crate) fn new(
        settings: Settings,
        library: LibraryService,
        playlists: PlaylistService,
        player: Arc<PlayerService>,
    ) -> Self {
        Self {
            settings,
            library,
            playlists,
            player,
        }
    }

    /// 启动后台补扫任务。
    ///
    /// # 参数
    /// - `task`：运行时任务句柄
    /// - `request`：后台补扫任务的静态上下文
    ///
    /// # 返回值
    /// - 无
    pub(crate) fn spawn(&self, task: RuntimeTaskHandle, request: BackgroundScanRequest) {
        let this = self.clone();
        tokio::spawn(async move {
            if let Err(err) = this.run(task.clone(), request).await {
                task.mark_failed(err.to_string());
            }
        });
    }

    /// 执行后台补扫主流程。
    ///
    /// # 参数
    /// - `task`：运行时任务句柄
    /// - `request`：后台补扫任务的静态上下文
    ///
    /// # 返回值
    /// - `MeloResult<()>`：执行结果
    async fn run(&self, task: RuntimeTaskHandle, request: BackgroundScanRequest) -> MeloResult<()> {
        let BackgroundScanRequest {
            source_name,
            source_kind,
            source_key,
            visible,
            expires_at,
            initial_song_ids,
            remaining_paths,
        } = request;
        let mut all_song_ids = initial_song_ids;
        let mut next_index = 0usize;
        let mut ready = BTreeMap::new();

        let mut stream = stream::iter(remaining_paths.into_iter().enumerate())
            .map(|(index, path)| {
                let library = self.library.clone();
                async move {
                    let song_id = library.ensure_song_id_for_path(&path).await?;
                    Ok::<_, crate::core::error::MeloError>((index, path, song_id))
                }
            })
            .buffer_unordered(self.settings.open.background_jobs.max(1));

        while let Some(result) = stream.next().await {
            let (index, path, song_id) = result?;
            ready.insert(index, (path, song_id));

            while let Some((path, song_id)) = ready.remove(&next_index) {
                all_song_ids.push(song_id);
                let current_item_name = path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .map(ToString::to_string);

                self.playlists
                    .upsert_ephemeral(
                        &source_name,
                        &source_kind,
                        &source_key,
                        visible,
                        expires_at.as_deref(),
                        &all_song_ids,
                    )
                    .await?;

                let queue_item = self
                    .library
                    .queue_items_for_song_ids(&[song_id])
                    .await?
                    .into_iter()
                    .next()
                    .ok_or_else(|| {
                        MeloError::Message(format!("后台补扫未能为歌曲生成队列项: {song_id}"))
                    })?;
                self.player.append(queue_item).await?;
                task.mark_indexing(all_song_ids.len(), all_song_ids.len(), current_item_name);
                next_index += 1;
            }
        }

        task.mark_completed(all_song_ids.len());
        Ok(())
    }
}

use std::path::{Path, PathBuf};
use std::sync::Arc;
use utoipa::ToSchema;

use crate::core::config::settings::Settings;
use crate::core::error::{MeloError, MeloResult};
use crate::core::model::player::PlayerSnapshot;
use crate::daemon::tasks::RuntimeTaskStore;
use crate::domain::library::service::LibraryService;
use crate::domain::player::service::PlayerService;
use crate::domain::playlist::service::PlaylistService;

/// 直接打开时识别出来的目标类型。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpenTarget {
    /// 单个音频文件。
    AudioFile(PathBuf),
    /// 目录。
    Directory(PathBuf),
}

/// daemon 直接打开请求。
#[derive(Debug, Clone, serde::Deserialize, ToSchema)]
pub struct OpenRequest {
    /// 用户传入的目标路径。
    pub target: String,
    /// 触发模式，例如 `path_file` / `path_dir` / `cwd_dir`。
    pub mode: String,
}

/// daemon 直接打开响应。
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, ToSchema)]
pub struct OpenResponse {
    /// 最新播放器快照。
    pub snapshot: PlayerSnapshot,
    /// 实际复用或创建的歌单名。
    pub playlist_name: String,
    /// 启动上下文标签。
    pub source_label: String,
}

/// 直接打开领域服务，负责把路径目标转成真实的播放上下文。
pub struct OpenService {
    settings: Settings,
    library: LibraryService,
    playlists: PlaylistService,
    player: Arc<PlayerService>,
    tasks: Arc<RuntimeTaskStore>,
}

impl OpenService {
    /// 创建新的直接打开服务。
    ///
    /// # 参数
    /// - `settings`：全局配置
    /// - `library`：媒体库服务
    /// - `playlists`：歌单服务
    /// - `player`：播放器服务
    /// - `tasks`：运行时任务存储
    ///
    /// # 返回值
    /// - `Self`：直接打开服务
    pub fn new(
        settings: Settings,
        library: LibraryService,
        playlists: PlaylistService,
        player: Arc<PlayerService>,
        tasks: Arc<RuntimeTaskStore>,
    ) -> Self {
        Self {
            settings,
            library,
            playlists,
            player,
            tasks,
        }
    }

    /// 执行一次直接打开，将目标转成队列并触发播放。
    ///
    /// # 参数
    /// - `request`：直接打开请求
    ///
    /// # 返回值
    /// - `MeloResult<OpenResponse>`：打开结果
    pub async fn open(&self, request: OpenRequest) -> MeloResult<OpenResponse> {
        match classify_target(Path::new(&request.target))? {
            OpenTarget::AudioFile(path) => self.open_audio_file_target(request, path).await,
            OpenTarget::Directory(path) => self.open_directory_target(&request, path).await,
        }
    }

    /// 按触发模式决定临时歌单是否可见。
    ///
    /// # 参数
    /// - `mode`：触发模式
    ///
    /// # 返回值
    /// - `bool`：是否可见
    fn playlist_visibility(&self, mode: &str) -> bool {
        match mode {
            "path_file" => self.settings.playlists.ephemeral.visibility.path_file,
            "path_dir" => self.settings.playlists.ephemeral.visibility.path_dir,
            "cwd_dir" => self.settings.playlists.ephemeral.visibility.cwd_dir,
            _ => false,
        }
    }

    /// 计算临时歌单的过期时间文本。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - `Option<String>`：过期时间文本，`None` 表示永不过期
    fn expires_at(&self) -> Option<String> {
        let ttl = self.settings.playlists.ephemeral.default_ttl_seconds;
        if ttl == 0 {
            return None;
        }

        crate::core::db::now_text()
            .parse::<u64>()
            .ok()
            .map(|now| now.saturating_add(ttl).to_string())
    }

    /// 处理单文件直接打开。
    ///
    /// # 参数
    /// - `request`：直接打开请求
    /// - `path`：目标音频文件路径
    ///
    /// # 返回值
    /// - `MeloResult<OpenResponse>`：打开结果
    async fn open_audio_file_target(
        &self,
        request: OpenRequest,
        path: PathBuf,
    ) -> MeloResult<OpenResponse> {
        let song_ids = vec![self.library.ensure_song_id_for_path(&path).await?];
        let expires_at = self.expires_at();
        let playlist = self
            .playlists
            .upsert_ephemeral(
                &request.target,
                &request.mode,
                &request.target,
                self.playlist_visibility(&request.mode),
                expires_at.as_deref(),
                &song_ids,
            )
            .await?;

        self.player.clear().await?;
        for item in self.library.queue_items_for_song_ids(&song_ids).await? {
            self.player.append(item).await?;
        }
        let snapshot = self.player.play().await?;

        Ok(OpenResponse {
            snapshot,
            playlist_name: playlist.name,
            source_label: request.target,
        })
    }

    /// 处理目录直接打开，先做同步预热，再把剩余路径交给后台补扫。
    ///
    /// # 参数
    /// - `request`：直接打开请求
    /// - `path`：目标目录路径
    ///
    /// # 返回值
    /// - `MeloResult<OpenResponse>`：打开结果
    async fn open_directory_target(
        &self,
        request: &OpenRequest,
        path: PathBuf,
    ) -> MeloResult<OpenResponse> {
        let audio_paths = discover_audio_paths(&path, self.settings.open.max_depth)?;
        if audio_paths.is_empty() {
            return Err(MeloError::Message("open_target_empty".to_string()));
        }

        let task = self
            .tasks
            .start_scan(request.target.clone(), audio_paths.len());
        let split_at = self
            .settings
            .open
            .prewarm_limit
            .min(audio_paths.len())
            .max(1);
        let mut song_ids = Vec::with_capacity(split_at);

        for audio_path in &audio_paths[..split_at] {
            let current_item_name = audio_path
                .file_name()
                .and_then(|name| name.to_str())
                .map(ToString::to_string);
            task.mark_prewarming(current_item_name);
            song_ids.push(self.library.ensure_song_id_for_path(audio_path).await?);
        }

        let expires_at = self.expires_at();
        let playlist = self
            .playlists
            .upsert_ephemeral(
                &request.target,
                &request.mode,
                &request.target,
                self.playlist_visibility(&request.mode),
                expires_at.as_deref(),
                &song_ids,
            )
            .await?;

        self.player.clear().await?;
        for item in self.library.queue_items_for_song_ids(&song_ids).await? {
            self.player.append(item).await?;
        }
        let snapshot = self.player.play().await?;

        let remaining_paths = audio_paths[split_at..].to_vec();
        if remaining_paths.is_empty() {
            task.mark_completed(song_ids.len());
        } else {
            crate::domain::open::background_scan::BackgroundScanCoordinator::new(
                self.settings.clone(),
                self.library.clone(),
                self.playlists.clone(),
                Arc::clone(&self.player),
            )
            .spawn(
                task,
                crate::domain::open::background_scan::BackgroundScanRequest {
                    source_name: request.target.clone(),
                    source_kind: request.mode.clone(),
                    source_key: request.target.clone(),
                    visible: self.playlist_visibility(&request.mode),
                    expires_at,
                    initial_song_ids: song_ids.clone(),
                    remaining_paths,
                },
            );
        }

        Ok(OpenResponse {
            snapshot,
            playlist_name: playlist.name,
            source_label: request.target.clone(),
        })
    }
}

/// 将传入路径分类为可打开目标。
///
/// # 参数
/// - `path`：用户传入的目标路径
///
/// # 返回值
/// - `MeloResult<OpenTarget>`：识别后的目标类型
pub fn classify_target(path: &Path) -> MeloResult<OpenTarget> {
    if path.is_dir() {
        return Ok(OpenTarget::Directory(path.to_path_buf()));
    }

    if crate::domain::open::formats::is_supported_audio_path(path) {
        return Ok(OpenTarget::AudioFile(path.to_path_buf()));
    }

    Err(MeloError::Message("unsupported_open_format".to_string()))
}

/// 发现目录内的音频文件路径。
///
/// # 参数
/// - `root`：扫描根目录
/// - `max_depth`：最大递归深度
///
/// # 返回值
/// - `MeloResult<Vec<PathBuf>>`：发现到的音频路径列表
pub fn discover_audio_paths(root: &Path, max_depth: usize) -> MeloResult<Vec<PathBuf>> {
    let mut paths = walkdir::WalkDir::new(root)
        .max_depth(max_depth.saturating_add(1))
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
        .map(|entry| entry.into_path())
        .filter(|path| crate::domain::open::formats::is_supported_audio_path(path))
        .collect::<Vec<_>>();
    paths.sort();
    Ok(paths)
}

#[cfg(test)]
mod tests;

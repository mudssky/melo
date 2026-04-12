/// 裸 `melo` 启动时的默认决策。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DefaultLaunchDecision {
    /// 保留当前正在播放的会话，只把调用目录作为启动上下文传给 TUI。
    PreserveCurrentSession {
        /// 调用方 shell 的当前目录。
        launch_cwd: String,
        /// 当前播放来源对应的歌单名。
        playlist_name: String,
    },
    /// 打开调用方 shell 的当前目录。
    OpenLaunchCwd {
        /// 调用方 shell 的当前目录。
        launch_cwd: String,
    },
}

/// 根据当前播放快照和调用目录决定裸启动语义。
///
/// # 参数
/// - `launch_cwd`：调用方 shell 当前目录
/// - `snapshot`：daemon 当前 TUI 首页聚合快照
///
/// # 返回值
/// - `DefaultLaunchDecision`：裸启动时应执行的默认行为
pub fn choose_default_launch_decision(
    launch_cwd: &std::path::Path,
    snapshot: &crate::core::model::tui::TuiSnapshot,
) -> DefaultLaunchDecision {
    let launch_cwd = launch_cwd.to_string_lossy().into_owned();
    let is_playing = snapshot.player.playback_state
        == crate::core::model::player::PlaybackState::Playing.as_str();

    if is_playing && let Some(current) = snapshot.playlist_browser.current_playing_playlist.as_ref()
    {
        return DefaultLaunchDecision::PreserveCurrentSession {
            launch_cwd,
            playlist_name: current.name.clone(),
        };
    }

    DefaultLaunchDecision::OpenLaunchCwd { launch_cwd }
}

#[cfg(test)]
mod tests;

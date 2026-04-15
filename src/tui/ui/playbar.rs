use crate::core::model::player::PlayerSnapshot;

/// 将秒数格式化为 `MM:SS`。
///
/// # 参数
/// - `seconds`：待格式化的秒数
///
/// # 返回值
/// - `String`：格式化后的时间字符串
fn format_mmss(seconds: f64) -> String {
    let rounded = seconds.floor() as u64;
    let minutes = rounded / 60;
    let secs = rounded % 60;
    format!("{minutes:02}:{secs:02}")
}

/// 生成播放栏文案。
///
/// # 参数
/// - `snapshot`：播放器快照
///
/// # 返回值
/// - `String`：播放栏显示文本
pub fn playback_label(snapshot: &PlayerSnapshot) -> String {
    let title = snapshot
        .current_song
        .as_ref()
        .map(|song| song.title.as_str())
        .unwrap_or("Nothing Playing");
    let progress = match (
        snapshot.position_seconds,
        snapshot
            .current_song
            .as_ref()
            .and_then(|song| song.duration_seconds),
    ) {
        (Some(position), Some(duration)) => {
            format!("{} / {}", format_mmss(position), format_mmss(duration))
        }
        (Some(position), None) => format!("{} / --:--", format_mmss(position)),
        _ => "--:-- / --:--".to_string(),
    };

    format!("{} | {} | {}", snapshot.playback_state, progress, title)
}

/// 为完整 TUI 状态生成播放栏文案。
///
/// # 参数
/// - `app`：当前 TUI 状态
/// - `now`：当前本地单调时钟时间
///
/// # 返回值
/// - `String`：播放栏显示文本
pub fn playback_label_at(app: &crate::tui::app::App, now: std::time::Instant) -> String {
    let title = app
        .player
        .current_song
        .as_ref()
        .map(|song| song.title.as_str())
        .unwrap_or("Nothing Playing");
    let progress = match (
        app.playback_clock().display_position(now),
        app.runtime.duration_seconds.or_else(|| {
            app.player
                .current_song
                .as_ref()
                .and_then(|song| song.duration_seconds)
        }),
    ) {
        (Some(position), Some(duration)) => {
            format!("{} / {}", format_mmss(position), format_mmss(duration))
        }
        (Some(position), None) => format!("{} / --:--", format_mmss(position)),
        _ => "--:-- / --:--".to_string(),
    };

    let pending = if app.pending_playlist_play().is_some() {
        " | pending"
    } else {
        ""
    };

    format!(
        "{} | {} | {}{}",
        app.runtime.playback_state, progress, title, pending
    )
}

/// 生成右侧详情区要展示的文本行。
///
/// # 参数
/// - `app`：当前 TUI 状态
///
/// # 返回值
/// - `Vec<String>`：详情区文本行
pub fn render_detail_lines(app: &crate::tui::app::App) -> Vec<String> {
    render_detail_lines_at(app, std::time::Instant::now())
}

/// 在指定时刻生成右侧详情区要展示的文本行。
///
/// # 参数
/// - `app`：当前 TUI 状态
/// - `now`：当前本地单调时钟时间
///
/// # 返回值
/// - `Vec<String>`：详情区文本行
pub fn render_detail_lines_at(app: &crate::tui::app::App, now: std::time::Instant) -> Vec<String> {
    let mut lines = Vec::new();

    if let Some(song) = app.player.current_song.as_ref() {
        lines.push(format!("当前曲目：{}", song.title));
    } else {
        lines.push("当前曲目：无".to_string());
    }

    lines.push(String::new());
    if let Some(song_id) = app.current_track_song_id
        && let Some(content) = app.track_content_cache.get(&song_id)
    {
        let visible_height = if app.lyric_viewport.visible_height <= 1 {
            content.lyrics.len().max(1)
        } else {
            app.lyric_viewport.visible_height
        };
        let start = app
            .lyric_viewport
            .scroll_top
            .min(content.lyrics.len().saturating_sub(visible_height));
        let end = (start + visible_height).min(content.lyrics.len());
        let current_index = app.current_lyric_index_at(now);

        lines.extend(
            content.lyrics[start..end]
                .iter()
                .enumerate()
                .map(|(offset, lyric)| {
                    let actual_index = start + offset;
                    let prefix = if current_index == Some(actual_index) {
                        "[current]"
                    } else {
                        "         "
                    };
                    format!("{prefix} {} │", lyric.text)
                }),
        );
    } else if let Some(lyrics) = &app.current_track_lyrics {
        lines.extend(lyrics.lines().take(6).map(ToString::to_string));
    } else {
        lines.push("No lyrics available".to_string());
    }

    lines.push(String::new());
    let cover_summary = app
        .current_track_song_id
        .and_then(|song_id| app.track_content_cache.get(&song_id))
        .and_then(|content| content.artwork.as_ref())
        .map(|artwork| artwork.terminal_summary.clone())
        .or_else(|| app.current_track_cover_summary.clone())
        .unwrap_or_else(|| "No cover available".to_string());
    lines.push(cover_summary);

    lines
}

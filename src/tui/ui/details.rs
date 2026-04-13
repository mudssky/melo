/// 生成右侧详情区要展示的文本行。
///
/// # 参数
/// - `app`：当前 TUI 状态
///
/// # 返回值
/// - `Vec<String>`：详情区文本行
pub fn render_detail_lines(app: &crate::tui::app::App) -> Vec<String> {
    let mut lines = Vec::new();

    if let Some(song) = app.player.current_song.as_ref() {
        lines.push(format!("当前曲目：{}", song.title));
    } else {
        lines.push("当前曲目：无".to_string());
    }

    lines.push(String::new());
    if let Some(lyrics) = &app.current_track_lyrics {
        lines.extend(lyrics.lines().take(6).map(ToString::to_string));
    } else {
        lines.push("No lyrics available".to_string());
    }

    lines.push(String::new());
    lines.push(
        app.current_track_cover_summary
            .clone()
            .unwrap_or_else(|| "No cover available".to_string()),
    );

    lines
}

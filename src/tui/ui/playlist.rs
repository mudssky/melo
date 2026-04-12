/// 渲染左侧歌单列表区域的文本行。
///
/// # 参数
/// - `app`：当前 TUI 状态
///
/// # 返回值
/// - `Vec<String>`：可直接显示的文本行
pub fn render_playlist_lines(app: &crate::tui::app::App) -> Vec<String> {
    let mut lines = Vec::new();
    if let Some(current) = &app.playlist_browser.current_playing_playlist {
        lines.push("当前播放来源".to_string());
        lines.push(format!("> {} ({})", current.name, current.kind));
        lines.push(String::new());
    }
    lines.push("播放列表".to_string());

    for playlist in &app.playlist_browser.visible_playlists {
        let marker = if app.selected_playlist_name() == Some(playlist.name.as_str()) {
            ">"
        } else {
            " "
        };
        lines.push(format!("{marker} {} ({})", playlist.name, playlist.count));
    }

    lines
}

/// 渲染播放状态摘要区域的文本行。
///
/// # 参数
/// - `app`：当前 TUI 状态
///
/// # 返回值
/// - `Vec<String>`：状态文本行
pub fn render_status_lines(app: &crate::tui::app::App) -> Vec<String> {
    vec![
        format!(
            "当前播放列表：{}",
            app.playlist_browser
                .current_playing_playlist
                .as_ref()
                .map(|playlist| playlist.name.as_str())
                .unwrap_or("无")
        ),
        format!("repeat={}", app.player.repeat_mode),
        format!("shuffle={}", app.player.shuffle_enabled),
    ]
}

/// 渲染歌单预览区域的文本行。
///
/// # 参数
/// - `app`：当前 TUI 状态
///
/// # 返回值
/// - `Vec<String>`：预览文本行
pub fn render_preview_lines(app: &crate::tui::app::App) -> Vec<String> {
    if app.preview_loading {
        return vec!["加载中...".to_string()];
    }
    if let Some(error) = &app.preview_error {
        return vec![format!("ERR {error}")];
    }
    if app.preview_titles.is_empty() {
        return vec!["暂无歌曲".to_string()];
    }

    app.preview_titles
        .iter()
        .enumerate()
        .map(|(index, title)| {
            if index == app.selected_preview_index {
                format!("> {title}")
            } else {
                format!("  {title}")
            }
        })
        .collect()
}

/// 根据点击行号计算歌单列表索引。
///
/// # 参数
/// - `area`：歌单面板区域
/// - `row`：鼠标所在终端行
/// - `item_count`：可见歌单数量
///
/// # 返回值
/// - `Option<usize>`：命中的歌单索引
pub fn playlist_index_at(
    area: ratatui::layout::Rect,
    row: u16,
    item_count: usize,
) -> Option<usize> {
    let start = area.y.saturating_add(2);
    if row < start {
        return None;
    }
    let index = (row - start) as usize;
    (index < item_count).then_some(index)
}

/// 根据点击行号计算预览列表索引。
///
/// # 参数
/// - `area`：预览面板区域
/// - `row`：鼠标所在终端行
/// - `item_count`：当前预览项目数量
///
/// # 返回值
/// - `Option<usize>`：命中的预览索引
pub fn preview_index_at(area: ratatui::layout::Rect, row: u16, item_count: usize) -> Option<usize> {
    let start = area.y.saturating_add(1);
    if row < start {
        return None;
    }
    let index = (row - start) as usize;
    (index < item_count).then_some(index)
}

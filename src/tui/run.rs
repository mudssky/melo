use std::io::{self, Stdout};
use std::time::Duration;

use crossterm::event::{
    DisableMouseCapture, EnableMouseCapture, Event, KeyEventKind, MouseEventKind,
};
use crossterm::terminal::{EnterAlternateScreen, LeaveAlternateScreen};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

use crate::core::error::{MeloError, MeloResult};

const PAGE_STEP: isize = 10;

/// TUI 启动时需要带入的上下文。
pub struct LaunchContext {
    /// 调用方 shell 的当前目录。
    pub launch_cwd: Option<String>,
    /// 启动来源标签。
    pub source_label: Option<String>,
    /// 一次性启动提示。
    pub startup_notice: Option<String>,
    /// 是否显示底部快捷键提示。
    pub footer_hints_enabled: bool,
}

/// 计算下一个循环模式。
///
/// # 参数
/// - `current`：当前循环模式
///
/// # 返回值
/// - `&'static str`：下一个循环模式
pub(crate) fn next_repeat_mode(current: &str) -> &'static str {
    match current {
        "off" => "all",
        "all" => "one",
        _ => "off",
    }
}

/// 启动真实的 TUI 运行循环。
///
/// # 参数
/// - `base_url`：daemon HTTP 基地址
/// - `context`：启动上下文
///
/// # 返回值
/// - `MeloResult<()>`：运行结果
pub async fn start(base_url: String, context: LaunchContext) -> MeloResult<()> {
    let settings = crate::core::config::settings::Settings::load().unwrap_or_default();
    let mouse_enabled = settings.tui.mouse_enabled;

    crossterm::terminal::enable_raw_mode().map_err(|err| MeloError::Message(err.to_string()))?;
    let mut stdout = io::stdout();
    if mouse_enabled {
        crossterm::execute!(stdout, EnterAlternateScreen, EnableMouseCapture)
            .map_err(|err| MeloError::Message(err.to_string()))?;
    } else {
        crossterm::execute!(stdout, EnterAlternateScreen)
            .map_err(|err| MeloError::Message(err.to_string()))?;
    }

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).map_err(|err| MeloError::Message(err.to_string()))?;

    let result = run_loop(&mut terminal, base_url, context).await;

    let _ = crossterm::terminal::disable_raw_mode();
    if mouse_enabled {
        let _ = crossterm::execute!(
            terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        );
    } else {
        let _ = crossterm::execute!(terminal.backend_mut(), LeaveAlternateScreen);
    }
    let _ = terminal.show_cursor();
    result
}

/// 驱动 TUI 的主循环。
///
/// # 参数
/// - `terminal`：已初始化的终端
/// - `base_url`：daemon HTTP 基地址
/// - `context`：启动上下文
///
/// # 返回值
/// - `MeloResult<()>`：循环结果
async fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    base_url: String,
    context: LaunchContext,
) -> MeloResult<()> {
    let settings = crate::core::config::settings::Settings::load().unwrap_or_default();
    let renderer = crate::core::runtime_templates::RuntimeTemplateRenderer::default();
    let theme = crate::tui::theme::Theme::default();
    let mut keymap = crate::tui::keymap::Keymap::from_settings(&settings.tui.keymap)?;
    let mut click_tracker = crate::tui::mouse::ClickTracker::default();
    let api_client = crate::cli::client::ApiClient::new(base_url.clone());
    let client = crate::tui::client::TuiClient::new(base_url);
    let mut app = crate::tui::app::App::new_for_test();
    let home = api_client.tui_home().await?;
    app.apply_tui_snapshot(home);
    if let Some(selected) = app.selected_playlist_name().map(ToString::to_string) {
        app.set_playlist_preview_loading();
        match api_client.playlist_preview(&selected).await {
            Ok(preview) => app.set_playlist_preview(&preview),
            Err(err) => app.set_playlist_preview_error(err.to_string()),
        }
    }
    app.footer_hints_enabled = context.footer_hints_enabled;
    app.startup_notice = context.startup_notice;
    if let Some(launch_cwd) = context.launch_cwd {
        app.set_launch_cwd(launch_cwd);
    }
    if let Some(source_label) = context.source_label {
        app.set_source_label(source_label);
    }

    let mut stream = client.connect().await?;
    let (snapshot_tx, mut snapshot_rx) = tokio::sync::mpsc::unbounded_channel();
    tokio::spawn(async move {
        while let Ok(snapshot) = stream
            .next_json::<crate::core::model::tui::TuiSnapshot>()
            .await
        {
            if snapshot_tx.send(snapshot).is_err() {
                break;
            }
        }
    });

    loop {
        while let Ok(snapshot) = snapshot_rx.try_recv() {
            app.apply_tui_snapshot(snapshot);
        }

        terminal
            .draw(|frame| {
                let layout = app.layout(frame.area());
                let status_lines = crate::tui::ui::playlist::render_status_lines(&app).join("\n");
                let preview_lines = crate::tui::ui::playlist::render_preview_lines(&app).join("\n");
                let preview_border_style =
                    if app.focus == crate::tui::app::FocusArea::PlaylistPreview {
                        theme.focused_border
                    } else {
                        theme.pane_border
                    };

                if let Some(task_area) = layout.task_bar
                    && let Some(text) =
                        app.task_bar_text(&renderer, &settings, task_area.width as usize)
                {
                    frame.render_widget(Paragraph::new(text), task_area);
                }

                crate::tui::ui::playlist::render_playlist_widget(
                    frame,
                    layout.sidebar,
                    &mut app,
                    theme,
                );
                frame.render_widget(
                    Paragraph::new(status_lines).block(
                        Block::default()
                            .borders(Borders::ALL)
                            .title("当前播放来源")
                            .border_style(theme.pane_border),
                    ),
                    layout.content_header,
                );
                frame.render_widget(
                    Paragraph::new(preview_lines).block(
                        Block::default()
                            .borders(Borders::ALL)
                            .title("歌单预览")
                            .border_style(preview_border_style),
                    ),
                    layout.content_body,
                );
                frame.render_widget(
                    Paragraph::new(format!(
                        "{} | {}",
                        crate::tui::ui::playbar::playback_label(&app.player),
                        app.footer_status()
                    ))
                    .block(Block::default().borders(Borders::ALL).title("Status")),
                    layout.playbar,
                );

                if app.show_help {
                    let popup_area = crate::tui::ui::popup::centered_area(frame.area());
                    frame.render_widget(Clear, popup_area);
                    frame.render_widget(
                        Paragraph::new(crate::tui::ui::popup::help_lines_for(&keymap).join("\n"))
                            .block(Block::default().borders(Borders::ALL).title("Help")),
                        popup_area,
                    );
                }
            })
            .map_err(|err| MeloError::Message(err.to_string()))?;

        if crossterm::event::poll(Duration::from_millis(50))
            .map_err(|err| MeloError::Message(err.to_string()))?
        {
            let event =
                crossterm::event::read().map_err(|err| MeloError::Message(err.to_string()))?;
            match event {
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    match keymap.resolve_key(key, std::time::Instant::now()) {
                        crate::tui::keymap::Resolution::Matched(action) => {
                            if handle_key_action(&mut app, action, &api_client).await? {
                                break;
                            }
                        }
                        crate::tui::keymap::Resolution::Pending => {}
                        crate::tui::keymap::Resolution::NoMatch => {}
                    }
                }
                Event::Mouse(mouse) if settings.tui.mouse_enabled && !app.show_help => {
                    let area = terminal
                        .size()
                        .map_err(|err| MeloError::Message(err.to_string()))?;
                    let layout = app.layout(area.into());
                    match mouse.kind {
                        MouseEventKind::Down(_) => {
                            let target =
                                hit_test_mouse_target(layout, &app, mouse.column, mouse.row);
                            if target == crate::tui::mouse::MouseTarget::None {
                                continue;
                            }

                            match click_tracker.classify(target, std::time::Instant::now()) {
                                crate::tui::mouse::ClickKind::Single => {
                                    apply_mouse_selection(&mut app, target, &api_client).await?;
                                }
                                crate::tui::mouse::ClickKind::Double => {
                                    apply_mouse_selection(&mut app, target, &api_client).await?;
                                    if let Some(intent) =
                                        app.handle_action(crate::tui::event::ActionId::Activate)
                                    {
                                        dispatch_intent(&mut app, intent, &api_client).await?;
                                    }
                                }
                            }
                        }
                        MouseEventKind::ScrollUp => {
                            let target =
                                hit_test_mouse_target(layout, &app, mouse.column, mouse.row);
                            let intent = match target {
                                crate::tui::mouse::MouseTarget::PreviewRow(_) => {
                                    crate::tui::event::Intent::ScrollPreview(-1)
                                }
                                crate::tui::mouse::MouseTarget::PlaylistRow(_) => {
                                    crate::tui::event::Intent::ScrollPlaylist(-1)
                                }
                                crate::tui::mouse::MouseTarget::None => match app.focus {
                                    crate::tui::app::FocusArea::PlaylistPreview => {
                                        crate::tui::event::Intent::ScrollPreview(-1)
                                    }
                                    crate::tui::app::FocusArea::PlaylistList => {
                                        crate::tui::event::Intent::ScrollPlaylist(-1)
                                    }
                                },
                            };
                            dispatch_intent(&mut app, intent, &api_client).await?;
                        }
                        MouseEventKind::ScrollDown => {
                            let target =
                                hit_test_mouse_target(layout, &app, mouse.column, mouse.row);
                            let intent = match target {
                                crate::tui::mouse::MouseTarget::PreviewRow(_) => {
                                    crate::tui::event::Intent::ScrollPreview(1)
                                }
                                crate::tui::mouse::MouseTarget::PlaylistRow(_) => {
                                    crate::tui::event::Intent::ScrollPlaylist(1)
                                }
                                crate::tui::mouse::MouseTarget::None => match app.focus {
                                    crate::tui::app::FocusArea::PlaylistPreview => {
                                        crate::tui::event::Intent::ScrollPreview(1)
                                    }
                                    crate::tui::app::FocusArea::PlaylistList => {
                                        crate::tui::event::Intent::ScrollPlaylist(1)
                                    }
                                },
                            };
                            dispatch_intent(&mut app, intent, &api_client).await?;
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }
    }

    Ok(())
}

/// 处理一个来自 keymap 的动作。
///
/// # 参数
/// - `app`：当前 TUI 状态
/// - `action`：命中的动作
/// - `api_client`：daemon API 客户端
///
/// # 返回值
/// - `MeloResult<bool>`：返回 `true` 表示应退出主循环
async fn handle_key_action(
    app: &mut crate::tui::app::App,
    action: crate::tui::event::ActionId,
    api_client: &crate::cli::client::ApiClient,
) -> MeloResult<bool> {
    if app.show_help
        && !matches!(
            action,
            crate::tui::event::ActionId::OpenHelp
                | crate::tui::event::ActionId::Quit
                | crate::tui::event::ActionId::FocusPrev
        )
    {
        return Ok(false);
    }

    match action {
        crate::tui::event::ActionId::OpenHelp => {
            app.show_help = !app.show_help;
            Ok(false)
        }
        crate::tui::event::ActionId::Quit => {
            if app.show_help {
                app.show_help = false;
                Ok(false)
            } else {
                Ok(true)
            }
        }
        crate::tui::event::ActionId::MoveUp => {
            match app.focus {
                crate::tui::app::FocusArea::PlaylistList => {
                    if let Some(intent) = move_playlist_selection(app, -1) {
                        dispatch_intent(app, intent, api_client).await?;
                    }
                }
                crate::tui::app::FocusArea::PlaylistPreview => move_preview_selection(app, -1),
            }
            Ok(false)
        }
        crate::tui::event::ActionId::MoveDown => {
            match app.focus {
                crate::tui::app::FocusArea::PlaylistList => {
                    if let Some(intent) = move_playlist_selection(app, 1) {
                        dispatch_intent(app, intent, api_client).await?;
                    }
                }
                crate::tui::app::FocusArea::PlaylistPreview => move_preview_selection(app, 1),
            }
            Ok(false)
        }
        crate::tui::event::ActionId::JumpTop => {
            match app.focus {
                crate::tui::app::FocusArea::PlaylistList => {
                    if let Some(intent) = jump_playlist_selection(app, 0) {
                        dispatch_intent(app, intent, api_client).await?;
                    }
                }
                crate::tui::app::FocusArea::PlaylistPreview => jump_preview_selection(app, 0),
            }
            Ok(false)
        }
        crate::tui::event::ActionId::JumpBottom => {
            match app.focus {
                crate::tui::app::FocusArea::PlaylistList => {
                    if let Some(intent) = jump_playlist_selection(
                        app,
                        app.playlist_browser
                            .visible_playlists
                            .len()
                            .saturating_sub(1),
                    ) {
                        dispatch_intent(app, intent, api_client).await?;
                    }
                }
                crate::tui::app::FocusArea::PlaylistPreview => {
                    jump_preview_selection(app, app.preview_titles.len().saturating_sub(1));
                }
            }
            Ok(false)
        }
        crate::tui::event::ActionId::PageUp => {
            match app.focus {
                crate::tui::app::FocusArea::PlaylistList => {
                    if let Some(intent) = move_playlist_selection(app, -PAGE_STEP) {
                        dispatch_intent(app, intent, api_client).await?;
                    }
                }
                crate::tui::app::FocusArea::PlaylistPreview => {
                    move_preview_selection(app, -PAGE_STEP);
                }
            }
            Ok(false)
        }
        crate::tui::event::ActionId::PageDown => {
            match app.focus {
                crate::tui::app::FocusArea::PlaylistList => {
                    if let Some(intent) = move_playlist_selection(app, PAGE_STEP) {
                        dispatch_intent(app, intent, api_client).await?;
                    }
                }
                crate::tui::app::FocusArea::PlaylistPreview => {
                    move_preview_selection(app, PAGE_STEP);
                }
            }
            Ok(false)
        }
        crate::tui::event::ActionId::FocusNext
        | crate::tui::event::ActionId::FocusPrev
        | crate::tui::event::ActionId::Activate => {
            if let Some(intent) = app.handle_action(action) {
                dispatch_intent(app, intent, api_client).await?;
            }
            Ok(false)
        }
        crate::tui::event::ActionId::TogglePlayback
        | crate::tui::event::ActionId::Next
        | crate::tui::event::ActionId::Prev
        | crate::tui::event::ActionId::CycleRepeatMode
        | crate::tui::event::ActionId::ToggleShuffle
        | crate::tui::event::ActionId::LoadPreview
        | crate::tui::event::ActionId::PlaySelection
        | crate::tui::event::ActionId::PlayPreviewSelection => {
            dispatch_intent(app, crate::tui::event::Intent::Action(action), api_client).await?;
            Ok(false)
        }
    }
}

/// 按偏移量移动左侧歌单选择。
///
/// # 参数
/// - `app`：当前 TUI 状态
/// - `delta`：选择偏移量
///
/// # 返回值
/// - `Option<crate::tui::event::Intent>`：选择变化时触发的后续意图
fn move_playlist_selection(
    app: &mut crate::tui::app::App,
    delta: isize,
) -> Option<crate::tui::event::Intent> {
    if app.playlist_browser.visible_playlists.is_empty() {
        return None;
    }

    let current_index = app
        .selected_playlist_name()
        .and_then(|selected| {
            app.playlist_browser
                .visible_playlists
                .iter()
                .position(|playlist| playlist.name == selected)
        })
        .unwrap_or(0);
    let next_index = if delta.is_negative() {
        current_index.saturating_sub(delta.unsigned_abs())
    } else {
        (current_index + delta as usize).min(app.playlist_browser.visible_playlists.len() - 1)
    };

    app.select_playlist_index(next_index)
}

/// 跳转左侧歌单选择到指定索引。
///
/// # 参数
/// - `app`：当前 TUI 状态
/// - `index`：目标索引
///
/// # 返回值
/// - `Option<crate::tui::event::Intent>`：选择变化时触发的后续意图
fn jump_playlist_selection(
    app: &mut crate::tui::app::App,
    index: usize,
) -> Option<crate::tui::event::Intent> {
    if app.playlist_browser.visible_playlists.is_empty() {
        return None;
    }

    app.select_playlist_index(index.min(app.playlist_browser.visible_playlists.len() - 1))
}

/// 按偏移量移动右侧预览选择。
///
/// # 参数
/// - `app`：当前 TUI 状态
/// - `delta`：选择偏移量
///
/// # 返回值
/// - 无
fn move_preview_selection(app: &mut crate::tui::app::App, delta: isize) {
    if app.preview_titles.is_empty() {
        return;
    }

    let current = app.selected_preview_index();
    let next = if delta.is_negative() {
        current.saturating_sub(delta.unsigned_abs())
    } else {
        (current + delta as usize).min(app.preview_titles.len() - 1)
    };
    app.select_preview_index(next);
}

/// 跳转右侧预览选择到指定索引。
///
/// # 参数
/// - `app`：当前 TUI 状态
/// - `index`：目标索引
///
/// # 返回值
/// - 无
fn jump_preview_selection(app: &mut crate::tui::app::App, index: usize) {
    if app.preview_titles.is_empty() {
        return;
    }

    app.select_preview_index(index.min(app.preview_titles.len() - 1));
}

/// 执行一个归一化后的输入意图。
///
/// # 参数
/// - `app`：当前 TUI 状态
/// - `intent`：待执行的意图
/// - `api_client`：daemon API 客户端
///
/// # 返回值
/// - `MeloResult<()>`：执行结果
async fn dispatch_intent(
    app: &mut crate::tui::app::App,
    intent: crate::tui::event::Intent,
    api_client: &crate::cli::client::ApiClient,
) -> crate::core::error::MeloResult<()> {
    let mut pending = Some(intent);
    while let Some(current) = pending.take() {
        match current {
            crate::tui::event::Intent::Action(crate::tui::event::ActionId::LoadPreview) => {
                if let Some(name) = app.selected_playlist_name().map(ToString::to_string) {
                    app.set_playlist_preview_loading();
                    match api_client.playlist_preview(&name).await {
                        Ok(preview) => app.set_playlist_preview(&preview),
                        Err(err) => app.set_playlist_preview_error(err.to_string()),
                    }
                }
            }
            crate::tui::event::Intent::Action(crate::tui::event::ActionId::PlaySelection) => {
                if let Some(name) = app.selected_playlist_name().map(ToString::to_string) {
                    let snapshot = api_client.playlist_play(&name, 0).await?;
                    app.apply_tui_snapshot(snapshot);
                }
            }
            crate::tui::event::Intent::Action(
                crate::tui::event::ActionId::PlayPreviewSelection,
            ) => {
                if let Some(name) = app.selected_playlist_name().map(ToString::to_string) {
                    let snapshot = api_client
                        .playlist_play(&name, app.selected_preview_index())
                        .await?;
                    app.apply_tui_snapshot(snapshot);
                }
            }
            crate::tui::event::Intent::Action(crate::tui::event::ActionId::TogglePlayback) => {
                app.apply_snapshot(api_client.post_json("/api/player/toggle").await?);
            }
            crate::tui::event::Intent::Action(crate::tui::event::ActionId::Next) => {
                app.apply_snapshot(api_client.post_json("/api/player/next").await?);
            }
            crate::tui::event::Intent::Action(crate::tui::event::ActionId::Prev) => {
                app.apply_snapshot(api_client.post_json("/api/player/prev").await?);
            }
            crate::tui::event::Intent::Action(crate::tui::event::ActionId::CycleRepeatMode) => {
                app.apply_snapshot(
                    api_client
                        .player_mode_repeat(next_repeat_mode(&app.player.repeat_mode))
                        .await?,
                );
            }
            crate::tui::event::Intent::Action(crate::tui::event::ActionId::ToggleShuffle) => {
                app.apply_snapshot(
                    api_client
                        .player_mode_shuffle(!app.player.shuffle_enabled)
                        .await?,
                );
            }
            crate::tui::event::Intent::ScrollPlaylist(delta) => {
                pending = move_playlist_selection(app, delta);
            }
            crate::tui::event::Intent::ScrollPreview(delta) => {
                move_preview_selection(app, delta);
            }
            crate::tui::event::Intent::SelectPlaylist { index, .. } => {
                pending = app.select_playlist_index(index);
            }
            crate::tui::event::Intent::SelectPreview { index, .. } => {
                app.select_preview_index(index);
            }
            crate::tui::event::Intent::Action(_) => {}
        }
    }

    Ok(())
}

/// 根据布局和鼠标坐标命中交互目标。
///
/// # 参数
/// - `layout`：当前页面布局
/// - `app`：当前 TUI 状态
/// - `column`：鼠标列坐标
/// - `row`：鼠标行坐标
///
/// # 返回值
/// - `crate::tui::mouse::MouseTarget`：命中的交互目标
fn hit_test_mouse_target(
    layout: crate::tui::ui::layout::AppLayout,
    app: &crate::tui::app::App,
    column: u16,
    row: u16,
) -> crate::tui::mouse::MouseTarget {
    if rect_contains(layout.sidebar, column, row)
        && let Some(index) = crate::tui::ui::playlist::playlist_index_at(
            layout.sidebar,
            row,
            app.playlist_browser.visible_playlists.len(),
        )
    {
        return crate::tui::mouse::MouseTarget::PlaylistRow(index);
    }

    if rect_contains(layout.content_body, column, row)
        && let Some(index) = crate::tui::ui::playlist::preview_index_at(
            layout.content_body,
            row,
            app.preview_titles.len(),
        )
    {
        return crate::tui::mouse::MouseTarget::PreviewRow(index);
    }

    crate::tui::mouse::MouseTarget::None
}

/// 根据鼠标点击先更新本地选择。
///
/// # 参数
/// - `app`：当前 TUI 状态
/// - `target`：点击命中的目标
/// - `api_client`：daemon API 客户端
///
/// # 返回值
/// - `MeloResult<()>`：处理结果
async fn apply_mouse_selection(
    app: &mut crate::tui::app::App,
    target: crate::tui::mouse::MouseTarget,
    api_client: &crate::cli::client::ApiClient,
) -> crate::core::error::MeloResult<()> {
    match target {
        crate::tui::mouse::MouseTarget::PlaylistRow(index) => {
            if let Some(intent) = app.select_playlist_index(index) {
                dispatch_intent(app, intent, api_client).await?;
            }
        }
        crate::tui::mouse::MouseTarget::PreviewRow(index) => {
            app.select_preview_index(index);
        }
        crate::tui::mouse::MouseTarget::None => {}
    }

    Ok(())
}

/// 判断坐标是否位于指定矩形内。
///
/// # 参数
/// - `area`：目标矩形
/// - `column`：列坐标
/// - `row`：行坐标
///
/// # 返回值
/// - `bool`：是否命中
fn rect_contains(area: ratatui::layout::Rect, column: u16, row: u16) -> bool {
    column >= area.x
        && column < area.x.saturating_add(area.width)
        && row >= area.y
        && row < area.y.saturating_add(area.height)
}

#[cfg(test)]
mod tests;

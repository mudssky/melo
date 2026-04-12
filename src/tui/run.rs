use std::io::{self, Stdout};
use std::time::Duration;

use crossterm::event::{Event, KeyEventKind};
use crossterm::terminal::{EnterAlternateScreen, LeaveAlternateScreen};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

use crate::core::error::{MeloError, MeloResult};
use crate::tui::event::Action;

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
    crossterm::terminal::enable_raw_mode().map_err(|err| MeloError::Message(err.to_string()))?;
    let mut stdout = io::stdout();
    crossterm::execute!(stdout, EnterAlternateScreen)
        .map_err(|err| MeloError::Message(err.to_string()))?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).map_err(|err| MeloError::Message(err.to_string()))?;

    let result = run_loop(&mut terminal, base_url, context).await;

    let _ = crossterm::terminal::disable_raw_mode();
    let _ = crossterm::execute!(terminal.backend_mut(), LeaveAlternateScreen);
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
                let playlist_lines =
                    crate::tui::ui::playlist::render_playlist_lines(&app).join("\n");
                let status_lines = crate::tui::ui::playlist::render_status_lines(&app).join("\n");
                let preview_lines = crate::tui::ui::playlist::render_preview_lines(&app).join("\n");

                if let Some(task_area) = layout.task_bar
                    && let Some(text) =
                        app.task_bar_text(&renderer, &settings, task_area.width as usize)
                {
                    frame.render_widget(Paragraph::new(text), task_area);
                }

                frame.render_widget(
                    Paragraph::new(playlist_lines)
                        .block(Block::default().borders(Borders::ALL).title("播放列表")),
                    layout.sidebar,
                );
                frame.render_widget(
                    Paragraph::new(status_lines)
                        .block(Block::default().borders(Borders::ALL).title("当前播放来源")),
                    layout.content_header,
                );
                frame.render_widget(
                    Paragraph::new(preview_lines)
                        .block(Block::default().borders(Borders::ALL).title("歌单预览")),
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
                        Paragraph::new(crate::tui::ui::popup::help_lines().join("\n"))
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
            if let Event::Key(key) = event {
                if key.kind != KeyEventKind::Press {
                    continue;
                }

                match app.handle_key(key) {
                    Some(Action::TogglePlayback) => {
                        app.apply_snapshot(api_client.post_json("/api/player/toggle").await?)
                    }
                    Some(Action::Next) => {
                        app.apply_snapshot(api_client.post_json("/api/player/next").await?)
                    }
                    Some(Action::Prev) => {
                        app.apply_snapshot(api_client.post_json("/api/player/prev").await?)
                    }
                    Some(Action::LoadSelectedPlaylistPreview) => {
                        if let Some(name) = app.selected_playlist_name().map(ToString::to_string) {
                            app.set_playlist_preview_loading();
                            match api_client.playlist_preview(&name).await {
                                Ok(preview) => app.set_playlist_preview(&preview),
                                Err(err) => app.set_playlist_preview_error(err.to_string()),
                            }
                        }
                    }
                    Some(Action::PlaySelectedPlaylistFromStart) => {
                        if let Some(name) = app.selected_playlist_name().map(ToString::to_string) {
                            let snapshot = api_client.playlist_play(&name, 0).await?;
                            app.apply_tui_snapshot(snapshot);
                        }
                    }
                    Some(Action::PlaySelectedPreviewSong) => {
                        if let Some(name) = app.selected_playlist_name().map(ToString::to_string) {
                            let snapshot = api_client
                                .playlist_play(&name, app.selected_preview_index())
                                .await?;
                            app.apply_tui_snapshot(snapshot);
                        }
                    }
                    Some(Action::CycleRepeatMode) => {
                        app.apply_snapshot(
                            api_client
                                .player_mode_repeat(next_repeat_mode(&app.player.repeat_mode))
                                .await?,
                        );
                    }
                    Some(Action::ToggleShuffle) => {
                        app.apply_snapshot(
                            api_client
                                .player_mode_shuffle(!app.player.shuffle_enabled)
                                .await?,
                        );
                    }
                    Some(Action::OpenHelp) => {}
                    Some(Action::Quit) => break,
                    _ => {}
                }
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests;

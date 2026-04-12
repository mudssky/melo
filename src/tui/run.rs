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
    /// 启动来源标签。
    pub source_label: Option<String>,
    /// 一次性启动提示。
    pub startup_notice: Option<String>,
    /// 是否显示底部快捷键提示。
    pub footer_hints_enabled: bool,
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
    let mut stream = client.connect().await?;
    let mut app = crate::tui::app::App::new_for_test();
    app.apply_tui_snapshot(
        stream
            .next_json::<crate::core::model::tui::TuiSnapshot>()
            .await?,
    );
    app.footer_hints_enabled = context.footer_hints_enabled;
    app.startup_notice = context.startup_notice;
    if let Some(source_label) = context.source_label {
        app.set_source_label(source_label);
    }

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
                let queue_lines = app.render_queue_lines().join("\n");

                if let Some(task_area) = layout.task_bar
                    && let Some(text) =
                        app.task_bar_text(&renderer, &settings, task_area.width as usize)
                {
                    frame.render_widget(Paragraph::new(text), task_area);
                }

                frame.render_widget(
                    Paragraph::new("Songs")
                        .block(Block::default().borders(Borders::ALL).title("Views")),
                    layout.sidebar,
                );
                frame.render_widget(
                    Paragraph::new(queue_lines)
                        .block(Block::default().borders(Borders::ALL).title("Queue")),
                    layout.content,
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

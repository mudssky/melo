use std::io::{self, Stdout};
use std::time::Duration;

use crossterm::event::{Event, KeyEventKind};
use crossterm::terminal::{EnterAlternateScreen, LeaveAlternateScreen};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::core::error::{MeloError, MeloResult};
use crate::tui::event::Action;

/// 启动真实的 TUI 运行循环。
///
/// # 参数
/// - `base_url`：daemon HTTP 基地址
/// - `source_label`：可选来源标签
///
/// # 返回值
/// - `MeloResult<()>`：运行结果
pub async fn start(base_url: String, source_label: Option<String>) -> MeloResult<()> {
    crossterm::terminal::enable_raw_mode().map_err(|err| MeloError::Message(err.to_string()))?;
    let mut stdout = io::stdout();
    crossterm::execute!(stdout, EnterAlternateScreen)
        .map_err(|err| MeloError::Message(err.to_string()))?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).map_err(|err| MeloError::Message(err.to_string()))?;

    let result = run_loop(&mut terminal, base_url, source_label).await;

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
/// - `source_label`：可选来源标签
///
/// # 返回值
/// - `MeloResult<()>`：循环结果
async fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    base_url: String,
    source_label: Option<String>,
) -> MeloResult<()> {
    let client = crate::cli::client::ApiClient::new(base_url);
    let mut app = crate::tui::app::App::new_for_test();
    app.apply_snapshot(client.status().await?);
    if let Some(source_label) = source_label {
        app.set_source_label(source_label);
    }

    loop {
        terminal
            .draw(|frame| {
                let layout = app.layout(frame.area());
                let song_title = app
                    .player
                    .current_song
                    .as_ref()
                    .map(|song| song.title.as_str())
                    .unwrap_or("Nothing Playing");

                frame.render_widget(
                    Paragraph::new("Songs")
                        .block(Block::default().borders(Borders::ALL).title("Views")),
                    layout.sidebar,
                );
                frame.render_widget(
                    Paragraph::new(song_title)
                        .block(Block::default().borders(Borders::ALL).title("Now Playing")),
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
                        app.apply_snapshot(client.post_json("/api/player/toggle").await?)
                    }
                    Some(Action::Next) => {
                        app.apply_snapshot(client.post_json("/api/player/next").await?)
                    }
                    Some(Action::Prev) => {
                        app.apply_snapshot(client.post_json("/api/player/prev").await?)
                    }
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

use bytes::Bytes;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers, MouseEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use futures::StreamExt;
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;
use std::sync::mpsc;
use std::thread;
use std::time::Instant;

mod error;
mod metrics;
mod tui;

use error::ApiSpeedError;
use metrics::StreamMetrics;
use tui::{render_error, render_finished, render_form, render_streaming, App, AppState};

#[derive(serde::Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<Message>,
    stream: bool,
}

#[derive(serde::Serialize)]
struct Message {
    role: String,
    content: String,
}

struct ApiResponse {
    token: String,
    done: bool,
}

fn scroll_up(app: &mut tui::App) {
    let line_count = app.output_buffer.lines().count() as u16;
    let max_scroll = line_count.saturating_sub(app.output_view_height);
    // resolve actual position first (scroll_offset may be u16::MAX when pinned)
    let current = app.scroll_offset.min(max_scroll);
    app.pinned_to_bottom = false;
    app.scroll_offset = current.saturating_sub(3);
}

fn scroll_down(app: &mut tui::App) {
    let line_count = app.output_buffer.lines().count() as u16;
    let max_scroll = line_count.saturating_sub(app.output_view_height);
    let current = app.scroll_offset.min(max_scroll);
    let next = (current + 3).min(max_scroll);
    if next >= max_scroll {
        app.pinned_to_bottom = true;
        app.scroll_offset = u16::MAX;
    } else {
        app.scroll_offset = next;
    }
}

fn run_api_request(
    client: reqwest::Client,
    url: String,
    api_key: String,
    model: String,
    prompt: String,
    tx: mpsc::Sender<Result<ApiResponse, ApiSpeedError>>,
    metrics_tx: mpsc::Sender<Result<StreamMetrics, ApiSpeedError>>,
) {
    let payload = ChatRequest {
        model: model.clone(),
        messages: vec![Message {
            role: "user".to_string(),
            content: prompt,
        }],
        stream: true,
    };

    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let start = Instant::now();
        let mut all_token_times = Vec::new();
        let mut token_count = 0;

        let response = match client
            .post(&url)
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => {
                let _ = tx.send(Err(ApiSpeedError::RequestError(e)));
                return;
            }
        };

        let stream = response.bytes_stream();
        let mut stream = Box::pin(stream);

        while let Some(chunk_result) = stream.next().await {
            let bytes: Bytes = match chunk_result {
                Ok(b) => b,
                Err(e) => {
                    let _ = tx.send(Err(ApiSpeedError::RequestError(e)));
                    break;
                }
            };

            let text = String::from_utf8_lossy(&bytes).to_string();

            for line in text.lines() {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }

                // Handle both "data: ..." and plain JSON responses
                let json_str = if line.starts_with("data:") {
                    line.strip_prefix("data:").unwrap().trim()
                } else {
                    line
                };

                // Check for [DONE] or [DONE]\n\n etc
                if json_str == "[DONE]" || json_str.starts_with("[DONE") {
                    let _ = tx.send(Ok(ApiResponse {
                        token: String::new(),
                        done: true,
                    }));
                    break;
                }

                // Try to parse JSON and extract content
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(json_str) {
                    let content = json
                        .pointer("/choices/0/delta/content")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());

                    if let Some(content) = content {
                        let elapsed = start.elapsed();
                        all_token_times.push(elapsed);
                        token_count += 1;
                        let _ = tx.send(Ok(ApiResponse {
                            token: content,
                            done: false,
                        }));
                    }
                }
            }
        }

        if token_count == 0 {
            let _ = tx.send(Err(ApiSpeedError::NoTokens));
            let _ = metrics_tx.send(Err(ApiSpeedError::NoTokens));
        } else {
            let total_time = start.elapsed();
            let metrics = StreamMetrics::calculate(token_count, total_time, all_token_times);
            let _ = metrics_tx.send(Ok(metrics));
        }
    });
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut app = App::new();

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let res: Result<(), Box<dyn std::error::Error>> = loop {
        match app.state.clone() {
            AppState::InputForm => {
                terminal.draw(|f| {
                    let size = f.size();
                    render_form(f, &app, size);
                })?;

                if let Event::Key(key) = event::read()? {
                    if key.kind != KeyEventKind::Press {
                        continue;
                    }

                    match key.code {
                        KeyCode::Tab => {
                            if event::KeyModifiers::SHIFT == key.modifiers {
                                app.prev_field();
                            } else {
                                app.next_field();
                            }
                        }
                        KeyCode::BackTab => {
                            app.prev_field();
                        }
                        KeyCode::Enter => {
                            if app.is_form_valid() {
                                app.trim_inputs();

                                let (tx, rx) = mpsc::channel();
                                let (metrics_tx, metrics_rx) = mpsc::channel();

                                app.state = AppState::Streaming;
                                app.start_time = Some(Instant::now());
                                app.token_count = 0;
                                app.output_buffer.clear();
                                app.metrics = None;
                                app.scroll_offset = 0;
                                app.pinned_to_bottom = true;

                                let client = reqwest::Client::builder()
                                    .timeout(std::time::Duration::from_secs(300))
                                    .build()
                                    .expect("Failed to create HTTP client");

                                let url = app.url().to_string();
                                let api_key = app.api_key().to_string();
                                let model = app.model().to_string();
                                let prompt = app.prompt().to_string();

                                thread::spawn(move || {
                                    run_api_request(
                                        client, url, api_key, model, prompt, tx, metrics_tx,
                                    );
                                });

                                // Process streaming with the receivers
                                loop {
                                    terminal.draw(|f| {
                                        let size = f.size();
                                        render_streaming(f, &mut app, size);
                                    })?;

                                    // Check for new tokens
                                    match rx.try_recv() {
                                        Ok(Ok(response)) => {
                                            if response.done {
                                                // Stream done, will exit after metrics
                                            } else {
                                                app.add_token(&response.token);
                                            }
                                        }
                                        Ok(Err(e)) => {
                                            app.state = AppState::Error(e.user_message());
                                        }
                                        Err(mpsc::TryRecvError::Disconnected) => {
                                            // Stream thread ended, check if we have an error state
                                            if matches!(app.state, AppState::Error(_)) {
                                                break;
                                            }
                                            // Otherwise continue to check metrics
                                        }
                                        Err(mpsc::TryRecvError::Empty) => {}
                                    }

                                    // Check for metrics
                                    match metrics_rx.try_recv() {
                                        Ok(Ok(m)) => {
                                            app.metrics = Some(m);
                                            app.elapsed_at_finish =
                                                app.start_time.map(|t| t.elapsed());
                                        }
                                        Ok(Err(e)) => {
                                            app.state = AppState::Error(e.user_message());
                                            break;
                                        }
                                        Err(mpsc::TryRecvError::Disconnected) => {
                                            // Both channels disconnected, streaming is complete
                                            break;
                                        }
                                        Err(mpsc::TryRecvError::Empty) => {}
                                    }

                                    if matches!(app.state, AppState::Error(_)) {
                                        break;
                                    }

                                    // Poll for events (replaces thread::sleep)
                                    if event::poll(std::time::Duration::from_millis(50))? {
                                        match event::read()? {
                                            Event::Key(key) if key.kind == KeyEventKind::Press => {
                                                match key.code {
                                                    KeyCode::Char('c')
                                                        if key.modifiers
                                                            .contains(KeyModifiers::CONTROL) =>
                                                    {
                                                        app.state = AppState::InputForm;
                                                        app.output_buffer.clear();
                                                        app.metrics = None;
                                                        app.elapsed_at_finish = None;
                                                        break;
                                                    }
                                                    KeyCode::Up | KeyCode::Char('k') => {
                                                        scroll_up(&mut app)
                                                    }
                                                    KeyCode::Down | KeyCode::Char('j') => {
                                                        scroll_down(&mut app)
                                                    }
                                                    KeyCode::PageUp => {
                                                        for _ in 0..5 {
                                                            scroll_up(&mut app)
                                                        }
                                                    }
                                                    KeyCode::PageDown => {
                                                        for _ in 0..5 {
                                                            scroll_down(&mut app)
                                                        }
                                                    }
                                                    _ => {}
                                                }
                                            }
                                            Event::Mouse(mouse) => match mouse.kind {
                                                MouseEventKind::ScrollUp => scroll_up(&mut app),
                                                MouseEventKind::ScrollDown => scroll_down(&mut app),
                                                _ => {}
                                            },
                                            _ => {}
                                        }
                                    }
                                }
                            }
                        }
                        KeyCode::Esc => {
                            break Ok(());
                        }
                        KeyCode::Char(c) => {
                            app.focused_value().push(c);
                        }
                        KeyCode::Backspace => {
                            app.focused_value().pop();
                        }
                        KeyCode::Left => {
                            if !app.focused_value().is_empty() {
                                app.focused_value().pop();
                            }
                        }
                        KeyCode::Delete => {
                            app.focused_value().clear();
                        }
                        KeyCode::Home => {
                            app.focused_value().clear();
                        }
                        KeyCode::End => {
                            // nothing
                        }
                        KeyCode::Up => {
                            app.prev_field();
                        }
                        KeyCode::Down => {
                            app.next_field();
                        }
                        _ => {}
                    }
                }
            }
            AppState::Finished => {
                terminal.draw(|f| {
                    let size = f.size();
                    render_finished(f, &mut app, size);
                })?;

                if let Event::Key(_) = event::read()? {
                    // Reset to input form
                    app.state = AppState::InputForm;
                }
            }
            AppState::Error(_) => {
                terminal.draw(|f| {
                    let size = f.size();
                    render_error(f, &app, size);
                })?;

                if let Event::Key(_) = event::read()? {
                    // Reset to input form
                    app.state = AppState::InputForm;
                }
            }
            AppState::Streaming => {
                terminal.draw(|f| {
                    let size = f.size();
                    render_streaming(f, &mut app, size);
                })?;

                if event::poll(std::time::Duration::from_millis(100))? {
                    match event::read()? {
                        Event::Key(key) if key.kind == KeyEventKind::Press => match key.code {
                            KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q') => {
                                app.state = AppState::InputForm;
                                app.output_buffer.clear();
                                app.metrics = None;
                                app.elapsed_at_finish = None;
                            }
                            KeyCode::Up | KeyCode::Char('k') => scroll_up(&mut app),
                            KeyCode::Down | KeyCode::Char('j') => scroll_down(&mut app),
                            KeyCode::PageUp => {
                                for _ in 0..5 {
                                    scroll_up(&mut app)
                                }
                            }
                            KeyCode::PageDown => {
                                for _ in 0..5 {
                                    scroll_down(&mut app)
                                }
                            }
                            _ => {}
                        },
                        Event::Mouse(mouse) => match mouse.kind {
                            MouseEventKind::ScrollUp => scroll_up(&mut app),
                            MouseEventKind::ScrollDown => scroll_down(&mut app),
                            _ => {}
                        },
                        _ => {}
                    }
                }
            }
        }
    };

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(e) = res {
        eprintln!("Error: {}", e);
    }

    Ok(())
}

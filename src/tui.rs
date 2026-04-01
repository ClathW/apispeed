use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph, Wrap},
    Frame,
};

use crate::metrics::StreamMetrics;

#[derive(Debug, Clone)]
pub struct FormField {
    pub label: String,
    pub value: String,
    pub masked: bool,
}

impl FormField {
    pub fn new(label: &str, masked: bool) -> Self {
        Self {
            label: label.to_string(),
            value: String::new(),
            masked,
        }
    }

    pub fn with_default(label: &str, masked: bool, default: &str) -> Self {
        Self {
            label: label.to_string(),
            value: default.to_string(),
            masked,
        }
    }
}

#[derive(Debug, Clone)]
pub enum AppState {
    InputForm,
    Streaming,
    #[allow(dead_code)]
    Finished,
    Error(String),
}

pub struct App {
    pub state: AppState,
    pub fields: Vec<FormField>,
    pub focused_field: usize,
    pub output_buffer: String,
    pub token_count: usize,
    pub start_time: Option<std::time::Instant>,
    pub elapsed_at_finish: Option<std::time::Duration>,
    pub metrics: Option<StreamMetrics>,
    pub scroll_offset: u16,
    pub pinned_to_bottom: bool,
    pub output_view_height: u16,
}

impl App {
    pub fn new() -> Self {
        let fields = vec![
            FormField::new("URL", false),
            FormField::new("API Key", true),
            FormField::new("Model", false),
            FormField::with_default(
                "Prompt",
                false,
                "Write a detailed travel guide for Tokyo, covering neighborhoods, food, transport, and tips.",
            ),
        ];
        Self {
            state: AppState::InputForm,
            fields,
            focused_field: 0,
            output_buffer: String::new(),
            token_count: 0,
            start_time: None,
            elapsed_at_finish: None,
            metrics: None,
            scroll_offset: 0,
            pinned_to_bottom: true,
            output_view_height: 20,
        }
    }

    pub fn url(&self) -> &str {
        &self.fields[0].value
    }

    pub fn api_key(&self) -> &str {
        &self.fields[1].value
    }

    pub fn model(&self) -> &str {
        &self.fields[2].value
    }

    pub fn prompt(&self) -> &str {
        &self.fields[3].value
    }

    pub fn focused_value(&mut self) -> &mut String {
        &mut self.fields[self.focused_field].value
    }

    pub fn next_field(&mut self) {
        if self.focused_field < self.fields.len() - 1 {
            self.focused_field += 1;
        }
    }

    pub fn prev_field(&mut self) {
        if self.focused_field > 0 {
            self.focused_field -= 1;
        }
    }

    pub fn trim_inputs(&mut self) {
        for field in &mut self.fields {
            field.value = field.value.trim().to_string();
        }
    }

    pub fn is_url_valid(&self) -> bool {
        self.fields[0].value.starts_with("http://") || self.fields[0].value.starts_with("https://")
    }

    pub fn is_form_valid(&self) -> bool {
        !self.fields[0].value.is_empty()
            && !self.fields[1].value.is_empty()
            && !self.fields[2].value.is_empty()
            && !self.fields[3].value.is_empty()
            && self.is_url_valid()
    }

    pub fn add_token(&mut self, token: &str) {
        self.output_buffer.push_str(token);
        self.token_count += 1;
        if self.pinned_to_bottom {
            self.scroll_offset = u16::MAX; // render will clamp to actual max
        }
    }

    pub fn elapsed_time(&self) -> std::time::Duration {
        if let Some(d) = self.elapsed_at_finish {
            return d;
        }
        self.start_time
            .map(|t| t.elapsed())
            .unwrap_or(std::time::Duration::ZERO)
    }

    pub fn tokens_per_second(&self) -> f64 {
        let elapsed = self.elapsed_time().as_secs_f64();
        if elapsed > 0.0 {
            self.token_count as f64 / elapsed
        } else {
            0.0
        }
    }

    pub fn ms_per_token(&self) -> f64 {
        if self.token_count > 0 {
            self.elapsed_time().as_secs_f64() * 1000.0 / self.token_count as f64
        } else {
            0.0
        }
    }
}

pub fn render_form(f: &mut Frame, app: &App, area: Rect) {
    if area.width < 44 || area.height < 22 {
        let warning = Paragraph::new("Terminal too small. Please resize.")
            .style(Style::new().red())
            .alignment(Alignment::Center);
        f.render_widget(warning, area);
        return;
    }

    let card_w = 58u16.min(area.width.saturating_sub(4));
    let card_h = 24u16;

    let vchunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(0),
            Constraint::Length(card_h),
            Constraint::Min(0),
        ])
        .split(area);

    let hchunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Min(0),
            Constraint::Length(card_w),
            Constraint::Min(0),
        ])
        .split(vchunks[1]);

    let card = hchunks[1];

    let outer = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::new().dark_gray())
        .title(Line::from(vec![
            Span::raw(" "),
            Span::styled("⚡ API Speed Tester", Style::new().light_blue().bold()),
            Span::raw(" "),
        ]))
        .title_alignment(Alignment::Center);

    let inner = outer.inner(card);
    f.render_widget(outer, card);

    // Layout: top_pad(1) + 4 fields × field_height(4) + gap(1) + hint(1) + bottom_pad(3) = 22
    let top_pad = 1u16;
    let field_height = 4u16; // label(1) + rounded block(3)

    for (i, field) in app.fields.iter().enumerate() {
        let fy = inner.y + top_pad + (i as u16) * field_height;
        let is_focused = i == app.focused_field;

        let label_style = if is_focused {
            Style::new().blue().add_modifier(Modifier::BOLD)
        } else {
            Style::new().dark_gray()
        };

        let label_rect = Rect::new(inner.x + 2, fy, inner.width.saturating_sub(4), 1);
        f.render_widget(
            Paragraph::new(field.label.as_str()).style(label_style),
            label_rect,
        );

        let input_rect = Rect::new(inner.x + 2, fy + 1, inner.width.saturating_sub(4), 3);

        let display_value = if field.masked && !field.value.is_empty() {
            "•".repeat(field.value.len().min(50))
        } else {
            field.value.clone()
        };

        let input_block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(if is_focused {
                Style::new().blue()
            } else {
                Style::new().dark_gray()
            });

        let input_inner = input_block.inner(input_rect);
        f.render_widget(input_block, input_rect);

        f.render_widget(
            Paragraph::new(display_value.as_str()).style(if is_focused {
                Style::new().white()
            } else {
                Style::new().dark_gray()
            }),
            input_inner,
        );

        if is_focused {
            let cursor_x = (input_inner.x + display_value.chars().count() as u16)
                .min(input_inner.x + input_inner.width.saturating_sub(1));
            f.set_cursor(cursor_x, input_inner.y);
        }
    }

    let hint_y = inner.y + top_pad + (app.fields.len() as u16) * field_height + 1;
    if hint_y < inner.y + inner.height {
        f.render_widget(
            Paragraph::new("[ Enter to Start ]")
                .style(if app.is_form_valid() {
                    Style::new().green().add_modifier(Modifier::BOLD)
                } else {
                    Style::new().dark_gray()
                })
                .alignment(Alignment::Center),
            Rect::new(inner.x, hint_y, inner.width, 1),
        );
    }

    let help_y = card.y + card_h;
    if help_y < area.y + area.height {
        f.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled("Tab", Style::new().blue()),
                Span::styled("  next  ", Style::new().dark_gray()),
                Span::styled("Shift+Tab", Style::new().blue()),
                Span::styled("  prev  ", Style::new().dark_gray()),
                Span::styled("Esc", Style::new().blue()),
                Span::styled("  quit", Style::new().dark_gray()),
            ]))
            .alignment(Alignment::Center),
            Rect::new(area.x, help_y, area.width, 1),
        );
    }
}

pub fn render_streaming(f: &mut Frame, app: &mut App, area: Rect) {
    if area.width < 30 || area.height < 10 {
        let warning = Paragraph::new("Terminal too small. Please resize.")
            .style(Style::new().red())
            .alignment(Alignment::Center);
        f.render_widget(warning, area);
        return;
    }

    let is_done = app.metrics.is_some();
    let metrics_rows: u16 = if is_done { 4 } else { 1 };

    let model_str = {
        let m = app.model();
        if m.len() > 24 {
            format!("{}…", &m[..24])
        } else {
            m.to_string()
        }
    };

    let outer = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(if is_done {
            Style::new().green()
        } else {
            Style::new().dark_gray()
        })
        .title(Line::from(vec![
            Span::raw(" "),
            Span::styled(model_str, Style::new().light_blue().bold()),
            Span::raw(" "),
        ]));

    let inner = outer.inner(area);
    f.render_widget(outer, area);

    // Inner: status(1) + output(flex) + divider(1) + metrics(n)
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(1),
            Constraint::Length(metrics_rows),
        ])
        .split(inner);

    // Status line
    let elapsed = app.elapsed_time().as_secs_f64();
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::raw("  "),
            if is_done {
                Span::styled("Done", Style::new().green().bold())
            } else {
                Span::styled("Streaming", Style::new().blue())
            },
            Span::styled("  ·  ", Style::new().dark_gray()),
            Span::styled(
                format!("{} tokens", app.token_count),
                Style::new().white().bold(),
            ),
            Span::styled("  ·  ", Style::new().dark_gray()),
            Span::styled(format!("{:.1}s", elapsed), Style::new().white()),
        ])),
        chunks[0],
    );

    // Output — scroll_offset controls position; clamped to actual max
    let line_count = app.output_buffer.lines().count() as u16;
    let max_scroll = line_count.saturating_sub(chunks[1].height);
    app.output_view_height = chunks[1].height;
    let scroll = app.scroll_offset.min(max_scroll);
    f.render_widget(
        Paragraph::new(app.output_buffer.as_str())
            .style(Style::new().white())
            .wrap(Wrap { trim: false })
            .scroll((scroll, 0)),
        chunks[1],
    );

    // Divider
    f.render_widget(
        Paragraph::new("─".repeat(inner.width as usize)).style(Style::new().dark_gray()),
        chunks[2],
    );

    // Metrics
    if is_done {
        if let Some(ref m) = app.metrics {
            f.render_widget(
                Paragraph::new(vec![
                    Line::from(vec![
                        Span::styled("  Total Time  ", Style::new().dark_gray()),
                        Span::styled(
                            format!("{:.3} s", m.total_time.as_secs_f64()),
                            Style::new().white().bold(),
                        ),
                        Span::styled("        Tokens  ", Style::new().dark_gray()),
                        Span::styled(format!("{}", m.token_count), Style::new().light_blue().bold()),
                    ]),
                    Line::from(vec![
                        Span::styled("  Speed       ", Style::new().dark_gray()),
                        Span::styled(
                            format!("{:.1} t/s", m.tokens_per_second),
                            Style::new().white().bold(),
                        ),
                        Span::styled("      Per Token  ", Style::new().dark_gray()),
                        Span::styled(
                            format!("{:.2} ms", m.time_per_token_ms),
                            Style::new().white(),
                        ),
                    ]),
                    Line::from(vec![
                        Span::styled("  TTFT        ", Style::new().dark_gray()),
                        Span::styled(
                            format!("{:.3} s", m.time_to_first_token.as_secs_f64()),
                            Style::new().white(),
                        ),
                    ]),
                    Line::from(vec![
                        Span::styled("  Percentiles  ", Style::new().dark_gray()),
                        Span::styled("p50 ", Style::new().dark_gray()),
                        Span::styled(
                            format!("{:.1} ms", percentile(&m.all_token_times, 0.5)),
                            Style::new().white(),
                        ),
                        Span::styled("   p90 ", Style::new().dark_gray()),
                        Span::styled(
                            format!("{:.1} ms", percentile(&m.all_token_times, 0.9)),
                            Style::new().white(),
                        ),
                        Span::styled("   p99 ", Style::new().dark_gray()),
                        Span::styled(
                            format!("{:.1} ms", percentile(&m.all_token_times, 0.99)),
                            Style::new().white(),
                        ),
                    ]),
                ]),
                chunks[3],
            );
        }
    } else {
        let tps = app.tokens_per_second();
        let mpt = app.ms_per_token();
        f.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled("  TTFT  ", Style::new().dark_gray()),
                Span::styled(format!("{:.3}s", elapsed), Style::new().white()),
                Span::styled("    ", Style::new().dark_gray()),
                Span::styled(format!("{:.1} t/s", tps), Style::new().light_blue().bold()),
                Span::styled("    ", Style::new().dark_gray()),
                Span::styled(format!("{:.2} ms/tok", mpt), Style::new().white()),
            ])),
            chunks[3],
        );
    }
}

pub fn render_finished(f: &mut Frame, app: &mut App, area: Rect) {
    render_streaming(f, app, area);
}

pub fn render_error(f: &mut Frame, app: &App, area: Rect) {
    if area.width < 30 || area.height < 8 {
        let warning = Paragraph::new("Terminal too small. Please resize.")
            .style(Style::new().red())
            .alignment(Alignment::Center);
        f.render_widget(warning, area);
        return;
    }

    let card_w = 54u16.min(area.width.saturating_sub(4));
    let card_h = 7u16;

    let vchunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(0),
            Constraint::Length(card_h),
            Constraint::Min(0),
        ])
        .split(area);

    let hchunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Min(0),
            Constraint::Length(card_w),
            Constraint::Min(0),
        ])
        .split(vchunks[1]);

    let card = hchunks[1];

    let error_msg = if let AppState::Error(ref msg) = app.state {
        msg.clone()
    } else {
        "Unknown error".to_string()
    };

    let outer = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::new().red())
        .title(Line::from(vec![
            Span::raw(" "),
            Span::styled("Error", Style::new().red().bold()),
            Span::raw(" "),
        ]))
        .title_alignment(Alignment::Center);

    let inner = outer.inner(card);
    f.render_widget(outer, card);

    f.render_widget(
        Paragraph::new(error_msg.as_str())
            .style(Style::new().white())
            .wrap(Wrap { trim: true }),
        Rect::new(
            inner.x + 1,
            inner.y + 1,
            inner.width.saturating_sub(2),
            inner.height.saturating_sub(2),
        ),
    );

    let help_y = card.y + card_h;
    if help_y < area.y + area.height {
        f.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled("any key", Style::new().blue()),
                Span::styled("  to go back", Style::new().dark_gray()),
            ]))
            .alignment(Alignment::Center),
            Rect::new(area.x, help_y, area.width, 1),
        );
    }
}

fn percentile(times: &[std::time::Duration], p: f64) -> f64 {
    if times.is_empty() {
        return 0.0;
    }
    let mut sorted: Vec<f64> = times.iter().map(|d| d.as_secs_f64() * 1000.0).collect();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let idx = ((sorted.len() as f64 * p) as usize).min(sorted.len() - 1);
    sorted[idx]
}

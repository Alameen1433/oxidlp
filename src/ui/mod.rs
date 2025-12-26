use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
    Frame,
};
use sysinfo::Pid;

use crate::app::App;
use crate::events::JobStatus;

pub mod input;

const CYAN: Color = Color::Rgb(80, 200, 200);
const YELLOW: Color = Color::Rgb(230, 200, 100);
const GREEN: Color = Color::Rgb(80, 200, 120);
const RED: Color = Color::Rgb(200, 80, 80);
const MUTED: Color = Color::Rgb(100, 110, 120);
const TEXT: Color = Color::Rgb(180, 190, 200);
const BG: Color = Color::Rgb(30, 35, 40);

pub fn render(f: &mut Frame, app: &App) {
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), 
            Constraint::Min(10),    
            Constraint::Length(3), 
        ])
        .split(f.area());

    render_input(f, app, main_chunks[0]);
    
    let content_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(55),  
            Constraint::Percentage(45),  
        ])
        .split(main_chunks[1]);

    render_queue(f, app, content_chunks[0]);
    
    if app.show_sysinfo {
        let right_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(8),
                Constraint::Length(8),
            ])
            .split(content_chunks[1]);
        render_details(f, app, right_chunks[0]);
        render_sysinfo(f, app, right_chunks[1]);
    } else {
        render_details(f, app, content_chunks[1]);
    }
    
    render_status_bar(f, app, main_chunks[2]);

    if app.format_popup.is_some() {
        render_format_popup(f, app);
    }

    if app.settings_popup.is_some() {
        render_settings_popup(f, app);
    }

    if app.show_help {
        render_help_popup(f);
    }

    if app.confirm_quit {
        render_confirm_quit(f);
    }
}

fn render_input(f: &mut Frame, app: &App, area: Rect) {
    let input_style = if app.input_mode {
        Style::default().fg(CYAN)
    } else {
        Style::default().fg(MUTED)
    };

    const SPINNER_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
    
    let spinner_text = if app.loading_playlists > 0 {
        let frame = SPINNER_FRAMES[app.spinner_frame % SPINNER_FRAMES.len()];
        format!(" {} parsing playlist...", frame)
    } else {
        String::new()
    };

    let placeholder = if app.input_buffer.is_empty() && app.input_mode {
        "Paste URL and press Enter to add to queue..."
    } else {
        ""
    };

    let display_text = if app.input_buffer.is_empty() {
        placeholder
    } else {
        &app.input_buffer
    };

    let text_style = if app.input_buffer.is_empty() {
        Style::default().fg(MUTED)
    } else {
        input_style
    };

    let title = if app.loading_playlists > 0 {
        format!(" Input{} ", spinner_text)
    } else {
        " Input ".to_string()
    };

    let input = Paragraph::new(display_text)
        .style(text_style)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(input_style)
                .title(title)
                .title_style(if app.loading_playlists > 0 { Style::default().fg(YELLOW) } else { input_style })
        );

    f.render_widget(input, area);

    if app.input_mode && app.format_popup.is_none() {
        let cursor_x = (area.x + 1 + app.input_buffer.len() as u16).min(area.x + area.width.saturating_sub(2));
        f.set_cursor_position((cursor_x, area.y + 1));
    }
}

fn render_queue(f: &mut Frame, app: &App, area: Rect) {
    let mut items: Vec<ListItem> = Vec::new();

    for (i, job) in app.jobs.iter().enumerate() {
        let is_selected = i == app.selected_index && !app.input_mode;

        let (badge, badge_style): (String, Style) = match &job.status {
            JobStatus::FetchingFormats => ("[FETCHING]".into(), Style::default().fg(YELLOW)),
            JobStatus::Ready { .. } => ("[READY]".into(), Style::default().fg(GREEN)),
            JobStatus::Queued => ("[QUEUED]".into(), Style::default().fg(CYAN)),
            JobStatus::Downloading { percent, .. } => {
                (format!("[{:.0}%]", percent), Style::default().fg(CYAN))
            }
            JobStatus::Completed => ("[DONE]".into(), Style::default().fg(GREEN)),
            JobStatus::Failed(_) => ("[FAILED]".into(), Style::default().fg(RED)),
            JobStatus::Cancelled => ("[CANCELLED]".into(), Style::default().fg(MUTED)),
        };

        let prefix = if is_selected { "> " } else { "  " };
        let title_style = if is_selected {
            Style::default().fg(YELLOW)
        } else {
            Style::default().fg(TEXT)
        };

        let display_name = job.display_name();
        let max_len = (area.width as usize).saturating_sub(badge.len() + 5);
        let truncated: String = if display_name.len() > max_len {
            format!("{}...", &display_name[..max_len.saturating_sub(3)])
        } else {
            display_name.into()
        };

        let line = Line::from(vec![
            Span::styled(prefix, title_style),
            Span::styled(truncated, title_style),
            Span::raw(" "),
            Span::styled(badge, badge_style),
        ]);
        items.push(ListItem::new(line));
    }

    let queue = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(CYAN))
            .title(" Download Queue ")
            .title_style(Style::default().fg(CYAN)),
    );

    f.render_widget(queue, area);
}

fn render_details(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(CYAN))
        .title(" Download Details ")
        .title_style(Style::default().fg(CYAN));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let Some(job) = app.selected_job() else {
        let empty = Paragraph::new("Select an item from the queue")
            .style(Style::default().fg(MUTED));
        f.render_widget(empty, inner);
        return;
    };

    let mut lines: Vec<Line> = Vec::new();

    let title = job.title.as_deref().unwrap_or(&job.url);
    let title_wrapped = textwrap_simple(title, inner.width as usize - 2);
    for line in title_wrapped {
        lines.push(Line::from(Span::styled(line, Style::default().fg(YELLOW))));
    }
    lines.push(Line::from(""));

    if job.title.is_some() {
        lines.push(Line::from(Span::styled("URL:", Style::default().fg(MUTED))));
        let url_display = if job.url.len() > inner.width as usize - 2 {
            format!("{}...", &job.url[..inner.width as usize - 5])
        } else {
            job.url.clone()
        };
        lines.push(Line::from(Span::styled(url_display, Style::default().fg(TEXT))));
        lines.push(Line::from(""));
    }

    match &job.status {
        JobStatus::FetchingFormats => {
            lines.push(Line::from(Span::styled("Fetching formats...", Style::default().fg(YELLOW))));
        }
        JobStatus::Ready { formats } => {
            lines.push(Line::from(Span::styled("Formats Available:", Style::default().fg(MUTED))));
            lines.push(Line::from(Span::styled("─".repeat(inner.width as usize - 2), Style::default().fg(MUTED))));
            
            let mut video_count = 0;
            let mut audio_count = 0;
            for fmt in formats.iter() {
                if video_count < 3 && fmt.is_video() {
                    let info = format!(
                        "▶ {} · {} · {}",
                        fmt.ext.to_uppercase(),
                        fmt.display_resolution(),
                        fmt.display_bitrate()
                    );
                    lines.push(Line::from(Span::styled(info, Style::default().fg(CYAN))));
                    video_count += 1;
                } else if audio_count < 2 && fmt.is_audio_only() {
                    let info = format!(
                        "▶ {} · {} · AUDIO",
                        fmt.ext.to_uppercase(),
                        fmt.display_bitrate()
                    );
                    lines.push(Line::from(Span::styled(info, Style::default().fg(GREEN))));
                    audio_count += 1;
                }
                if video_count >= 3 && audio_count >= 2 { break; }
            }
            
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled("Press Enter to select format", Style::default().fg(MUTED))));
        }
        JobStatus::Queued => {
            if let Some(fmt) = &job.selected_format {
                lines.push(Line::from(Span::styled("Selected Format:", Style::default().fg(MUTED))));
                let info = format!(
                    "▶ {} · {} · {}",
                    fmt.ext.to_uppercase(),
                    fmt.display_resolution(),
                    fmt.display_bitrate()
                );
                lines.push(Line::from(Span::styled(info, Style::default().fg(CYAN))));
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled("Press 's' to start download", Style::default().fg(MUTED))));
                lines.push(Line::from(Span::styled("Press Enter to change format", Style::default().fg(MUTED))));
            }
        }
        JobStatus::Downloading { percent, speed, eta } => {
            lines.push(Line::from(Span::styled("Downloading...", Style::default().fg(CYAN))));
            
            let bar_width = (inner.width as usize).saturating_sub(2);
            let bar = progress_bar(bar_width, *percent);
            lines.push(Line::from(Span::styled(bar, Style::default().fg(CYAN))));
            
            lines.push(Line::from(Span::styled(
                format!("{:.1}% · {} · ETA {}", percent, speed, eta),
                Style::default().fg(TEXT)
            )));
        }
        JobStatus::Completed => {
            lines.push(Line::from(Span::styled("✓ Download Complete", Style::default().fg(GREEN))));
            if let Some(path) = &job.output_path {
                let path_str = path.to_string_lossy();
                lines.push(Line::from(Span::styled(
                    format!("Saved: {}", path_str),
                    Style::default().fg(MUTED)
                )));
            }
        }
        JobStatus::Failed(err) => {
            lines.push(Line::from(Span::styled("✗ Download Failed", Style::default().fg(RED))));
            let err_wrapped = textwrap_simple(err, inner.width as usize - 2);
            for line in err_wrapped.into_iter().take(3) {
                lines.push(Line::from(Span::styled(line, Style::default().fg(RED))));
            }
        }
        JobStatus::Cancelled => {
            lines.push(Line::from(Span::styled("Download Cancelled", Style::default().fg(MUTED))));
        }
    }

    let details = Paragraph::new(lines);
    f.render_widget(details, inner);
}

fn render_status_bar(f: &mut Frame, app: &App, area: Rect) {
    let counts = app.status_counts();

    let mode = if app.input_mode { "INPUT" } else { "QUEUE" };

    let mut spans = vec![
        Span::styled(format!(" {} ", mode), Style::default().fg(BG).bg(if app.input_mode { CYAN } else { YELLOW })),
        Span::styled("  ", Style::default()),
    ];
    
    if app.loading_playlists > 0 {
        spans.push(Span::styled("⟳ parsing ", Style::default().fg(YELLOW)));
    }
    
    if counts.fetching > 0 {
        spans.push(Span::styled(format!("↻{} ", counts.fetching), Style::default().fg(YELLOW)));
    }
    if counts.ready > 0 {
        spans.push(Span::styled(format!("●{} ", counts.ready), Style::default().fg(GREEN)));
    }
    if counts.queued > 0 {
        spans.push(Span::styled(format!("◇{} ", counts.queued), Style::default().fg(CYAN)));
    }
    if counts.active > 0 {
        spans.push(Span::styled(format!("▼{} ", counts.active), Style::default().fg(CYAN).add_modifier(Modifier::BOLD)));
    }
    if counts.completed > 0 {
        spans.push(Span::styled(format!("✓{} ", counts.completed), Style::default().fg(GREEN)));
    }
    if counts.failed > 0 {
        spans.push(Span::styled(format!("✗{}", counts.failed), Style::default().fg(RED)));
    }
    
    spans.push(Span::styled("  │  ", Style::default().fg(MUTED)));
    spans.push(Span::styled("?", Style::default().fg(CYAN)));
    spans.push(Span::styled(" help  ", Style::default().fg(MUTED)));
    spans.push(Span::styled("g", Style::default().fg(CYAN)));
    spans.push(Span::styled(" settings  ", Style::default().fg(MUTED)));
    spans.push(Span::styled("q", Style::default().fg(CYAN)));
    spans.push(Span::styled(" quit", Style::default().fg(MUTED)));

    let status = Line::from(spans);

    let bar = Paragraph::new(status).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(MUTED)),
    );

    f.render_widget(bar, area);
}

fn render_format_popup(f: &mut Frame, app: &App) {
    let Some(popup) = &app.format_popup else { return };
    let Some(job) = app.jobs.get(popup.job_index) else { return };

    let area = centered_rect(60, 65, f.area());
    f.render_widget(Clear, area);

    let block = popup_block(" Select Format ");
    let inner = block.inner(area);
    f.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), 
            Constraint::Length(2), 
            Constraint::Min(5),    
            Constraint::Length(2), 
        ])
        .split(inner);

    let title = Paragraph::new(job.display_name())
        .style(Style::default().fg(YELLOW).bg(BG));
    f.render_widget(title, chunks[0]);

    let video_style = if !popup.audio_only {
        Style::default().fg(CYAN).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(MUTED)
    };
    let audio_style = if popup.audio_only {
        Style::default().fg(GREEN).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(MUTED)
    };
    let apply_style = if popup.apply_to_all {
        Style::default().fg(GREEN).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(MUTED)
    };

    let apply_text = if popup.apply_to_all {
        "[✓ Apply All] (will apply to all ready items)"
    } else {
        "[ Apply All] (press A to apply to all ready items)"
    };

    let toggles = Paragraph::new(vec![
        Line::from(vec![
            Span::styled("[Video]", video_style),
            Span::raw(" "),
            Span::styled("[Audio]", audio_style),
        ]),
        Line::from(Span::styled(apply_text, apply_style)),
    ]).style(Style::default().bg(BG));
    f.render_widget(toggles, chunks[1]);

    let filtered = popup.filtered_formats();
    let visible_height = chunks[2].height as usize;
    let scroll_offset = popup.scroll_offset.min(filtered.len().saturating_sub(visible_height));

    let mut format_items: Vec<ListItem> = Vec::new();
    for (i, fmt) in filtered.iter().enumerate().skip(scroll_offset).take(visible_height) {
        let is_sel = i == popup.selected;
        let prefix = if is_sel { "▶ " } else { "  " };
        let style = if is_sel {
            Style::default().fg(YELLOW).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(TEXT)
        };

        let info = format!(
            "{} · {} · {} · {}",
            fmt.ext.to_uppercase(),
            fmt.display_resolution(),
            fmt.display_bitrate(),
            fmt.display_size()
        );
        format_items.push(ListItem::new(Line::from(vec![
            Span::styled(prefix, style),
            Span::styled(info, style),
        ])));
    }

    let list = List::new(format_items).style(Style::default().bg(BG));
    f.render_widget(list, chunks[2]);

    let hints = Paragraph::new(Line::from(vec![
        Span::styled("enter ", Style::default().fg(MUTED)),
        Span::styled("select", Style::default().fg(TEXT)),
        Span::raw("  "),
        Span::styled("a ", Style::default().fg(MUTED)),
        Span::styled("audio", Style::default().fg(TEXT)),
        Span::raw("  "),
        Span::styled("A ", Style::default().fg(MUTED)),
        Span::styled("all", Style::default().fg(TEXT)),
        Span::raw("  "),
        Span::styled("esc ", Style::default().fg(MUTED)),
        Span::styled("cancel", Style::default().fg(TEXT)),
    ])).style(Style::default().bg(BG));
    f.render_widget(hints, chunks[3]);
}

fn render_help_popup(f: &mut Frame) {
    let area = centered_rect(60, 70, f.area());
    f.render_widget(Clear, area);

    let help_text = vec![
        Line::from(Span::styled("━━━ oxidlp Help ━━━", Style::default().fg(CYAN).add_modifier(Modifier::BOLD))),
        Line::from(""),
        Line::from(Span::styled("Navigation", Style::default().fg(CYAN))),
        Line::from(vec![Span::styled("  Tab     ", Style::default().fg(YELLOW)), Span::styled("Switch between input and queue", Style::default().fg(TEXT))]),
        Line::from(vec![Span::styled("  j / ↓   ", Style::default().fg(YELLOW)), Span::styled("Move down in queue/formats", Style::default().fg(TEXT))]),
        Line::from(vec![Span::styled("  k / ↑   ", Style::default().fg(YELLOW)), Span::styled("Move up in queue/formats", Style::default().fg(TEXT))]),
        Line::from(""),
        Line::from(Span::styled("Queue Actions", Style::default().fg(CYAN))),
        Line::from(vec![Span::styled("  Enter   ", Style::default().fg(YELLOW)), Span::styled("Open format selector (on ready item)", Style::default().fg(TEXT))]),
        Line::from(vec![Span::styled("  s       ", Style::default().fg(YELLOW)), Span::styled("Start all queued downloads", Style::default().fg(TEXT))]),
        Line::from(vec![Span::styled("  d       ", Style::default().fg(YELLOW)), Span::styled("Remove selected item from queue", Style::default().fg(TEXT))]),
        Line::from(vec![Span::styled("  c       ", Style::default().fg(YELLOW)), Span::styled("Cancel active download", Style::default().fg(TEXT))]),
        Line::from(""),
        Line::from(Span::styled("Format Selection", Style::default().fg(CYAN))),
        Line::from(vec![Span::styled("  a       ", Style::default().fg(YELLOW)), Span::styled("Toggle video/audio only formats", Style::default().fg(TEXT))]),
        Line::from(vec![Span::styled("  A       ", Style::default().fg(YELLOW)), Span::styled("Apply format to ALL ready items", Style::default().fg(TEXT))]),
        Line::from(vec![Span::styled("  Enter   ", Style::default().fg(YELLOW)), Span::styled("Confirm selection", Style::default().fg(TEXT))]),
        Line::from(vec![Span::styled("  Esc     ", Style::default().fg(YELLOW)), Span::styled("Close without selecting", Style::default().fg(TEXT))]),
        Line::from(""),
        Line::from(Span::styled("General", Style::default().fg(CYAN))),
        Line::from(vec![Span::styled("  g       ", Style::default().fg(YELLOW)), Span::styled("Open settings", Style::default().fg(TEXT))]),
        Line::from(vec![Span::styled("  S       ", Style::default().fg(YELLOW)), Span::styled("Toggle system info panel", Style::default().fg(TEXT))]),
        Line::from(vec![Span::styled("  ?       ", Style::default().fg(YELLOW)), Span::styled("Toggle this help", Style::default().fg(TEXT))]),
        Line::from(vec![Span::styled("  q       ", Style::default().fg(YELLOW)), Span::styled("Quit application", Style::default().fg(TEXT))]),
    ];

    let help = Paragraph::new(help_text)
        .block(popup_block(" Help "))
        .style(Style::default().bg(BG));

    f.render_widget(help, area);
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

fn progress_bar(width: usize, percent: f32) -> String {
    let filled = ((percent / 100.0) * width as f32) as usize;
    std::iter::repeat_n('█', filled)
        .chain(std::iter::repeat_n('░', width.saturating_sub(filled)))
        .collect()
}

fn popup_block(title: &str) -> Block<'_> {
    Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(CYAN))
        .title(title)
        .title_style(Style::default().fg(CYAN))
        .style(Style::default().bg(BG))
}

fn textwrap_simple(text: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();
    
    for word in text.split_whitespace() {
        if current.is_empty() {
            current = word.to_string();
        } else if current.len() + 1 + word.len() <= width {
            current.push(' ');
            current.push_str(word);
        } else {
            lines.push(current);
            current = word.to_string();
        }
    }
    
    if !current.is_empty() {
        lines.push(current);
    }
    
    if lines.is_empty() {
        lines.push(String::new());
    }
    
    lines
}

fn render_sysinfo(f: &mut Frame, app: &App, area: Rect) {
    let pid = Pid::from_u32(std::process::id());
    
    let (cpu, rss) = app.sysinfo.process(pid).map_or((0.0, 0), |p| {
        (p.cpu_usage(), p.memory())
    });
    
    let rss_mb = rss as f64 / (1024.0 * 1024.0);
    
    let mut lines = vec![
        Line::from(vec![
            Span::styled("CPU  ", Style::default().fg(MUTED)),
            Span::styled(format!("{:.1}%", cpu), Style::default().fg(CYAN)),
        ]),
        Line::from(vec![
            Span::styled("RSS  ", Style::default().fg(MUTED)),
            Span::styled(format!("{:.1} MB", rss_mb), Style::default().fg(GREEN)),
        ]),
    ];
    
    if let Some((percent, speed, eta)) = app.aggregate_progress() {
        lines.push(Line::from(""));
        let bar_width = (area.width as usize).saturating_sub(4);
        let bar = progress_bar(bar_width, percent);
        lines.push(Line::from(Span::styled(bar, Style::default().fg(CYAN))));
        lines.push(Line::from(vec![
            Span::styled(format!("{:.0}%", percent), Style::default().fg(TEXT)),
            Span::styled(" · ", Style::default().fg(MUTED)),
            Span::styled(speed, Style::default().fg(CYAN)),
            Span::styled(" · ETA ", Style::default().fg(MUTED)),
            Span::styled(eta, Style::default().fg(TEXT)),
        ]));
    }
    
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(MUTED))
        .title(" System ")
        .title_style(Style::default().fg(MUTED));
    
    let para = Paragraph::new(lines).block(block);
    f.render_widget(para, area);
}

fn render_confirm_quit(f: &mut Frame) {
    let area = centered_rect(40, 20, f.area());
    f.render_widget(Clear, area);
    
    let text = vec![
        Line::from(""),
        Line::from(Span::styled("Downloads in progress!", Style::default().fg(YELLOW))),
        Line::from(""),
        Line::from(Span::styled("Quit anyway?", Style::default().fg(TEXT))),
        Line::from(""),
        Line::from(vec![
            Span::styled("[Y]", Style::default().fg(GREEN)),
            Span::styled(" Yes  ", Style::default().fg(TEXT)),
            Span::styled("[N]", Style::default().fg(RED)),
            Span::styled(" No", Style::default().fg(TEXT)),
        ]),
    ];
    
    let popup = Paragraph::new(text)
        .alignment(ratatui::layout::Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(YELLOW))
                .title(" Confirm Exit ")
                .title_style(Style::default().fg(YELLOW))
                .style(Style::default().bg(BG)),
        );
    
    f.render_widget(popup, area);
}

fn render_settings_popup(f: &mut Frame, app: &App) {
    let Some(settings) = &app.settings_popup else { return };
    
    let area = centered_rect(55, 35, f.area());
    f.render_widget(Clear, area);
    
    let concurrent_style = if settings.selected_field == 0 {
        Style::default().fg(CYAN).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(TEXT)
    };
    
    let path_style = if settings.selected_field == 1 {
        if settings.editing_path {
            Style::default().fg(GREEN).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(CYAN).add_modifier(Modifier::BOLD)
        }
    } else {
        Style::default().fg(TEXT)
    };
    
    let text = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("  Concurrent Downloads: ", Style::default().fg(MUTED)),
            Span::styled("◄ ", Style::default().fg(if settings.selected_field == 0 { CYAN } else { MUTED })),
            Span::styled(format!("{}", settings.concurrent_downloads), concurrent_style),
            Span::styled(" ►", Style::default().fg(if settings.selected_field == 0 { CYAN } else { MUTED })),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Download Location: ", Style::default().fg(MUTED)),
        ]),
        Line::from(vec![
            Span::styled("  ", Style::default()),
            Span::styled(&settings.output_dir, path_style),
            Span::styled(if settings.editing_path { "│" } else { "" }, Style::default().fg(GREEN)),
        ]),
        Line::from(""),
        Line::from(""),
        Line::from(vec![
            Span::styled("  ", Style::default()),
            Span::styled("[s]", Style::default().fg(CYAN)),
            Span::styled(" Save  ", Style::default().fg(TEXT)),
            Span::styled("[Esc]", Style::default().fg(MUTED)),
            Span::styled(" Cancel", Style::default().fg(TEXT)),
        ]),
        Line::from(vec![
            Span::styled("  ", Style::default()),
            Span::styled("[←/→]", Style::default().fg(MUTED)),
            Span::styled(" Adjust  ", Style::default().fg(TEXT)),
            Span::styled("[Enter]", Style::default().fg(MUTED)),
            Span::styled(" Edit path", Style::default().fg(TEXT)),
        ]),
    ];
    
    let popup = Paragraph::new(text)
        .block(popup_block(" Settings "));
    
    f.render_widget(popup, area);
}

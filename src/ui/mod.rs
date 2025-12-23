use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Clear},
    Frame,
};

use crate::app::App;
use crate::events::JobStatus;

pub mod input;

const ACCENT: Color = Color::Rgb(139, 233, 253);    
const SUCCESS: Color = Color::Rgb(80, 250, 123);    
const WARNING: Color = Color::Rgb(255, 184, 108);   
const ERROR: Color = Color::Rgb(255, 85, 85);      
const MUTED: Color = Color::Rgb(98, 114, 164);      
const BG: Color = Color::Rgb(40, 42, 54);           

pub fn render(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3),  
            Constraint::Min(10),    
            Constraint::Length(3),  
        ])
        .split(f.area());

    render_input(f, app, chunks[0]);
    render_queue(f, app, chunks[1]);
    render_status_bar(f, app, chunks[2]);

    if app.show_help {
        render_help_popup(f);
    }
}

fn render_input(f: &mut Frame, app: &App, area: Rect) {
    let input_style = if app.input_mode {
        Style::default().fg(ACCENT)
    } else {
        Style::default().fg(MUTED)
    };

    let input = Paragraph::new(app.input_buffer.as_str())
        .style(input_style)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(input_style)
                .title(" Paste URL ")
                .title_style(Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)),
        );
    
    f.render_widget(input, area);

    if app.input_mode {
        f.set_cursor_position((
            area.x + app.input_buffer.len() as u16 + 1,
            area.y + 1,
        ));
    }
}

fn render_queue(f: &mut Frame, app: &App, area: Rect) {
    let items: Vec<ListItem> = app
        .jobs
        .iter()
        .enumerate()
        .map(|(i, job)| {
            let (status_icon, status_color) = match &job.status {
                JobStatus::Pending => ("○", MUTED),
                JobStatus::FetchingMetadata => ("◌", WARNING),
                JobStatus::Downloading { percent, .. } => {
                    if *percent > 50.0 { ("◐", ACCENT) } else { ("◔", ACCENT) }
                }
                JobStatus::Completed => ("●", SUCCESS),
                JobStatus::Failed(_) => ("✗", ERROR),
                JobStatus::Cancelled => ("⊘", MUTED),
            };

            let progress_info = match &job.status {
                JobStatus::Downloading { percent, speed, eta } => {
                    format!(" [{:.1}% @ {} ETA {}]", percent, speed, eta)
                }
                JobStatus::Failed(e) => format!(" [{}]", e),
                _ => String::new(),
            };

            let is_selected = i == app.selected_index;
            let style = if is_selected {
                Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };

            let prefix = if is_selected { "▶ " } else { "  " };
            let line = Line::from(vec![
                Span::styled(prefix, style),
                Span::styled(status_icon, Style::default().fg(status_color)),
                Span::raw(" "),
                Span::styled(job.display_name(), style),
                Span::styled(progress_info, Style::default().fg(MUTED)),
            ]);

            ListItem::new(line)
        })
        .collect();

    let queue = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(MUTED))
            .title(" Queue ")
            .title_style(Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)),
    );

    f.render_widget(queue, area);
}

fn render_status_bar(f: &mut Frame, app: &App, area: Rect) {
    let pending = app.pending_count();
    let active = app.active_count();
    let completed = app.completed_count();

    let status = Line::from(vec![
        Span::styled(" oxidlp ", Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)),
        Span::styled("│ ", Style::default().fg(MUTED)),
        Span::styled(format!("Pending: {} ", pending), Style::default().fg(WARNING)),
        Span::styled("│ ", Style::default().fg(MUTED)),
        Span::styled(format!("Active: {} ", active), Style::default().fg(ACCENT)),
        Span::styled("│ ", Style::default().fg(MUTED)),
        Span::styled(format!("Done: {} ", completed), Style::default().fg(SUCCESS)),
        Span::styled("│ ", Style::default().fg(MUTED)),
        Span::styled("?:Help ", Style::default().fg(MUTED)),
        Span::styled("q:Quit", Style::default().fg(MUTED)),
    ]);

    let bar = Paragraph::new(status).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(MUTED)),
    );

    f.render_widget(bar, area);
}

fn render_help_popup(f: &mut Frame) {
    let area = centered_rect(50, 50, f.area());
    
    f.render_widget(Clear, area);
    
    let help_text = vec![
        Line::from(vec![Span::styled("Keyboard Shortcuts", Style::default().fg(ACCENT).add_modifier(Modifier::BOLD))]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Enter    ", Style::default().fg(WARNING)),
            Span::raw("Add URL / Start downloads"),
        ]),
        Line::from(vec![
            Span::styled("↑/↓ j/k  ", Style::default().fg(WARNING)),
            Span::raw("Navigate queue"),
        ]),
        Line::from(vec![
            Span::styled("d        ", Style::default().fg(WARNING)),
            Span::raw("Remove selected job"),
        ]),
        Line::from(vec![
            Span::styled("c        ", Style::default().fg(WARNING)),
            Span::raw("Cancel selected job"),
        ]),
        Line::from(vec![
            Span::styled("Esc      ", Style::default().fg(WARNING)),
            Span::raw("Exit input mode"),
        ]),
        Line::from(vec![
            Span::styled("?        ", Style::default().fg(WARNING)),
            Span::raw("Toggle help"),
        ]),
        Line::from(vec![
            Span::styled("q        ", Style::default().fg(WARNING)),
            Span::raw("Quit"),
        ]),
    ];

    let help = Paragraph::new(help_text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(ACCENT))
                .title(" Help ")
                .title_style(Style::default().fg(ACCENT).add_modifier(Modifier::BOLD))
                .style(Style::default().bg(BG)),
        )
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

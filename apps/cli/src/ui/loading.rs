use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};
use std::time::{Duration, SystemTime};

const SPINNER_FRAMES: [&str; 8] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧"];

pub struct LoadingSpinner {
    frame: usize,
    last_update: SystemTime,
}

impl LoadingSpinner {
    pub fn new() -> Self {
        Self {
            frame: 0,
            last_update: SystemTime::now(),
        }
    }

    pub fn tick(&mut self) {
        if let Ok(elapsed) = self.last_update.elapsed() {
            if elapsed > Duration::from_millis(100) {
                self.frame = (self.frame + 1) % SPINNER_FRAMES.len();
                self.last_update = SystemTime::now();
            }
        }
    }

    pub fn current_frame(&self) -> &str {
        SPINNER_FRAMES[self.frame]
    }
}

pub fn draw_loading(f: &mut Frame, area: Rect, message: &str) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(45),
            Constraint::Length(3),
            Constraint::Percentage(45),
        ])
        .split(area);

    let loading_area = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(35),
            Constraint::Percentage(30),
            Constraint::Percentage(35),
        ])
        .split(chunks[1])[1];

    let loading_text = vec![
        Line::from(vec![
            Span::styled(
                get_spinner_frame(),
                Style::default().fg(Color::Yellow),
            ),
            Span::raw(" "),
            Span::raw(message),
        ]),
    ];

    let loading_widget = Paragraph::new(loading_text)
        .block(Block::default().borders(Borders::NONE))
        .alignment(Alignment::Center);

    f.render_widget(loading_widget, loading_area);
}

pub fn draw_error(f: &mut Frame, area: Rect, error: &str) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(40),
            Constraint::Length(5),
            Constraint::Percentage(40),
        ])
        .split(area);

    let error_area = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(20),
            Constraint::Percentage(60),
            Constraint::Percentage(20),
        ])
        .split(chunks[1])[1];

    let error_text = vec![
        Line::from(vec![
            Span::styled("Error: ", Style::default().fg(Color::Red)),
            Span::raw(error),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "Press 'r' to retry",
            Style::default().fg(Color::Gray),
        )),
    ];

    let error_widget = Paragraph::new(error_text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Red))
                .title(" Error "),
        )
        .alignment(Alignment::Center);

    f.render_widget(error_widget, error_area);
}

fn get_spinner_frame() -> &'static str {
    let elapsed = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let frame_index = ((elapsed / 100) % SPINNER_FRAMES.len() as u128) as usize;
    SPINNER_FRAMES[frame_index]
}
use ratatui::{
    layout::Constraint,
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, Borders, Cell, Row, Table},
    Frame,
};

use crate::app::App;

pub fn draw(f: &mut Frame, app: &App, area: Rect) {
    let signals: Vec<Row> = app
        .signals
        .iter()
        .map(|signal| {
            let label = signal
                .atom
                .as_ref()
                .and_then(|atom| atom.label.as_deref())
                .or_else(|| {
                    signal
                        .triple
                        .as_ref()
                        .and_then(|triple| triple.label.as_deref())
                })
                .unwrap_or("N/A")
                .to_string();

            let account_label = signal
                .account
                .as_ref()
                .and_then(|account| Some(account.label.clone()))
                .unwrap_or("N/A".to_string());

            Row::new(vec![
                Cell::from(account_label),
                Cell::from(label),
                Cell::from(signal.delta.to_string()),
            ])
        })
        .collect();

    let header_cells = ["Account", "Label", "Delta"]
        .iter()
        .map(|h| Cell::from(*h).style(Style::default().fg(Color::Yellow)));
    let header = Row::new(header_cells)
        .style(Style::default().bg(Color::Black))
        .height(1);

    let signals_table = Table::new(signals, vec![Constraint::Percentage(100)])
        .header(header)
        .block(Block::default().title("Signals").borders(Borders::ALL))
        .widths([
            ratatui::layout::Constraint::Max(15),
            ratatui::layout::Constraint::Fill(1),
            ratatui::layout::Constraint::Max(15),
        ]);

    f.render_widget(signals_table, area);
}

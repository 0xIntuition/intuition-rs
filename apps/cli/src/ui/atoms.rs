use ratatui::{
    Frame,
    layout::{Constraint, Rect},
    style::{Color, Style},
    widgets::{Block, Borders, Cell, Row, Table},
};

use crate::app::App;

pub fn draw(f: &mut Frame, app: &App, area: Rect) {
    let header_cells = ["ID", "Label"]
        .iter()
        .map(|h| Cell::from(*h).style(Style::default().fg(Color::Yellow)));
    let header = Row::new(header_cells).style(Style::default().bg(Color::Black));

    let rows = app.atoms.iter().map(|atom| {
        let cells = vec![
            Cell::from(atom.term_id.to_string()),
            Cell::from(atom.label.as_deref().unwrap_or("None")),
        ];
        Row::new(cells)
    });

    let table = Table::new(rows, vec![Constraint::Percentage(100)])
        .header(header)
        .block(Block::default().title("Atoms").borders(Borders::ALL))
        .widths([Constraint::Max(5), Constraint::Percentage(50)]);

    f.render_widget(table, area);
}

use ratatui::{
    layout::Constraint,
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, Borders, Cell, Row, Table},
    Frame,
};

use crate::app::App;

pub fn draw(f: &mut Frame, app: &App, area: Rect) {
    let predicate_objects: Vec<Row> = app
        .predicate_objects
        .iter()
        .map(|predicate_object| {
            let object_label = predicate_object
                .object
                .label
                .as_ref()
                .unwrap_or(&"N/A".to_string())
                .to_string();

            let predicate_label = predicate_object.predicate
                .label
                .as_ref()
                .unwrap_or(&"N/A".to_string())
                .to_string();

            Row::new(vec![
                Cell::from(predicate_object.claim_count.to_string()),
                Cell::from(predicate_object.triple_count.to_string()),
                Cell::from(predicate_label),
                Cell::from(object_label),
            ])
        })
        .collect();

    let header_cells = ["Claims", "Triples", "Predicate", "Object"]
        .iter()
        .map(|h| Cell::from(*h).style(Style::default().fg(Color::Yellow)));
    let header = Row::new(header_cells)
        .style(Style::default().bg(Color::Black))
        .height(1);

    let table = Table::new(predicate_objects, vec![Constraint::Percentage(100)])
        .header(header)
        .block(
            Block::default()
                .title("Predicate Objects")
                .borders(Borders::ALL),
        )
        .widths([
            ratatui::layout::Constraint::Max(7),
            ratatui::layout::Constraint::Max(7),
            ratatui::layout::Constraint::Max(10),
            ratatui::layout::Constraint::Fill(20),
        ]);

    f.render_widget(table, area);
}

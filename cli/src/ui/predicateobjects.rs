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
            // let label = predicate_object
            //     .object
            //     .as_ref()
            //     .map(|object| object.label.clone())
            //     .unwrap_or_else(|| Some(String::from("N/A")));

            // FIXME: This is a temporary fix to get the code to compile
            let label = "N/A";
            Row::new(vec![
                Cell::from(predicate_object.claim_count.to_string()),
                Cell::from(predicate_object.triple_count.to_string()),
                Cell::from(label),
            ])
        })
        .collect();

    let header_cells = ["Claims", "Triples", "Object"]
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
            ratatui::layout::Constraint::Fill(1),
        ]);

    f.render_widget(table, area);
}

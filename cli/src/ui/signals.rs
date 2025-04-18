use crate::app::App;
use alloy::primitives::utils::{format_units, parse_units};
use ratatui::{
    layout::Constraint,
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, Borders, Cell, Row, Table},
    Frame,
};

pub fn draw(f: &mut Frame, app: &App, area: Rect) {
    let signals: Vec<Row> = app
        .signals
        .iter()
        .map(|signal| {
            // use signal.term.atom.label or `signal.term.triple.subject.label signal.term.triple.predicate.label signal.term.triple.object.label`  
            // both atom and triple are optional
            let label = match &signal.term.atom {
                Some(atom) => atom.label.clone().unwrap_or_else(|| "N/A".to_string()),
                None => {
                    // Handle the triple case
                    if let Some(triple) = &signal.term.triple {
                        // Both triple.subject, triple.predicate, and triple.object exist
                        // But their label properties might be Option<String>
                        let subject_str = triple.subject.label.clone().unwrap_or_else(|| "N/A".to_string());
                        let predicate_str = triple.predicate.label.clone().unwrap_or_else(|| "N/A".to_string());
                        let object_str = triple.object.label.clone().unwrap_or_else(|| "N/A".to_string());
                        
                        format!("{} {} {}", subject_str, predicate_str, object_str)
                    } else {
                        "N/A".to_string()
                    }
                }
            };

            let account_label = signal
                .account
                .as_ref()
                .map(|account| account.label.clone())
                .unwrap_or("N/A".to_string());

            Row::new(vec![
                Cell::from(account_label),
                Cell::from(
                    format_units(
                        parse_units(&signal.delta.to_string(), "wei").unwrap(),
                        "ether",
                    )
                    .unwrap_or_else(|_| "Error".to_string()),
                ),
                Cell::from(label),
            ])
        })
        .collect();

    let header_cells = ["Account", "Delta ETH", "Label"]
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
            ratatui::layout::Constraint::Max(25),
            ratatui::layout::Constraint::Fill(1),
        ]);

    f.render_widget(signals_table, area);
}

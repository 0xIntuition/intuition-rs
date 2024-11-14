use ratatui::{
    layout::{Constraint, Rect},
    text::Span,
    widgets::{Block, Borders, Cell, Row, Table},
    Frame,
};

use crate::app::App;

pub fn draw(f: &mut Frame, app: &App, area: Rect) {
    let mut rows: Vec<Row> = vec![];
    if let Some(aggregates) = &app.aggregates {
        rows.push(Row::new(vec![
            Cell::from(Span::raw("Last block")),
            Cell::from(Span::raw(
                aggregates
                    .events
                    .first()
                    .map(|e| e.block_number.to_string())
                    .unwrap_or_default()
            )),
        ]));

        rows.push(Row::new(vec![
            Cell::from(Span::raw("Accounts")),
            Cell::from(Span::raw(
                aggregates
                    .accounts_aggregate
                    .aggregate
                    .as_ref()
                    .unwrap()
                    .count
                    .to_string(),
            )),
        ]));

        rows.push(Row::new(vec![
            Cell::from(Span::raw("Atoms")),
            Cell::from(Span::raw(
                aggregates
                    .atoms_aggregate
                    .aggregate
                    .as_ref()
                    .unwrap()
                    .count
                    .to_string(),
            )),
        ]));

        rows.push(Row::new(vec![
            Cell::from(Span::raw("Triples")),
            Cell::from(Span::raw(
                aggregates
                    .triples_aggregate
                    .aggregate
                    .as_ref()
                    .unwrap()
                    .count
                    .to_string(),
            )),
        ]));
        rows.push(Row::new(vec![
            Cell::from(Span::raw("Predicate Objects")),
            Cell::from(Span::raw(
                aggregates
                    .predicate_objects_aggregate
                    .aggregate
                    .as_ref()
                    .unwrap()
                    .count
                    .to_string(),
            )),
        ]));
        rows.push(Row::new(vec![
            Cell::from(Span::raw("Vaults")),
            Cell::from(Span::raw(
                aggregates
                    .vaults_aggregate
                    .aggregate
                    .as_ref()
                    .unwrap()
                    .count
                    .to_string(),
            )),
        ]));
        rows.push(Row::new(vec![
            Cell::from(Span::raw("Atom Values")),
            Cell::from(Span::raw(
                aggregates
                    .atom_values_aggregate
                    .aggregate
                    .as_ref()
                    .unwrap()
                    .count
                    .to_string(),
            )),
        ]));
        rows.push(Row::new(vec![
            Cell::from(Span::raw("Organizations")),
            Cell::from(Span::raw(
                aggregates
                    .organizations_aggregate
                    .aggregate
                    .as_ref()
                    .unwrap()
                    .count
                    .to_string(),
            )),
        ]));

        rows.push(Row::new(vec![
            Cell::from(Span::raw("Persons")),
            Cell::from(Span::raw(
                aggregates
                    .persons_aggregate
                    .aggregate
                    .as_ref()
                    .unwrap()
                    .count
                    .to_string(),
            )),
        ]));
        rows.push(Row::new(vec![
            Cell::from(Span::raw("Things")),
            Cell::from(Span::raw(
                aggregates
                    .things_aggregate
                    .aggregate
                    .as_ref()
                    .unwrap()
                    .count
                    .to_string(),
            )),
        ]));
        rows.push(Row::new(vec![
            Cell::from(Span::raw("Signals")),
            Cell::from(Span::raw(
                aggregates
                    .signals_aggregate
                    .aggregate
                    .as_ref()
                    .unwrap()
                    .count
                    .to_string(),
            )),
        ]));

        rows.push(Row::new(vec![
            Cell::from(Span::raw("Claims")),
            Cell::from(Span::raw(
                aggregates
                    .claims_aggregate
                    .aggregate
                    .as_ref()
                    .unwrap()
                    .count
                    .to_string(),
            )),
        ]));
        rows.push(Row::new(vec![
            Cell::from(Span::raw("Positions")),
            Cell::from(Span::raw(
                aggregates
                    .positions_aggregate
                    .aggregate
                    .as_ref()
                    .unwrap()
                    .count
                    .to_string(),
            )),
        ]));
        rows.push(Row::new(vec![
            Cell::from(Span::raw("Events")),
            Cell::from(Span::raw(
                aggregates
                    .events_aggregate
                    .aggregate
                    .as_ref()
                    .unwrap()
                    .count
                    .to_string(),
            )),
        ]));
        rows.push(Row::new(vec![
            Cell::from(Span::raw("Deposits")),
            Cell::from(Span::raw(
                aggregates
                    .deposits_aggregate
                    .aggregate
                    .as_ref()
                    .unwrap()
                    .count
                    .to_string(),
            )),
        ]));

        rows.push(Row::new(vec![
            Cell::from(Span::raw("Redemptions")),
            Cell::from(Span::raw(
                aggregates
                    .redemptions_aggregate
                    .aggregate
                    .as_ref()
                    .unwrap()
                    .count
                    .to_string(),
            )),
        ]));

        rows.push(Row::new(vec![
            Cell::from(Span::raw("Fee Transfers")),
            Cell::from(Span::raw(
                aggregates
                    .fee_transfers_aggregate
                    .aggregate
                    .as_ref()
                    .unwrap()
                    .count
                    .to_string(),
            )),
        ]));
    }

    let table = Table::new(rows, vec![Constraint::Percentage(100)])
        .block(Block::default().title("Aggregates").borders(Borders::ALL))
        .widths([Constraint::Max(18), Constraint::Fill(50)]);

    f.render_widget(table, area);
}

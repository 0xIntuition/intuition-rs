use ratatui::{
    layout::{Constraint, Rect},
    text::Span,
    widgets::{Block, Borders, Cell, Row, Table},
    Frame,
};
use chrono::{DateTime, Local, TimeZone, Utc};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

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

        if let Some(event) = aggregates.events.first() {
            if let Ok(timestamp) = event.block_timestamp.parse::<i64>() {
                let block_time = match Utc.timestamp_opt(timestamp, 0) {
                    chrono::LocalResult::Single(dt) => dt,
                    _ => Utc::now(),
                };
                let local_time = DateTime::<Local>::from(block_time);
                let formatted_time = local_time.format("%b %d %Y, %I:%M %p").to_string();
                
                // Calculate time elapsed
                let now = Utc::now();
                let duration = now.signed_duration_since(block_time);
                let elapsed = format_duration(duration);
                
                rows.push(Row::new(vec![
                    Cell::from(Span::raw("Timestamp")),
                    Cell::from(Span::raw(format!("{} - {} ago", formatted_time, elapsed))),
                ]));
            }
        }

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

// Helper function to format duration in a human-readable way
fn format_duration(duration: chrono::Duration) -> String {
    let seconds = duration.num_seconds();
    if seconds < 60 {
        return format!("{} sec", seconds);
    }
    
    let minutes = duration.num_minutes();
    if minutes < 60 {
        let remaining_seconds = seconds - (minutes * 60);
        return format!("{} min {} sec", minutes, remaining_seconds);
    }
    
    let hours = duration.num_hours();
    let remaining_minutes = minutes - (hours * 60);
    format!("{} hr {} min", hours, remaining_minutes)
}

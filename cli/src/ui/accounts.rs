use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, List, ListItem, Paragraph, Row, Table},
    Frame,
};

use crate::app::App;

pub fn draw(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
        .split(area);

    draw_accounts_list(f, app, chunks[0]);
    draw_account_details(f, app, chunks[1]);
}

fn draw_accounts_list(f: &mut Frame, app: &App, area: Rect) {
    let header_cells = ["Label", "Positions"]
        .iter()
        .map(|h| Cell::from(*h).style(Style::default().fg(Color::Yellow)));
    let header = Row::new(header_cells)
        .style(Style::default().bg(Color::Black))
        .height(1);

    let rows = app.accounts.iter().map(|account| {
        let style = if Some(account.id.clone()) == app.selected_account {
            Style::default().fg(Color::Yellow).bg(Color::DarkGray)
        } else {
            Style::default()
        };
        let cells = vec![
            Cell::from(account.label.clone()),
            Cell::from(
                account
                    .positions_aggregate
                    .aggregate
                    .as_ref()
                    .map(|agg| agg.count.to_string())
                    .unwrap_or_default(),
            ),
        ];
        Row::new(cells).style(style)
    });

    let table = Table::new(rows, vec![Constraint::Percentage(100)])
        .header(header)
        .block(Block::default().title("Accounts").borders(Borders::ALL))
        .widths([Constraint::Max(18), Constraint::Fill(50)]);

    f.render_widget(table, area);
}

fn draw_account_details(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .title("Account Details")
        .borders(Borders::ALL);
    let inner_area = block.inner(area);

    if let Some(details) = &app.account_details {
        let text = vec![
            Line::from(Span::raw(format!("ID: {}", details.id))),
            Line::from(Span::raw(format!("Label: {}", details.label))),
        ];

        let details_paragraph = Paragraph::new(text).block(block);
        f.render_widget(details_paragraph, area);

        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
            .split(inner_area);

        // Positions

        let positions: Vec<ListItem> = details
            .positions
            .iter()
            .map(|pos| {
                // Extract the label, defaulting to "N/A" if none exists
                let label = 
                    if let Some(atom) = pos.vault.atom.as_ref() {
                        atom.label.as_deref().unwrap_or("N/A")
                    // } else if let Some(triple) = pos.vault.triple.as_ref() {
                    //     triple.label.as_deref().unwrap_or("N/A")
                    } else {
                        "N/A"
                    };

                ListItem::new(Line::from(format!("{}, {}", label, pos.shares)))
            })
            .collect();

        let positions_list =
            List::new(positions).block(Block::default().title("Positions").borders(Borders::ALL));

        f.render_widget(positions_list, chunks[1]);

        // Claims

        let claims: Vec<ListItem> = details
            .claims
            .iter()
            .map(|claim| {
                // let label = claim.triple.label.as_deref().unwrap_or("N/A").to_string();
                let label = "FIXME";
                ListItem::new(Line::from(format!("{}, {}", label, claim.shares)))
            })
            .collect();

        let claims_list =
            List::new(claims).block(Block::default().title("Claims").borders(Borders::ALL));

        f.render_widget(claims_list, chunks[0]);
    } else {
        let text = vec![Line::from(Span::raw("No account details"))];
        let paragraph = Paragraph::new(text).block(block);
        f.render_widget(paragraph, area);
    }
}

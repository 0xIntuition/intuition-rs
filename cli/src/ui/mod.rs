use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Style};
use ratatui::widgets::{Block, Borders, Tabs};
use ratatui::Frame;

mod accounts;
mod aggregates;
mod atoms;
mod predicateobjects;
mod signals;

use crate::app::{App, Tab};

pub fn draw(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(2), Constraint::Min(0)].as_ref())
        .split(f.area());

    let titles = vec![
        "Aggregates",
        "Accounts",
        "Predicate Objects",
        "Atoms",
        "Signals",
    ];
    let tabs = Tabs::new(titles)
        .block(Block::default().borders(Borders::NONE))
        .select(match app.current_tab {
            Tab::Aggregates => 0,
            Tab::Accounts => 1,
            Tab::PredicateObjects => 2,
            Tab::Atoms => 3,
            Tab::Signals => 4,
        })
        .style(Style::default().fg(Color::White))
        .highlight_style(Style::default().fg(Color::Yellow));

    f.render_widget(tabs, chunks[0]);

    match app.current_tab {
        Tab::Aggregates => aggregates::draw(f, app, chunks[1]),
        Tab::Accounts => accounts::draw(f, app, chunks[1]),
        Tab::PredicateObjects => predicateobjects::draw(f, app, chunks[1]),
        Tab::Atoms => atoms::draw(f, app, chunks[1]),
        Tab::Signals => signals::draw(f, app, chunks[1]),
    }
}

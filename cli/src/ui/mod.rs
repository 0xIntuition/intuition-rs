use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Style};
use ratatui::widgets::{Block, Borders, Tabs};
use ratatui::Frame;

mod accounts;
mod atoms;
mod predicateobjects;
mod signals;

use crate::app::{App, Tab};

pub fn draw(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)].as_ref())
        .split(f.area());

    let titles = vec!["Accounts", "PredicateObjects", "Atoms", "Signals"];
    let tabs = Tabs::new(titles)
        .block(Block::default().borders(Borders::ALL).title("Intuition"))
        .select(match app.current_tab {
            Tab::Accounts => 0,
            Tab::PredicateObjects => 1,
            Tab::Atoms => 2,
            Tab::Signals => 3,
        })
        .style(Style::default().fg(Color::White))
        .highlight_style(Style::default().fg(Color::Yellow));

    f.render_widget(tabs, chunks[0]);

    match app.current_tab {
        Tab::Accounts => accounts::draw(f, app, chunks[1]),
        Tab::PredicateObjects => predicateobjects::draw(f, app, chunks[1]),
        Tab::Atoms => atoms::draw(f, app, chunks[1]),
        Tab::Signals => signals::draw(f, app, chunks[1]),
    }
}

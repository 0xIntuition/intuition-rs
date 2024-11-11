use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::{error::Error, io};

mod app;
mod queries;
mod ui;

use app::App;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app and run it
    let mut app = App::new();
    app.initialize().await;
    let res = run_app(&mut terminal, app).await;

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("{:?}", err)
    }

    Ok(())
}

async fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    mut app: App,
) -> io::Result<()> {
    loop {
        terminal.draw(|f| ui::draw(f, &app))?;

        if let Event::Key(key) = event::read()? {
            match key.code {
                KeyCode::Char('q') => return Ok(()),
                KeyCode::Char('r') => app.fetch_data().await,
                KeyCode::Tab => app.next_tab(),
                KeyCode::Right => app.next_tab(),
                KeyCode::Left => app.previous_tab(),
                KeyCode::Down => {
                    app.next_account();
                    app.fetch_account_details().await;
                }
                KeyCode::Up => {
                    app.previous_account();
                    app.fetch_account_details().await;
                }
                KeyCode::Enter => {
                    if let Some(selected) = app.selected_account() {
                        app.select_account(selected);
                        app.fetch_account_details().await;
                    }
                }
                _ => {}
            }
        }
    }
}

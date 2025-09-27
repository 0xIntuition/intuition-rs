use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::backend::Backend;
use ratatui::{Terminal, backend::CrosstermBackend};
use std::{error::Error, io};
use std::{
    io::stdout,
    panic::{set_hook, take_hook},
};
mod app;
mod queries;
mod ui;

use app::{App, LoadingState};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    init_panic_hook();

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

pub fn init_panic_hook() {
    let original_hook = take_hook();
    set_hook(Box::new(move |panic_info| {
        // intentionally ignore errors here since we're already in a panic
        let _ = restore_tui();
        original_hook(panic_info);
    }));
}

pub fn init_tui() -> io::Result<Terminal<impl Backend>> {
    enable_raw_mode()?;
    execute!(stdout(), EnterAlternateScreen)?;
    Terminal::new(CrosstermBackend::new(stdout()))
}

pub fn restore_tui() -> io::Result<()> {
    disable_raw_mode()?;
    execute!(stdout(), LeaveAlternateScreen)?;
    Ok(())
}

async fn handle_key_event<B: Backend>(
    key: KeyCode,
    app: &mut App,
    terminal: &mut Terminal<B>,
) -> io::Result<bool> {
    match key {
        KeyCode::Char('q') => return Ok(true), // Signal to quit

        KeyCode::Char('r') => {
            // Refresh current tab data
            app.set_loading_state(app.current_tab, LoadingState::Loading);
            terminal.draw(|f| ui::draw(f, app))?;
            app.fetch_current_tab_data().await;
        }

        KeyCode::Char('R') => {
            // Refresh all tabs data (Shift+R)
            app.fetch_all_data().await;
        }

        KeyCode::Tab | KeyCode::Right => {
            app.next_tab();
            // Check if we need to load data for the new tab
            if app.should_load_tab() {
                app.set_loading_state(app.current_tab, LoadingState::Loading);
                terminal.draw(|f| ui::draw(f, app))?;
                app.fetch_current_tab_data().await;
            }
        }

        KeyCode::Left => {
            app.previous_tab();
            // Check if we need to load data for the new tab
            if app.should_load_tab() {
                app.set_loading_state(app.current_tab, LoadingState::Loading);
                terminal.draw(|f| ui::draw(f, app))?;
                app.fetch_current_tab_data().await;
            }
        }

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

    Ok(false) // Don't quit
}

async fn run_app<B: Backend>(terminal: &mut Terminal<B>, mut app: App) -> io::Result<()> {
    let mut last_tick = std::time::Instant::now();
    let tick_rate = std::time::Duration::from_millis(100);  // Faster tick for spinner animation

    loop {
        terminal.draw(|f| ui::draw(f, &app))?;

        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| std::time::Duration::from_secs(0));

        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                if handle_key_event(key.code, &mut app, terminal).await? {
                    return Ok(()); // Quit requested
                }
            }
        }

        if last_tick.elapsed() >= tick_rate {
            last_tick = std::time::Instant::now();
        }
    }
}
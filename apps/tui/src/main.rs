use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{prelude::*, widgets::*};
use std::io;

struct App {
    // Mimic the tabs in your screenshot
    tabs: Vec<&'static str>,
    selected_tab: usize,
    // Mimic the data rows
    items: Vec<Vec<&'static str>>,
}

impl App {
    fn new() -> App {
        App {
            tabs: vec!["Identities", "Claims", "Lists", "Activity"],
            selected_tab: 0,
            items: vec![
                vec!["Intuition", "Thing", "34.17K", "115.4K TRUST"],
                vec!["Trust Card", "Thing", "55", "64.4K TRUST"],
                vec!["has tag", "Keywords", "42.05K", "48.1K TRUST"],
                vec!["twood.eth", "Account", "158", "37.0K TRUST"],
                vec!["Sofia", "Thing", "40", "28.8K TRUST"],
            ],
        }
    }
}

fn main() -> Result<(), io::Error> {
    // 1. Terminal Setup
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // 2. App State
    let app = App::new();
    let res = run_app(&mut terminal, app);

    // 3. Cleanup
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

fn run_app<B: Backend>(terminal: &mut Terminal<B>, app: App) -> io::Result<()> {
    loop {
        terminal.draw(|f| ui(f, &app))?;

        if let Event::Key(key) = event::read()? {
            if let KeyCode::Char('q') = key.code {
                return Ok(());
            }
        }
    }
}

fn ui(f: &mut Frame, app: &App) {
    // --- LAYOUT STRATEGY ---
    // Split screen into Sidebar (Left) and Main Content (Right)
    let main_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(5), // Thin Sidebar (Icons)
            Constraint::Min(0),    // The rest of the screen
        ])
        .split(f.area());

    // Split Main Content into Top Bar (Header/Tabs) and List (Table)
    let content_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Top Bar Height
            Constraint::Length(3), // Title Area ("Identities")
            Constraint::Min(0),    // Table Area
        ])
        .split(main_layout[1]);

    // --- 1. SIDEBAR (The Icons) ---
    // Note: You need a Nerd Font installed to see these icons correctly.
    let sidebar_items = vec![
        ListItem::new("  "), // User
        ListItem::new("  "), // Chart
        ListItem::new("  "), // Bolt
        ListItem::new("  "), // Clock
        ListItem::new("  "), // Game
        ListItem::new("  "), // Folder
    ];

    let sidebar = List::new(sidebar_items)
        .style(Style::default().fg(Color::DarkGray))
        .block(Block::default().borders(Borders::RIGHT)); // Only right border like the image

    f.render_widget(sidebar, main_layout[0]);

    // --- 2. TOP BAR (Search + Tabs + Buttons) ---
    // We split the top bar horizontally to separate Search from Tabs
    let top_bar_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(30), // Search Bar
            Constraint::Percentage(50), // Tabs
            Constraint::Percentage(20), // Action Buttons
        ])
        .split(content_layout[0]);

    // Search Input simulation
    let search = Paragraph::new("   Search Intuition")
        .style(Style::default().fg(Color::Gray))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray)),
        );
    f.render_widget(search, top_bar_layout[0]);

    // Tabs
    let tabs = Tabs::new(app.tabs.clone())
        .highlight_style(
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )
        .style(Style::default().fg(Color::DarkGray))
        .divider(" ");
    f.render_widget(tabs, top_bar_layout[1]);

    // Connect Wallet Button (simulated)
    let btn = Paragraph::new("Connect Wallet")
        .alignment(Alignment::Center)
        .style(Style::default().fg(Color::Black).bg(Color::White)) // High contrast button
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(btn, top_bar_layout[2]);

    // --- 3. PAGE TITLE ---
    let title = Paragraph::new("Identities\nDecentralized identities for anything...").style(
        Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD),
    );
    f.render_widget(title, content_layout[1]);

    // --- 4. THE DATA TABLE ---
    // Transforming raw data into Rows
    let rows: Vec<Row> = app
        .items
        .iter()
        .map(|item| {
            let cells = vec![
                Cell::from(Span::styled(
                    format!("● {}", item[0]),
                    Style::default().fg(Color::White),
                )), // Identity
                Cell::from(Span::styled(
                    item[1],
                    Style::default().bg(Color::DarkGray).fg(Color::White),
                )), // Tag (Badge style)
                Cell::from(item[2]), // Users
                Cell::from(item[3]), // Trust
                Cell::from(Span::styled(
                    "Support",
                    Style::default().bg(Color::Blue).fg(Color::White),
                )), // Button
            ];
            Row::new(cells).height(2).bottom_margin(1)
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Percentage(30),
            Constraint::Percentage(15),
            Constraint::Percentage(15),
            Constraint::Percentage(20),
            Constraint::Percentage(10),
        ],
    )
    .header(
        Row::new(vec!["Identity", "Type", "Users", "Trust", "Action"])
            .style(Style::default().fg(Color::DarkGray)),
    )
    .block(
        Block::default()
            .borders(Borders::TOP)
            .title("Search identities..."),
    );

    f.render_widget(table, content_layout[2]);
}

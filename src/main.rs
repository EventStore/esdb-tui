use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::{
    io,
    time::{Duration, Instant},
};
use tui::{
    backend::{Backend, CrosstermBackend},
    layout::{Constraint, Layout},
    style::{Color, Modifier, Style},
    text::{Span, Spans},
    widgets::{Block, Row, Table, TableState, Tabs},
    widgets::{Borders, Cell},
    Frame, Terminal,
};

struct App<'a> {
    pub titles: Vec<&'a str>,
    pub dashboard_headers: Vec<&'a str>,
    pub dashboard_table_state: TableState,
    pub index: usize,
}

impl<'a> App<'a> {
    fn new() -> App<'a> {
        App {
            titles: vec![
                "Dashboard",
                "Streams Browser",
                "Projections",
                "Query",
                "Persistent Subscriptions",
            ],
            dashboard_headers: vec![
                "Queue Name",
                "Length",
                "Rate (items/s)",
                "Time (ms/item)",
                "Items Processed",
                "Current / Last Message",
            ],
            dashboard_table_state: TableState::default(),
            index: 0,
        }
    }

    fn next_tab(&mut self) {
        self.index = (self.index + 1) % self.titles.len();
    }

    fn previous_tab(&mut self) {
        if self.index > 0 {
            self.index -= 1;
        } else {
            self.index = self.titles.len() - 1;
        }
    }
}

fn main() -> Result<(), io::Error> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let tick_rate = Duration::from_millis(250);
    let app = App::new();
    let res = run_app(&mut terminal, app, tick_rate);

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture,
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("{:?}", err);
    }

    Ok(())
}

fn run_app<B: Backend>(
    terminal: &mut Terminal<B>,
    mut app: App,
    tick_rate: Duration,
) -> io::Result<()> {
    let mut last_tick = Instant::now();

    loop {
        terminal.draw(|f| ui(f, &mut app))?;

        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));

        if crossterm::event::poll(timeout)? {
            if let Event::Key(key) = crossterm::event::read()? {
                match key.code {
                    KeyCode::Char('q') => return Ok(()),
                    KeyCode::Right => app.next_tab(),
                    KeyCode::Left => app.previous_tab(),
                    _ => {}
                }
            }
        }

        if last_tick.elapsed() >= tick_rate {
            last_tick = Instant::now();
        }
    }
}

fn ui<B: Backend>(f: &mut Frame<B>, app: &mut App) {
    let size = f.size();

    let titles = app
        .titles
        .iter()
        .map(|t| Spans::from(vec![Span::styled(*t, Style::default().fg(Color::Green))]))
        .collect();

    let tabs = Tabs::new(titles)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("EventStoreDB Administration Tool")
                .title_alignment(tui::layout::Alignment::Right),
        )
        .select(app.index)
        .style(Style::default().fg(Color::Black))
        .highlight_style(
            Style::default()
                .add_modifier(Modifier::BOLD)
                .bg(Color::White),
        );

    f.render_widget(tabs, size);

    match app.index {
        0 => ui_dashboard(f, app),
        _ => {}
    }
}

fn ui_dashboard<B: Backend>(f: &mut Frame<B>, app: &mut App) {
    let rects = Layout::default()
        .constraints([Constraint::Percentage(90)].as_ref())
        .margin(3)
        .split(f.size());

    let selected_style = Style::default().add_modifier(Modifier::REVERSED);
    let normal_style = Style::default().add_modifier(Modifier::REVERSED);
    let header_cells = app
        .dashboard_headers
        .iter()
        .map(|h| Cell::from(*h).style(Style::default().fg(Color::Red)));

    let rows: Vec<Row> = Vec::new();

    let header = Row::new(header_cells)
        .style(normal_style)
        .height(1)
        .bottom_margin(1);

    let table = Table::new(rows)
        .header(header)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Dashboard")
                .title_alignment(tui::layout::Alignment::Right),
        )
        .highlight_style(selected_style)
        .highlight_symbol(">> ")
        .widths(&[
            Constraint::Percentage(20),
            Constraint::Percentage(10),
            Constraint::Percentage(10),
            Constraint::Percentage(10),
            Constraint::Percentage(10),
            Constraint::Percentage(40),
        ]);

    f.render_stateful_widget(table, rects[0], &mut app.dashboard_table_state)
}

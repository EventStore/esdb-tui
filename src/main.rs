mod views;

#[macro_use]
use log;

use crate::views::{Context, View, B};
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use eventstore::{ClientSettings, ProjectionStatus, StreamPosition};
use futures::{channel::mpsc::UnboundedReceiver, SinkExt, StreamExt};
use futures::{channel::mpsc::UnboundedSender, TryStreamExt};
use itertools::Itertools;
use log::{debug, error, LevelFilter};
use log4rs::config::{Appender, Logger, Root};
use std::{collections::HashMap, sync::Arc};
use std::{
    io,
    time::{Duration, Instant},
};
use structopt::StructOpt;
use tokio::{runtime::Runtime, sync::RwLock};
use tui::{
    backend::{Backend, CrosstermBackend},
    layout::{Constraint, Layout},
    style::{Color, Modifier, Style},
    text::Spans,
    widgets::{Block, Row, Table, TableState, Tabs},
    widgets::{Borders, Cell},
    Frame, Terminal,
};
use tui::{layout::Direction, text::Span};

#[derive(StructOpt, Debug)]
struct Args {
    #[structopt(short = "c",  long = "connection-string", default_value = "esdb://localhost:2113", parse(try_from_str = parse_connection_string))]
    conn_setts: eventstore::ClientSettings,
}

fn parse_connection_string(
    input: &str,
) -> Result<ClientSettings, eventstore::ClientSettingsParseError> {
    ClientSettings::parse_str(input)
}

struct App<'a> {
    pub context: views::Context,
    pub titles: Vec<&'a str>,
    pub stream_browser_headers: Vec<&'a str>,
    pub projections_headers: Vec<&'a str>,
    pub index: usize,
    pub projection_last_time: Option<Duration>,
    pub projection_instant: Instant,
    pub projection_last: HashMap<String, i64>,
    pub streams_view: StreamsView,
    pub dashboard_view: views::dashboard::DashboardView,
}

#[derive(Debug, Copy, Clone)]
enum StreamsViewState {
    RecentlyCreate,
    RecentlyChanged,
}

#[derive(Debug)]
struct StreamsView {
    selected_index: usize,
    state: usize,
    stream_screen: bool,
    stream_state: TableState,
}

impl Default for StreamsView {
    fn default() -> Self {
        Self {
            selected_index: 0,
            state: 0,
            stream_screen: false,
            stream_state: TableState::default(),
        }
    }
}

impl<'a> App<'a> {
    fn new(setts: ClientSettings) -> io::Result<App<'a>> {
        let context = views::Context::new(setts)?;
        Ok(App {
            context,
            titles: vec![
                "Dashboard",
                "Streams Browser",
                "Projections",
                "Query",
                "Persistent Subscriptions",
            ],
            stream_browser_headers: vec!["Recently Created Streams", "Recently Changed Streams"],
            projections_headers: vec![
                "Name",
                "Status",
                "Checkpoint Status",
                "Mode",
                "Done",
                "Read / Write in Progress",
                "Write Queues",
                "Partitions Cached",
                "Rate (events/s)",
                "Events",
            ],
            index: 0,
            projection_last_time: None,
            projection_instant: Instant::now(),
            projection_last: Default::default(),
            streams_view: Default::default(),
            dashboard_view: Default::default(),
        })
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

    fn streams_next_table(&mut self) {
        self.streams_view.state = (self.streams_view.state + 1) % 2;
    }

    fn streams_previous_table(&mut self) {
        self.streams_view.state = (self.streams_view.state + 1) % 2;
    }

    fn streams_up(&mut self) {
        if self.streams_view.selected_index == 0 {
            return;
        }

        self.streams_view.selected_index -= 1;
    }

    fn streams_down(&mut self) {
        self.streams_view.selected_index += 1;
    }
}

fn main() -> Result<(), io::Error> {
    let args = Args::from_args();

    let file = log4rs::append::file::FileAppender::builder().build("esdb.log")?;
    let config = log4rs::config::Config::builder()
        .appender(Appender::builder().build("file", Box::new(file)))
        .logger(Logger::builder().build("esdb", LevelFilter::Debug))
        .build(Root::builder().appender("file").build(LevelFilter::Error))
        .unwrap();

    let _ = log4rs::init_config(config).unwrap();

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let res = run_app(&mut terminal, args.conn_setts);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen,)?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("{:?}", err);
    }

    Ok(())
}

fn run_app(terminal: &mut Terminal<B>, setts: ClientSettings) -> io::Result<()> {
    let tick_rate = Duration::from_millis(250);
    let refresh_rate = Duration::from_secs(2);
    let mut last_tick = Instant::now();
    let mut last_refresh = Instant::now();
    let mut ctx = Context::new(setts)?;

    ctx.init();

    loop {
        terminal.draw(|frame| ctx.draw(frame))?;

        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));

        if crossterm::event::poll(timeout)? {
            if let Event::Key(key) = crossterm::event::read()? {
                if !ctx.on_key_pressed(key) {
                    return Ok(());
                }
            }
        }

        if last_tick.elapsed() >= tick_rate {
            last_tick = Instant::now();
        }

        if last_refresh.elapsed() >= refresh_rate {
            last_refresh = Instant::now();
            ctx.refresh();
        }
    }
}

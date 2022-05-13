#[macro_use]
use log;

use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use eventstore::{ClientSettings, StreamPosition};
use futures::channel::mpsc::UnboundedSender;
use futures::{channel::mpsc::UnboundedReceiver, SinkExt, StreamExt};
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
    text::{Span, Spans},
    widgets::{Block, Row, Table, TableState, Tabs},
    widgets::{Borders, Cell},
    Frame, Terminal,
};

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
                "Length (Current | Peak)",
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
    let args = Args::from_args();
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;

    let file = log4rs::append::file::FileAppender::builder().build("esdb.log")?;
    let config = log4rs::config::Config::builder()
        .appender(Appender::builder().build("file", Box::new(file)))
        .logger(Logger::builder().build("esdb", LevelFilter::Debug))
        .build(Root::builder().appender("file").build(LevelFilter::Error))
        .unwrap();

    let _ = log4rs::init_config(config).unwrap();

    let (sender, recv) = futures::channel::mpsc::unbounded();
    let state = Arc::new(RwLock::new(State::default()));
    // let _ = runtime.spawn(ticking_loop(sender.clone()));
    let _handle = runtime.spawn(main_esdb_loop(args.conn_setts, state.clone()));

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let tick_rate = Duration::from_millis(250);
    let app = App::new();
    let res = run_app(&runtime, state, &sender, &mut terminal, app, tick_rate);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen,)?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("{:?}", err);
    }

    Ok(())
}

fn run_app<B: Backend>(
    runtime: &Runtime,
    state: Arc<RwLock<State>>,
    bus: &UnboundedSender<Msg>,
    terminal: &mut Terminal<B>,
    mut app: App,
    tick_rate: Duration,
) -> io::Result<()> {
    let mut last_tick = Instant::now();

    loop {
        terminal.draw(|f| ui(runtime, state.clone(), bus, f, &mut app))?;

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

fn ui<B: Backend>(
    runtime: &Runtime,
    state: Arc<RwLock<State>>,
    bus: &UnboundedSender<Msg>,
    f: &mut Frame<B>,
    app: &mut App,
) {
    let size = f.size();

    let titles = app
        .titles
        .iter()
        .map(|t| {
            Spans::from(vec![Span::styled(
                *t,
                Style::default().fg(Color::LightGreen),
            )])
        })
        .collect();

    let tabs = Tabs::new(titles)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .style(Style::default().bg(Color::DarkGray))
                .title("EventStoreDB Administration Tool")
                .title_alignment(tui::layout::Alignment::Right),
        )
        .select(app.index)
        .style(Style::default().fg(Color::Gray))
        .highlight_style(Style::default().add_modifier(Modifier::BOLD | Modifier::REVERSED));

    f.render_widget(tabs, size);

    match app.index {
        0 => ui_dashboard(runtime, state, f, app),
        _ => {}
    }
}

fn ui_dashboard<B: Backend>(
    runtime: &Runtime,
    state: Arc<RwLock<State>>,
    f: &mut Frame<B>,
    app: &mut App,
) {
    let rects = Layout::default()
        .constraints([Constraint::Percentage(90)].as_ref())
        .margin(3)
        .split(f.size());

    let selected_style = Style::default().add_modifier(Modifier::REVERSED);
    let normal_style = Style::default().add_modifier(Modifier::REVERSED);
    let header_cells = app
        .dashboard_headers
        .iter()
        .map(|h| Cell::from(*h).style(Style::default().fg(Color::Green)));

    let stats = ask_stats(runtime, state);
    let mut rows: Vec<Row> = Vec::new();

    for (name, queue) in stats.queues.iter().sorted_by(|(a, _), (b, _)| a.cmp(b)) {
        let mut cells = Vec::new();

        cells.push(Cell::from(queue.queue_name.as_str()));
        cells.push(Cell::from(format!(
            "{} | {}",
            queue.length_current_try_peak, queue.length_lifetime_peak
        )));
        cells.push(Cell::from(queue.avg_items_per_second.as_str()));
        cells.push(Cell::from(queue.current_idle_time.as_str()));
        cells.push(Cell::from(queue.total_items_processed.as_str()));
        cells.push(Cell::from(format!(
            "{} / {}",
            queue.in_progress_message, queue.last_processed_message
        )));

        rows.push(Row::new(cells));
    }

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
            Constraint::Percentage(15),
            Constraint::Percentage(15),
            Constraint::Percentage(10),
            Constraint::Percentage(10),
            Constraint::Percentage(10),
            Constraint::Percentage(40),
        ]);

    f.render_stateful_widget(table, rects[0], &mut app.dashboard_table_state)
}

enum Msg {
    Tick,
}

#[derive(Default, Clone, Debug)]
struct State {
    stats: Stats,
    last_created_streams: Vec<String>,
}

#[derive(Default, Clone, Debug)]
struct Queue {
    avg_items_per_second: String,
    length_current_try_peak: String,
    current_idle_time: String,
    length: String,
    group_name: String,
    length_lifetime_peak: String,
    last_processed_message: String,
    total_items_processed: String,
    idle_time_percent: String,
    queue_name: String,
    in_progress_message: String,
}

#[derive(Default, Clone, Debug)]
struct Stats {
    queues: HashMap<String, Queue>,
}

impl Stats {
    fn from(map: HashMap<String, String>) -> Self {
        if map.is_empty() {
            error!("Stats from the server are empty");
        }

        let mut queues = HashMap::<String, Queue>::new();

        for (key, value) in map {
            let sections = key.split('-').collect::<Vec<&str>>();
            if sections[1] == "queue" {
                let queue_name = sections[2].to_string();
                let queue = queues.entry(queue_name).or_default();

                match sections[3] {
                    "idleTimePercent" => queue.idle_time_percent = value,
                    "queueName" => queue.queue_name = value,
                    "avgItemsPerSecond" => queue.avg_items_per_second = value,
                    "lengthCurrentTryPeak" => queue.length_current_try_peak = value,
                    "currentIdleTime" => queue.current_idle_time = value,
                    "length" => queue.length = value,
                    "groupName" => queue.group_name = value,
                    "lengthLifetimePeak" => queue.length_lifetime_peak = value,
                    "inProgressMessage" => queue.in_progress_message = value,
                    "lastProcessedMessage" => queue.last_processed_message = value,
                    _ => {}
                }
            }
        }

        Self { queues }
    }
}

async fn ticking_loop(mut sender: futures::channel::mpsc::UnboundedSender<Msg>) {
    let mut interval = tokio::time::interval(Duration::from_secs(2));
    loop {
        interval.tick().await;
        debug!("ticking...");

        if let Err(_) = sender.send(Msg::Tick).await {
            error!("Main bus is no longer able to receive MSG anymore. WTF!?");
            break;
        }
    }
}

async fn main_esdb_loop(
    setts: ClientSettings,
    state_ref: Arc<RwLock<State>>,
) -> eventstore::Result<()> {
    let mut time = tokio::time::interval(Duration::from_secs(2));
    let client = eventstore::Client::new(setts.clone())?;
    let op_client = eventstore::operations::Client::new(setts);
    let stats_options = eventstore::operations::StatsOptions::default()
        .refresh_time(Duration::from_secs(1))
        .use_metadata(true);

    let mut stats = op_client.stats(&stats_options).await?;

    let last_created_streams_options = eventstore::ReadStreamOptions::default()
        .max_count(20)
        .position(StreamPosition::End)
        .backwards();

    loop {
        time.tick().await;

        let mut state = state_ref.write().await;
        state.stats = Stats::from(stats.next().await?.unwrap_or_default());
        let mut stream_names = client
            .read_stream("$streams", &last_created_streams_options)
            .await?;

        let mut streams = Vec::with_capacity(20);
        while let Some(event) = read_stream_next(&mut stream_names).await? {
            let stream_name = std::str::from_utf8(event.get_original_event().data.as_ref())
                .expect("UTF-8 formatted text")
                .split('@')
                .collect::<Vec<&str>>()[0];

            streams.push(stream_name.to_string());
        }
    }

    error!("Stream of MSG went empty, WTF!?");
    Ok(())
}

async fn read_stream_next(
    stream: &mut eventstore::ReadStream,
) -> eventstore::Result<Option<eventstore::ResolvedEvent>> {
    match stream.next().await {
        Err(e) => {
            if let eventstore::Error::ResourceNotFound = e {
                return Ok(None);
            }

            Err(e)
        }
        Ok(v) => Ok(v),
    }
}

fn ask_stats(runtime: &Runtime, state_ref: Arc<RwLock<State>>) -> Stats {
    runtime.block_on(async move {
        let state = state_ref.read().await;
        state.stats.clone()
    })
}

fn send_msg(runtime: &Runtime, mut sender: futures::channel::mpsc::UnboundedSender<Msg>, msg: Msg) {
    runtime.spawn(async move {
        let _ = sender.send(msg).await;
    });
}

mod views;

#[macro_use]
extern crate log;

use crate::views::{Context, Request, View, B};
use crossterm::{
    event::Event,
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use eventstore::ClientSettings;
use log::LevelFilter;
use log4rs::config::{Appender, Logger, Root};
use std::{
    io,
    time::{Duration, Instant},
};
use structopt::StructOpt;
use tui::{backend::CrosstermBackend, Terminal};

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
                match ctx.on_key_pressed(key) {
                    Request::Exit => return Ok(()),
                    Request::Refresh => {
                        last_refresh = Instant::now();
                        ctx.refresh();
                    }
                    Request::Noop => {}
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

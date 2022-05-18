use crate::views::{Context, Env, View, ViewCtx, B};
use crossterm::event::KeyCode;
use eventstore::operations::Stats;
use futures::TryStreamExt;
use itertools::Itertools;
use log::error;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tui::backend::Backend;
use tui::layout::{Constraint, Layout};
use tui::style::{Color, Style};
use tui::widgets::{Block, Borders, Cell, Row, Table, TableState};
use tui::Frame;

static HEADERS: &[&'static str] = &[
    "Queue Name",
    "Length (Current | Peak)",
    "Rate (items/s)",
    "Time (ms/item)",
    "Items Processed",
    "Current / Last Message",
];

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

#[derive(Default)]
struct Model {
    queues: HashMap<String, Queue>,
}

impl Model {
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
                    "totalItemsProcessed" => queue.total_items_processed = value,
                    _ => {}
                }
            }
        }

        Self { queues }
    }
}

pub struct DashboardView {
    table_state: TableState,
    model: Model,
    stats: Arc<RwLock<Option<Stats>>>,
}

impl Default for DashboardView {
    fn default() -> Self {
        Self {
            table_state: TableState::default(),
            model: Model::default(),
            stats: Arc::new(RwLock::new(None)),
        }
    }
}

impl View for DashboardView {
    fn load(&mut self, env: &Env) {
        self.refresh(env);
    }

    fn unload(&mut self, env: &Env) {}

    fn refresh(&mut self, env: &Env) {
        let client = env.op_client.clone();
        let mut state = self.stats.clone();

        self.model = env
            .handle
            .block_on(async move {
                let mut state = state.write().await;
                if state.is_none() {
                    let options = eventstore::operations::StatsOptions::default()
                        .use_metadata(true)
                        .refresh_time(Duration::from_secs(2));

                    *state = Some(client.stats(&options).await?);
                }

                let mut model = Model::default();
                if let Some(stats) = state.as_mut() {
                    model = Model::from(stats.next().await?.unwrap_or_default());
                }

                Ok::<_, eventstore::Error>(model)
            })
            .unwrap();
    }

    fn draw(&mut self, ctx: ViewCtx, frame: &mut Frame<B>) {
        let rects = Layout::default()
            .constraints([Constraint::Percentage(100)].as_ref())
            .margin(3)
            .split(frame.size());

        let header_cells = HEADERS
            .iter()
            .map(|h| Cell::from(*h).style(Style::default().fg(Color::Green)));

        let mut rows = Vec::new();
        for (name, queue) in self
            .model
            .queues
            .iter()
            .sorted_by(|(a, _), (b, _)| a.cmp(b))
        {
            let mut cells = Vec::new();

            cells.push(Cell::from(name.as_str()));
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
            .style(ctx.normal_style)
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
            .highlight_style(ctx.selected_style)
            .widths(&[
                Constraint::Percentage(15),
                Constraint::Percentage(15),
                Constraint::Percentage(10),
                Constraint::Percentage(10),
                Constraint::Percentage(10),
                Constraint::Percentage(40),
            ]);

        frame.render_stateful_widget(table, rects[0], &mut self.table_state)
    }

    fn on_key_pressed(&mut self, key: KeyCode) -> bool {
        true
    }
}

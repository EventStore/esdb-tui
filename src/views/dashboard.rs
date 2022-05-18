use crate::views::Context;
use eventstore::operations::Stats;
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

pub struct View {
    table_state: TableState,
    stats: Option<Arc<RwLock<Stats>>>,
    instant: Instant,
}

impl Default for View {
    fn default() -> Self {
        Self {
            table_state: TableState::default(),
            stats: None,
            instant: Instant::now(),
        }
    }
}

impl View {
    pub fn draw<B: Backend>(&mut self, ctx: &Context, frame: &mut Frame<B>) {
        let rects = Layout::default()
            .constraints([Constraint::Percentage(100)].as_ref())
            .margin(3)
            .split(frame.size());

        let header_cells = HEADERS
            .iter()
            .map(|h| Cell::from(*h).style(Style::default().fg(Color::Green)));

        if self.stats.is_none() {
            let client = ctx.op_client.clone();
            let stats = ctx
                .runtime
                .block_on(async move {
                    let options = eventstore::operations::StatsOptions::default()
                        .use_metadata(true)
                        .refresh_time(Duration::from_secs(2));

                    client.stats(&options).await
                })
                .unwrap();

            self.stats = Some(Arc::new(RwLock::new(stats)));
        }

        if self.instant.elapsed() < Duration::from_secs(1) {
            return;
        }

        self.instant = Instant::now();
        let stats = self.stats.as_ref().unwrap().clone();
        let model = ctx
            .runtime
            .block_on(async move {
                let mut stats = stats.write().await;
                let model = Model::from(stats.next().await?.unwrap_or_default());

                Ok::<_, eventstore::Error>(model)
            })
            .unwrap();

        let mut rows = Vec::new();
        for (name, queue) in model.queues.iter().sorted_by(|(a, _), (b, _)| a.cmp(b)) {
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
}

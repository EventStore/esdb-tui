use crate::views::Context;
use crate::View;
use eventstore::ProjectionStatus;
use futures::TryStreamExt;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tui::backend::Backend;
use tui::layout::{Constraint, Layout};
use tui::style::{Color, Modifier, Style};
use tui::widgets::{Block, Borders, Cell, Row, Table, TableState};
use tui::Frame;

static HEADERS: &[&'static str] = &[
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
];

#[derive(Default)]
pub struct ProjectionsViews {
    table_state: TableState,
    model: Model,
}

struct Model {
    projections: Vec<ProjectionStatus>,
    last_metrics: HashMap<String, i64>,
    last_time: Option<Duration>,
    instant: Instant,
}

impl Default for Model {
    fn default() -> Self {
        Self {
            projections: vec![],
            last_metrics: Default::default(),
            last_time: None,
            instant: Instant::now(),
        }
    }
}

impl View for ProjectionsViews {
    fn load(&mut self, ctx: &Context) {
        let client = ctx.proj_client.clone();
        self.model.projections = ctx
            .runtime
            .block_on(async move {
                client
                    .list(&Default::default())
                    .await?
                    .try_collect::<Vec<_>>()
                    .await
            })
            .unwrap();
    }

    fn unload(&mut self, ctx: &Context) {}

    fn refresh(&mut self, ctx: &Context) {}

    fn draw<B: Backend>(&mut self, ctx: &Context, frame: &mut Frame<B>) {
        let rects = Layout::default()
            .constraints([Constraint::Percentage(100)].as_ref())
            .margin(3)
            .split(frame.size());

        let header_cells = HEADERS
            .iter()
            .map(|h| Cell::from(*h).style(Style::default().fg(Color::Green)));

        let mut rows: Vec<Row> = Vec::new();

        for proj in self.model.projections.iter() {
            let mut cells = Vec::new();

            cells.push(Cell::from(proj.name.as_str()));
            cells.push(Cell::from(proj.status.as_str()));
            if proj.checkpoint_status.is_empty() {
                cells.push(Cell::from("-"));
            } else {
                cells.push(Cell::from(proj.checkpoint_status.as_str()));
            }
            cells.push(Cell::from(proj.mode.as_str()));
            cells.push(Cell::from(format!("{:.1}%", proj.progress)));
            cells.push(Cell::from(format!(
                "{} / {}",
                proj.reads_in_progress, proj.writes_in_progress
            )));
            cells.push(Cell::from(proj.buffered_events.to_string()));
            cells.push(Cell::from(proj.partitions_cached.to_string()));

            if let Some(last_time) = self.model.last_time {
                let last = self
                    .model
                    .last_metrics
                    .get(&proj.name)
                    .copied()
                    .unwrap_or_default();
                let events_processed = proj.events_processed_after_restart - last;
                let now = self.model.instant.elapsed();
                let rate = events_processed as f32 / (now.as_secs_f32() - last_time.as_secs_f32());
                cells.push(Cell::from(format!("{:.1}", rate)));
                cells.push(Cell::from(events_processed.to_string()));
                self.model
                    .last_metrics
                    .insert(proj.name.clone(), proj.events_processed_after_restart);
            } else {
                cells.push(Cell::from("0.0"));
                cells.push(Cell::from("0"));
                self.model
                    .last_metrics
                    .insert(proj.name.clone(), proj.events_processed_after_restart);
            }

            rows.push(Row::new(cells));
            self.model.last_time = Some(self.model.instant.elapsed());
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
                    .title("Projections")
                    .title_alignment(tui::layout::Alignment::Right),
            )
            .highlight_style(ctx.selected_style)
            .widths(&[
                Constraint::Percentage(15),
                Constraint::Percentage(5),
                Constraint::Percentage(10),
                Constraint::Percentage(10),
                Constraint::Percentage(10),
                Constraint::Percentage(10),
                Constraint::Percentage(10),
                Constraint::Percentage(10),
                Constraint::Percentage(10),
                Constraint::Percentage(10),
                Constraint::Percentage(10),
            ]);

        frame.render_stateful_widget(table, rects[0], &mut Default::default());
    }
}

use crate::views::{Env, Request, ViewCtx, B};
use crate::View;
use crossterm::event::KeyCode;
use eventstore::ProjectionStatus;
use futures::TryStreamExt;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tui::layout::{Constraint, Layout, Rect};
use tui::style::{Color, Style};
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
    model: Model,
    main_table_state: TableState,
    selected: usize,
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
    fn load(&mut self, env: &Env) {
        let client = env.proj_client.clone();
        self.model.projections = env
            .handle
            .block_on(async move {
                client
                    .list(&Default::default())
                    .await?
                    .try_collect::<Vec<_>>()
                    .await
            })
            .unwrap();
    }

    fn unload(&mut self, _env: &Env) {}

    fn refresh(&mut self, _env: &Env) {}

    fn draw(&mut self, ctx: ViewCtx, frame: &mut Frame<B>, area: Rect) {
        let rects = Layout::default()
            .constraints([Constraint::Min(0)].as_ref())
            .margin(2)
            .split(area);

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
                    .borders(Borders::TOP)
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

        self.main_table_state.select(Some(self.selected));

        frame.render_stateful_widget(table, rects[0], &mut self.main_table_state);
    }

    fn on_key_pressed(&mut self, key: KeyCode) -> Request {
        if let KeyCode::Char('q' | 'Q') = key {
            return Request::Exit;
        }

        match key {
            KeyCode::Char('q' | 'Q') => return Request::Exit,
            KeyCode::Up => {
                if !self.model.projections.is_empty() && self.selected > 0 {
                    self.selected -= 1;
                }
            }

            KeyCode::Down => {
                if !self.model.projections.is_empty()
                    && self.selected < self.model.projections.len() - 1
                {
                    self.selected += 1;
                }
            }

            _ => {}
        }

        Request::Noop
    }

    fn keybindings(&self) -> &[(&str, &str)] {
        &[
            ("↑", "Scroll up"),
            ("↓", "Scroll down"),
            ("Enter", "Select"),
        ]
    }
}

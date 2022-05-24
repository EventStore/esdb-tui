use crate::views::{render_line_numbers, Env, Request, ViewCtx, B};
use crate::View;
use crossterm::event::KeyCode;
use eventstore::{ProjectionStatus, ReadStreamOptions, StreamPosition};
use futures::TryStreamExt;
use serde::Deserialize;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use tui::style::{Color, Style};
use tui::widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState};
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

#[derive(Copy, Clone, Eq, PartialEq)]
enum Stage {
    Main,
    Detail,
}

impl Default for Stage {
    fn default() -> Self {
        Stage::Main
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProjectionDetails {
    query: String,
}

#[derive(Default)]
pub struct ProjectionsViews {
    model: Model,
    main_table_state: TableState,
    selected: usize,
    stage: Stage,
    scroll: u16,
}

struct Model {
    projections: Vec<ProjectionStatus>,
    last_metrics: HashMap<String, i64>,
    last_time: Option<Duration>,
    instant: Instant,
    selected_projection: Option<ProjectionDetails>,
}

impl Default for Model {
    fn default() -> Self {
        Self {
            projections: vec![],
            last_metrics: Default::default(),
            last_time: None,
            instant: Instant::now(),
            selected_projection: Default::default(),
        }
    }
}

impl ProjectionsViews {
    fn draw_main(&mut self, ctx: ViewCtx, frame: &mut Frame<B>, area: Rect) {
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

    fn draw_details(&mut self, ctx: ViewCtx, frame: &mut Frame<B>, area: Rect) {
        let rects = Layout::default()
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
            .margin(2)
            .direction(Direction::Horizontal)
            .split(area);

        let proj = &self.model.selected_projection.as_ref().unwrap();
        let proj_name = &self.model.projections[self.selected].name.as_str();

        let content = render_line_numbers(proj.query.as_str());

        let query = Paragraph::new(content)
            .alignment(Alignment::Left)
            .block(Block::default().borders(Borders::ALL))
            .scroll((self.scroll, 0));

        frame.render_widget(query, rects[0]);

        let table = Table::new(vec![])
            .block(
                Block::default()
                    .borders(Borders::TOP | Borders::BOTTOM)
                    .title(*proj_name)
                    .title_alignment(Alignment::Right),
            )
            .highlight_style(ctx.selected_style)
            .widths(&[Constraint::Percentage(60), Constraint::Percentage(40)]);

        frame.render_stateful_widget(table, rects[1], &mut Default::default());
    }
}

impl View for ProjectionsViews {
    fn load(&mut self, env: &Env) -> eventstore::Result<()> {
        let client = env.proj_client.clone();
        self.model.projections = env.handle.block_on(async move {
            client
                .list(&Default::default())
                .await?
                .try_collect::<Vec<_>>()
                .await
        })?;

        Ok(())
    }

    fn unload(&mut self, _env: &Env) {}

    fn refresh(&mut self, env: &Env) -> eventstore::Result<()> {
        if self.stage == Stage::Detail && self.model.selected_projection.is_none() {
            let proj_name = self.model.projections[self.selected].name.clone();
            let client = env.client.clone();

            let details = env.handle.block_on(async move {
                let options = ReadStreamOptions::default()
                    .position(StreamPosition::End)
                    .backwards();

                let stream_name = format!("$projections-{}", proj_name);

                let mut stream = client.read_stream(stream_name.as_str(), &options).await?;

                while let Some(event) = stream.next().await? {
                    if event.get_original_event().event_type == "$ProjectionUpdated" {
                        let details = event
                            .get_original_event()
                            .as_json::<ProjectionDetails>()
                            .expect("valid projection details JSON");

                        return Ok(details);
                    }
                }

                Err(eventstore::Error::ResourceNotFound)
            })?;

            self.model.selected_projection = Some(details);
        }
        Ok(())
    }

    fn draw(&mut self, ctx: ViewCtx, frame: &mut Frame<B>, area: Rect) {
        match self.stage {
            Stage::Main => self.draw_main(ctx, frame, area),
            Stage::Detail => self.draw_details(ctx, frame, area),
        }
    }

    fn on_key_pressed(&mut self, key: KeyCode) -> Request {
        if let KeyCode::Char('q' | 'Q') = key {
            if self.stage == Stage::Detail {
                self.stage = Stage::Main;
                self.model.selected_projection = None;
                return Request::Noop;
            }

            return Request::Exit;
        }

        match key {
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

            KeyCode::Enter => {
                self.stage = Stage::Detail;
                return Request::Refresh;
            }

            _ => {}
        }

        Request::Noop
    }

    fn keybindings(&self) -> &[(&str, &str)] {
        match self.stage {
            Stage::Main => &[
                ("↑", "Scroll up"),
                ("↓", "Scroll down"),
                ("Enter", "Select"),
            ],

            Stage::Detail => &[
                ("↑", "Scroll up"),
                ("↓", "Scroll down"),
                ("Enter", "Select"),
                ("q", "Close"),
            ],
        }
    }
}

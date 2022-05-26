use crate::models::{Projection, Projections};
use crate::views::{render_line_numbers, Env, Request, ViewCtx, B};
use crate::View;
use crossterm::event::KeyCode;
use eventstore::{ReadStreamOptions, StreamPosition};
use futures::TryStreamExt;
use serde::Deserialize;
use tui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use tui::style::{Color, Modifier, Style};
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
    model: Projections,
    main_table_state: TableState,
    selected: usize,
    stage: Stage,
    scroll: u16,
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

        for proj in self.model.list() {
            rows.push(Row::new(main_proj_mapping(proj)));
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

        let proj = self.model.by_idx(self.selected).unwrap();
        let content = render_line_numbers(proj.query.as_str());

        let query = Paragraph::new(content)
            .alignment(Alignment::Left)
            .block(Block::default().borders(Borders::ALL))
            .scroll((self.scroll, 0));

        frame.render_widget(query, rects[0]);

        let table = Table::new(detail_proj_mapping(proj))
            .block(
                Block::default()
                    .borders(Borders::TOP | Borders::BOTTOM)
                    .title(proj.name.as_str())
                    .title_alignment(Alignment::Right),
            )
            .highlight_style(ctx.selected_style)
            .widths(&[Constraint::Percentage(60), Constraint::Percentage(40)]);

        frame.render_stateful_widget(table, rects[1], &mut Default::default());
    }
}

impl View for ProjectionsViews {
    fn load(&mut self, env: &Env) -> eventstore::Result<()> {
        self.refresh(env)
    }

    fn unload(&mut self, _env: &Env) {}

    fn refresh(&mut self, env: &Env) -> eventstore::Result<()> {
        if self.stage == Stage::Detail {
            let proj = self.model.by_idx_mut(self.selected).unwrap();
            let proj_name = proj.name.clone();
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

            proj.query = details.query;
        } else {
            let client = env.proj_client.clone();
            let projections = env.handle.block_on(async move {
                client
                    .list(&Default::default())
                    .await?
                    .try_collect::<Vec<_>>()
                    .await
            })?;

            self.model.update(projections);
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
                self.selected = 0;
                return Request::Noop;
            }

            return Request::Exit;
        }

        match key {
            KeyCode::Up => {
                if self.selected > 0 {
                    self.selected -= 1;
                }
            }

            KeyCode::Down => {
                if self.selected + 1 < self.model.count() {
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

fn main_proj_mapping(proj: &Projection) -> Vec<Cell> {
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
    cells.push(Cell::from(format!("{:.1}", proj.rate)));
    cells.push(Cell::from(proj.events_processed.to_string()));

    cells
}

fn detail_proj_mapping(proj: &Projection) -> Vec<Row> {
    let mut rows = Vec::<Row>::new();

    rows.push(Row::new(vec![
        Cell::from("Events/sec"),
        Cell::from(proj.rate.to_string()),
    ]));
    rows.push(Row::new(vec![
        Cell::from("Buffered events"),
        Cell::from(proj.buffered_events.to_string()),
    ]));
    rows.push(Row::new(vec![
        Cell::from("Events processed"),
        Cell::from(proj.events_processed.to_string()),
    ]));
    rows.push(Row::new(vec![
        Cell::from("Partitions cached"),
        Cell::from(proj.partitions_cached.to_string()),
    ]));
    rows.push(Row::new(vec![
        Cell::from("Reads in-progress"),
        Cell::from(proj.reads_in_progress.to_string()),
    ]));
    rows.push(Row::new(vec![
        Cell::from("Writes in-progress"),
        Cell::from(proj.writes_in_progress.to_string()),
    ]));
    rows.push(Row::new(vec![
        Cell::from("Write queue"),
        Cell::from(proj.write_queue.to_string()),
    ]));
    rows.push(Row::new(vec![
        Cell::from("Write queue (chkp)"),
        Cell::from(proj.write_queue_checkpoint.to_string()),
    ]));
    rows.push(Row::new(vec![
        Cell::from("Checkpoint status"),
        Cell::from(proj.checkpoint_status.as_str()),
    ]));
    rows.push(Row::new(vec![
        Cell::from("Position"),
        Cell::from(proj.position.as_str()),
    ]));
    rows.push(Row::new(vec![
        Cell::from("Last checkpoint"),
        Cell::from(proj.last_checkpoint.as_str()),
    ]));
    rows.push(Row::new(vec![
        Cell::from("Results").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from(proj.result.as_str()),
    ]));
    rows.push(Row::new(vec![
        Cell::from("State").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from(proj.state.as_str()),
    ]));

    rows
}

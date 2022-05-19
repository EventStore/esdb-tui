use crate::views::{Context, Env, Request, View, ViewCtx, B};
use chrono::Utc;
use crossterm::cursor::position;
use crossterm::event::KeyCode;
use eventstore::{ResolvedEvent, StreamPosition};
use tui::backend::Backend;
use tui::layout::{Constraint, Direction, Layout};
use tui::style::Color::Gray;
use tui::style::{Color, Style};
use tui::widgets::{Block, Borders, Cell, Row, Table, TableState};
use tui::Frame;

static HEADERS: &[&'static str] = &["Recently Created Streams", "Recently Changed Streams"];
static STREAM_HEADERS: &[&'static str] = &["Event #", "Name", "Type", "Created Date"];

pub struct StreamsView {
    selected_tab: usize,
    selected: usize,
    stream_view_showing: bool,
    main_table_states: Vec<TableState>,
    stream_table_state: TableState,
    model: Model,
}

impl Default for StreamsView {
    fn default() -> Self {
        Self {
            selected_tab: 0,
            selected: 0,
            stream_view_showing: false,
            main_table_states: vec![TableState::default(), TableState::default()],
            stream_table_state: Default::default(),
            model: Default::default(),
        }
    }
}

#[derive(Default)]
struct Model {
    last_created: Vec<String>,
    recently_changed: Vec<String>,
    selected_stream: Option<String>,
    selected_stream_events: Vec<ResolvedEvent>,
}

impl View for StreamsView {
    fn load(&mut self, env: &Env) {
        self.selected = 0;
        self.selected_tab = 0;
        let client = env.client.clone();
        self.model = env
            .handle
            .block_on(async move {
                let mut model = Model::default();
                let options_1 = eventstore::ReadStreamOptions::default()
                    .max_count(20)
                    .position(StreamPosition::End)
                    .backwards();

                let options_2 = eventstore::ReadAllOptions::default()
                    .max_count(20)
                    .position(StreamPosition::End)
                    .backwards();

                let mut stream_names = client.read_stream("$streams", &options_1).await?;
                let mut all_stream = client.read_all(&options_2).await?;

                while let Some(event) = read_stream_next(&mut stream_names).await? {
                    let (_, stream_name) =
                        std::str::from_utf8(event.get_original_event().data.as_ref())
                            .expect("UTF-8 formatted text")
                            .rsplit_once('@')
                            .unwrap_or_default();

                    model.last_created.push(stream_name.to_string());
                }

                while let Some(event) = read_stream_next(&mut all_stream).await? {
                    let stream_id = &event.get_original_event().stream_id;
                    if model.recently_changed.contains(stream_id) {
                        continue;
                    }

                    model.recently_changed.push(stream_id.clone());
                }

                Ok::<_, eventstore::Error>(model)
            })
            .unwrap();
    }

    fn unload(&mut self, env: &Env) {}

    fn refresh(&mut self, env: &Env) {
        if let Some(stream_name) = self.model.selected_stream.clone() {
            let client = env.client.clone();
            self.model.selected_stream_events = env
                .handle
                .block_on(async move {
                    let options = eventstore::ReadStreamOptions::default()
                        .max_count(20)
                        .resolve_link_tos()
                        .position(StreamPosition::End)
                        .backwards();

                    let mut stream = client.read_stream(stream_name, &options).await?;
                    let mut events = Vec::new();

                    while let Some(event) = stream.next().await? {
                        events.push(event);
                    }

                    Ok::<_, eventstore::Error>(events)
                })
                .unwrap();
        }
    }

    fn draw(&mut self, ctx: ViewCtx, frame: &mut Frame<B>) {
        if self.stream_view_showing {
            let rects = Layout::default()
                .constraints([Constraint::Percentage(100)].as_ref())
                .margin(3)
                .split(frame.size());

            let stream_name = self.model.selected_stream.clone().unwrap_or_default();

            let header_cells = STREAM_HEADERS
                .iter()
                .map(|h| Cell::from(*h).style(Style::default().fg(Color::Green)));

            let header = Row::new(header_cells)
                .style(ctx.normal_style)
                .height(1)
                .bottom_margin(1);

            let mut rows = Vec::new();

            for event in self.model.selected_stream_events.iter() {
                let event = event.get_original_event();
                let mut cols = Vec::new();

                cols.push(Cell::from(event.revision.to_string()).style(Style::default().fg(Gray)));

                let name = format!("{}@{}", event.revision, event.stream_id);
                cols.push(Cell::from(name).style(Style::default().fg(Gray)));
                cols.push(Cell::from(event.event_type.clone()).style(Style::default().fg(Gray)));
                cols.push(Cell::from(Utc::now().to_string()).style(Style::default().fg(Gray)));

                rows.push(Row::new(cols));
            }

            let table = Table::new(rows)
                .header(header)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(format!("Event Stream '{}'", stream_name))
                        .title_alignment(tui::layout::Alignment::Left),
                )
                .highlight_style(ctx.selected_style)
                .widths(&[
                    Constraint::Percentage(25),
                    Constraint::Percentage(25),
                    Constraint::Percentage(25),
                    Constraint::Percentage(25),
                ]);

            self.stream_table_state.select(Some(self.selected));

            frame.render_stateful_widget(table, rects[0], &mut self.stream_table_state);
        } else {
            let rects = Layout::default()
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
                .direction(Direction::Horizontal)
                .margin(3)
                .split(frame.size());

            for (idx, name) in HEADERS.iter().enumerate() {
                let header_cells = vec![Cell::from(*name).style(Style::default().fg(Color::Green))];
                let header = Row::new(header_cells)
                    .style(ctx.normal_style)
                    .height(1)
                    .bottom_margin(1);

                let cells = match idx {
                    0 => self.model.last_created.iter(),
                    _ => self.model.recently_changed.iter(),
                };

                if self.selected_tab == idx {
                    self.main_table_states[idx].select(Some(self.selected));
                } else {
                    self.main_table_states[idx].select(None);
                }

                let rows = cells
                    .map(|c| {
                        Row::new(vec![
                            Cell::from(c.as_str()).style(Style::default().fg(Color::Gray))
                        ])
                    })
                    .collect::<Vec<_>>();

                let table = Table::new(rows)
                    .header(header)
                    .block(Block::default().borders(Borders::ALL))
                    .highlight_style(ctx.selected_style)
                    .widths(&[Constraint::Percentage(100)]);

                frame.render_stateful_widget(table, rects[idx], &mut self.main_table_states[idx]);
            }
        }
    }

    fn on_key_pressed(&mut self, key: KeyCode) -> Request {
        match key {
            KeyCode::Left | KeyCode::Right => {
                self.selected_tab = (self.selected_tab + 1) % 2;
                self.selected = 0;
            }

            KeyCode::Up => {
                if self.selected > 0 {
                    self.selected -= 1;
                }
            }

            KeyCode::Down => {
                if self.stream_view_showing {
                    if self.selected < self.model.selected_stream_events.len() - 1 {
                        self.selected += 1;
                    }
                } else {
                    let len = if self.selected_tab == 0 {
                        self.model.last_created.len()
                    } else {
                        self.model.recently_changed.len()
                    };

                    if self.selected < len - 1 {
                        self.selected += 1;
                    }
                }
            }

            KeyCode::Enter => {
                if !self.stream_view_showing {
                    self.stream_view_showing = true;

                    let rows = if self.selected_tab == 0 {
                        &self.model.last_created
                    } else {
                        &self.model.recently_changed
                    };

                    self.model.selected_stream = Some(rows[self.selected].clone());
                    self.selected = 0;

                    return Request::Refresh;
                }
            }

            KeyCode::Esc => {
                if self.stream_view_showing {
                    self.stream_view_showing = false;
                    self.model.selected_stream = None;
                    self.selected = 0;
                    return Request::Refresh;
                }
            }

            _ => {}
        }

        Request::Noop
    }
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

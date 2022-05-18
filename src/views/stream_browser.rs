use crate::views::{Context, View};
use eventstore::StreamPosition;
use tui::backend::Backend;
use tui::layout::{Constraint, Direction, Layout};
use tui::style::{Color, Style};
use tui::widgets::{Block, Borders, Cell, Row, Table, TableState};
use tui::Frame;

static HEADERS: &[&'static str] = &["Recently Created Streams", "Recently Changed Streams"];
static STREAM_HEADERS: &[&'static str] = &["Event #", "Name", "Type", "Created Date", ""];

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
}

impl View for StreamsView {
    fn load(&mut self, ctx: &Context) {
        let client = ctx.client.clone();
        self.model = ctx
            .runtime
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
                    model
                        .recently_changed
                        .push(event.get_original_event().stream_id.clone());
                }

                Ok::<_, eventstore::Error>(model)
            })
            .unwrap();
    }

    fn unload(&mut self, ctx: &Context) {}

    fn refresh(&mut self, ctx: &Context) {}

    fn draw<B: Backend>(&mut self, ctx: &Context, frame: &mut Frame<B>) {
        if self.stream_view_showing {
            let rects = Layout::default()
                .constraints([Constraint::Percentage(100)].as_ref())
                .margin(3)
                .split(frame.size());

            let stream_name = if self.selected_tab == 0 {
                self.model.last_created[self.selected].as_str()
            } else {
                self.model.recently_changed[self.selected].as_str()
            };

            let header_cells = STREAM_HEADERS
                .iter()
                .map(|h| Cell::from(*h).style(Style::default().fg(Color::Green)));

            let header = Row::new(header_cells)
                .style(ctx.normal_style)
                .height(1)
                .bottom_margin(1);

            let rows = Vec::new();

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
                    Constraint::Percentage(20),
                    Constraint::Percentage(20),
                    Constraint::Percentage(20),
                    Constraint::Percentage(20),
                    Constraint::Percentage(20),
                ]);

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

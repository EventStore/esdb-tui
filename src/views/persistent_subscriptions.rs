use crate::models::PersistentSubscriptions;
use crate::views::{Env, ViewCtx};
use crate::{Request, View, B};
use crossterm::event::KeyCode;
use eventstore::{RevisionOrPosition, StreamPosition};
use tui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use tui::style::{Color, Modifier, Style};
use tui::widgets::{Block, Borders, Cell, Clear, Row, Table, TableState};
use tui::Frame;

use super::centered_rect;

static HEADERS: &[&'static str] = &[
    "Stream/Group",
    "Rate (messages/s)",
    "Messages (Known | Current | In Flight)",
    "Connections",
    "Status # of msgs / estimated time to catchup in seconds",
];

static SETTINGS_HEADERS: &[&'static str] = &[
    "Buffer Size",
    "Check Point After",
    "Extra Statistics",
    "Live Buffer Size",
    "Max Checkpoint Count",
    "Max Retry Count",
    "Message Timeout (ms)",
    "Min Checkpoint Count",
    "Consumer Strategy",
    "Read Batch Size",
    "Resolve Link Tos",
    "Start From",
];

#[derive(Copy, Clone, Eq, PartialEq)]
enum Stage {
    Main,
    Choices,
    Detail,
}

impl Default for Stage {
    fn default() -> Self {
        Stage::Main
    }
}

#[derive(Default)]
pub struct PersistentSubscriptionView {
    stage: Stage,
    main_table_state: TableState,
    choices_table_state: TableState,
    selected: u16,
    selected_choices: u16,
    model: PersistentSubscriptions,
}

impl PersistentSubscriptionView {
    fn draw_main(&mut self, ctx: ViewCtx, frame: &mut Frame<B>, area: Rect) {
        let rects = Layout::default()
            .constraints([Constraint::Min(0)].as_ref())
            .margin(2)
            .split(area);

        let header_cells = HEADERS
            .iter()
            .map(|h| Cell::from(*h).style(Style::default().fg(Color::Green)));

        let mut rows: Vec<Row> = Vec::new();
        for (key, sub) in self.model.list() {
            let mut cells = Vec::new();

            cells.push(Cell::from(key.as_str()));
            cells.push(Cell::from(format!("{:.1}", sub.average_items_per_second)));
            cells.push(Cell::from(format!(
                "{} | {} | {}",
                display_rev_or_pos(sub.last_known_event_position.as_ref()),
                display_rev_or_pos(sub.last_checkpointed_event_position.as_ref()),
                sub.in_flight_messages,
            )));
            cells.push(Cell::from(sub.connection_count.to_string()));
            cells.push(Cell::from(format!(
                "{} / {:.1}",
                sub.behind_by_messages, sub.behind_by_time
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
                    .borders(Borders::TOP)
                    .title("Persistent Subscriptions")
                    .title_alignment(tui::layout::Alignment::Right),
            )
            .highlight_style(ctx.selected_style)
            .widths(&[
                Constraint::Percentage(20),
                Constraint::Percentage(20),
                Constraint::Percentage(20),
                Constraint::Percentage(10),
                Constraint::Percentage(30),
            ]);

        self.main_table_state.select(Some(self.selected as usize));

        frame.render_stateful_widget(table, rects[0], &mut self.main_table_state);

        if self.stage == Stage::Choices {
            let block = Block::default()
                .title("Actions")
                .title_alignment(Alignment::Center)
                .borders(Borders::ALL)
                .style(Style::default().add_modifier(Modifier::REVERSED));
            let area = centered_rect(19, 22, frame.size());

            frame.render_widget(Clear, area);
            frame.render_widget(block, area);

            let layout = Layout::default()
                .margin(1)
                .constraints([Constraint::Percentage(100)])
                .direction(Direction::Vertical)
                .split(area)[0];

            let rows = vec![
                Row::new(vec![Cell::from("WIP - Edit")]),
                Row::new(vec![Cell::from("WIP - Delete")]),
                Row::new(vec![Cell::from("Detail")]),
                Row::new(vec![Cell::from("WIP - Replay Parked Messages")]),
                Row::new(vec![Cell::from("WIP - View Parked Messages")]),
            ];

            if self.selected_choices >= rows.len() as u16 {
                self.selected_choices = rows.len() as u16 - 1;
            }

            self.choices_table_state
                .select(Some(self.selected_choices as usize));

            let table = Table::new(rows)
                .highlight_style(Style::default().fg(Color::Green))
                .widths(&[Constraint::Percentage(100)]);

            frame.render_stateful_widget(table, layout, &mut self.choices_table_state);
        }
    }

    fn draw_detail(&mut self, ctx: ViewCtx, frame: &mut Frame<B>, area: Rect) {
        let rects = Layout::default()
            .constraints([Constraint::Min(0)].as_ref())
            .margin(2)
            .split(area);

        let header_cells = SETTINGS_HEADERS
            .iter()
            .map(|h| Cell::from(*h).style(Style::default().fg(Color::Green)));

        let mut rows: Vec<Row> = Vec::new();
        let p = self.model.get(self.selected).unwrap();
        let setts = p.settings.as_ref().unwrap();

        let mut cells = Vec::<Cell>::new();

        cells.push(Cell::from(setts.history_buffer_size.to_string()));
        cells.push(Cell::from(setts.checkpoint_after.as_millis().to_string()));
        cells.push(Cell::from(setts.extra_statistics.to_string()));
        cells.push(Cell::from(setts.live_buffer_size.to_string()));
        cells.push(Cell::from(setts.checkpoint_upper_bound.to_string()));
        cells.push(Cell::from(setts.max_retry_count.to_string()));
        cells.push(Cell::from(setts.message_timeout.as_millis().to_string()));
        cells.push(Cell::from(setts.checkpoint_lower_bound.to_string()));
        cells.push(Cell::from(setts.consumer_strategy_name.to_string()));
        cells.push(Cell::from(setts.read_batch_size.to_string()));
        cells.push(Cell::from(setts.resolve_link_tos.to_string()));
        cells.push(Cell::from(display_stream_position(&setts.start_from)));

        rows.push(Row::new(cells));

        let header = Row::new(header_cells)
            .style(ctx.normal_style)
            .height(1)
            .bottom_margin(1);

        let title = format!("Subscription - {}/{}", p.stream_name, p.group_name);

        let table = Table::new(rows)
            .header(header)
            .block(
                Block::default()
                    .borders(Borders::TOP)
                    .title(title)
                    .title_alignment(tui::layout::Alignment::Right),
            )
            .highlight_style(ctx.selected_style)
            .widths(&[
                Constraint::Percentage(8),
                Constraint::Percentage(8),
                Constraint::Percentage(8),
                Constraint::Percentage(8),
                Constraint::Percentage(8),
                Constraint::Percentage(8),
                Constraint::Percentage(8),
                Constraint::Percentage(8),
                Constraint::Percentage(8),
                Constraint::Percentage(8),
                Constraint::Percentage(8),
                Constraint::Percentage(8),
            ]);

        frame.render_stateful_widget(table, rects[0], &mut Default::default());
    }
}

fn display_rev_or_pos(value: Option<&RevisionOrPosition>) -> String {
    if let Some(value) = value {
        match value {
            RevisionOrPosition::Position(p) => p.to_string(),
            RevisionOrPosition::Revision(rev) => rev.to_string(),
        }
    } else {
        "0".to_string()
    }
}

impl View for PersistentSubscriptionView {
    fn load(&mut self, env: &Env) -> eventstore::Result<()> {
        self.refresh(env)
    }

    fn refresh(&mut self, env: &Env) -> eventstore::Result<()> {
        let client = env.client.clone();

        if self.stage == Stage::Main {
            let subs = env.handle.block_on(async move {
                client
                    .list_all_persistent_subscriptions(&Default::default())
                    .await
            })?;

            self.model.update(subs);
        }

        Ok(())
    }

    fn draw(&mut self, ctx: ViewCtx, frame: &mut Frame<B>, area: Rect) {
        match self.stage {
            Stage::Main | Stage::Choices => self.draw_main(ctx, frame, area),
            Stage::Detail => self.draw_detail(ctx, frame, area),
        }
    }

    fn on_key_pressed(&mut self, key: KeyCode) -> Request {
        match key {
            KeyCode::Char('q' | 'Q') => {
                if self.stage == Stage::Choices || self.stage == Stage::Detail {
                    self.stage = Stage::Main;
                    self.selected = 0;
                    self.selected_choices = 0;

                    return Request::Noop;
                }

                return Request::Exit;
            }

            KeyCode::Enter => {
                if self.stage == Stage::Main {
                    self.stage = Stage::Choices;
                } else if self.stage == Stage::Choices {
                    if self.selected_choices == 2 {
                        self.stage = Stage::Detail;
                    }
                }
            }

            KeyCode::Up => {
                if self.stage == Stage::Main {
                    if self.selected > 0 {
                        self.selected -= 1;
                    }
                } else if self.stage == Stage::Choices {
                    if self.selected_choices > 0 {
                        self.selected_choices -= 1;
                    }
                }
            }

            KeyCode::Down => {
                if self.stage == Stage::Main {
                    self.selected += 1;
                } else if self.stage == Stage::Choices {
                    self.selected_choices += 1;
                }
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

            Stage::Detail => &[("q", "Close")],

            Stage::Choices => &[
                ("↑", "Scroll up"),
                ("↓", "Scroll down"),
                ("Enter", "Select"),
                ("q", "Close"),
            ],
        }
    }
}

fn display_stream_position(value: &StreamPosition<RevisionOrPosition>) -> String {
    match value {
        StreamPosition::Start => "beginning".to_string(),
        StreamPosition::End => "end".to_string(),
        StreamPosition::Position(value) => display_rev_or_pos(Some(value)),
    }
}

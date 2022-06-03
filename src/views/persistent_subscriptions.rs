use crate::models::PersistentSubscriptions;
use crate::views::{Env, ViewCtx};
use crate::{Request, View, B};
use crossterm::event::KeyCode;
use eventstore::RevisionOrPosition;
use tui::layout::{Constraint, Layout, Rect};
use tui::style::{Color, Style};
use tui::widgets::{Block, Borders, Cell, Row, Table, TableState};
use tui::Frame;

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
    selected: usize,
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

        self.main_table_state.select(Some(self.selected));

        frame.render_stateful_widget(table, rects[0], &mut self.main_table_state);
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
        for (key, sub) in self.model.list() {
            let mut cells = Vec::<Cell>::new();

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
                    .title("Subscriptions")
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
            Stage::Main => self.draw_main(ctx, frame, area),
            Stage::Detail => self.draw_detail(ctx, frame, area),
        }
    }

    fn on_key_pressed(&mut self, key: KeyCode) -> Request {
        Request::Noop
    }
}

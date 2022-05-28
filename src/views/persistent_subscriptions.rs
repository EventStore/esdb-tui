use crate::views::{Env, ViewCtx};
use crate::{Request, View, B};
use crossterm::event::KeyCode;
use tui::layout::{Constraint, Layout, Rect};
use tui::style::{Color, Style};
use tui::widgets::{Block, Borders, Cell, Row, Table, TableState};
use tui::Frame;

static HEADERS: &[&'static str] = &[
    "Stream/Group",
    "Rate (messages/s)",
    "Messages",
    "Connections",
    "Status # of msgs / estimated time to catchup in seconds",
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

        // for proj in self.model.list() {
        //     rows.push(Row::new(main_proj_mapping(proj)));
        // }

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

    fn draw_detail(&mut self, ctx: ViewCtx, frame: &mut Frame<B>, area: Rect) {}
}

impl View for PersistentSubscriptionView {
    fn draw(&mut self, ctx: ViewCtx, frame: &mut Frame<B>, area: Rect) {
        match self.stage {
            Stage::Main => self.draw_main(ctx, frame, area),
            Stage::Detail => self.draw_detail(ctx, frame, area),
        }
    }
}

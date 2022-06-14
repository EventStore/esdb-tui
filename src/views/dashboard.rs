use crate::views::{Env, Request, View, ViewCtx, B};
use crossterm::event::KeyCode;
use eventstore::operations::Stats;
use eventstore_extras::stats::{Statistics, StatisticsExt};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tui::layout::{Constraint, Direction, Layout, Rect};
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

pub struct DashboardView {
    table_state: TableState,
    model: Statistics,
    stats: Arc<RwLock<Option<Stats>>>,
    scroll: u16,
}

impl Default for DashboardView {
    fn default() -> Self {
        Self {
            table_state: TableState::default(),
            model: Default::default(),
            stats: Arc::new(RwLock::new(None)),
            scroll: 0,
        }
    }
}

impl View for DashboardView {
    fn load(&mut self, env: &Env) -> eventstore::Result<()> {
        self.refresh(env)
    }

    fn unload(&mut self, _env: &Env) {}

    fn refresh(&mut self, env: &Env) -> eventstore::Result<()> {
        let client = env.op_client.clone();
        let state = self.stats.clone();

        self.model = env.handle.block_on(async move {
            let mut state = state.write().await;
            if state.is_none() {
                let options = eventstore::operations::StatsOptions::default()
                    .refresh_time(Duration::from_secs(2));

                *state = Some(client.stats(&options).await?);
            }

            state.as_mut().unwrap().next().await?.parse_statistics()
        })?;

        Ok(())
    }

    fn draw(&mut self, ctx: ViewCtx, frame: &mut Frame<B>, area: Rect) {
        let rect = Layout::default()
            .constraints([Constraint::Min(0)].as_ref())
            .direction(Direction::Vertical)
            .margin(2)
            .split(area)[0];

        let header_cells = HEADERS
            .iter()
            .map(|h| Cell::from(*h).style(Style::default().fg(Color::Green)));

        // 4 is the height taken by borders.
        if rect.height >= self.model.es.queues.len() as u16 + 4 {
            self.scroll = 0;
        } else if self.scroll + rect.height >= self.model.es.queues.len() as u16 + 4 {
            self.scroll = (self.model.es.queues.len() as u16 + 4) - rect.height;
        }

        let mut rows = Vec::new();
        let mut count = 0u16;
        for (idx, (name, queue)) in self.model.es.queues.iter().enumerate() {
            if count == rect.height {
                break;
            }

            if self.scroll > idx as u16 {
                continue;
            }

            count += 1;

            let mut cells = Vec::new();

            cells.push(Cell::from(name.as_str()));
            cells.push(Cell::from(format!(
                "{} | {}",
                queue.length_current_try_peak, queue.length_lifetime_peak
            )));

            let current_idle_time = if let Some(value) = queue.current_idle_time.as_ref() {
                value.as_str()
            } else {
                "N/A"
            };

            cells.push(Cell::from(queue.avg_items_per_second.to_string()));
            cells.push(Cell::from(current_idle_time));
            cells.push(Cell::from(queue.total_items_processed.to_string()));
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
                    .borders(Borders::TOP | Borders::BOTTOM)
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

        frame.render_stateful_widget(table, rect, &mut self.table_state)
    }

    fn on_key_pressed(&mut self, key: KeyCode) -> Request {
        match key {
            KeyCode::Char('q' | 'Q') => return Request::Exit,
            KeyCode::Up => {
                if self.scroll > 0 {
                    self.scroll -= 1;
                }
            }
            KeyCode::Down => {
                self.scroll += 1;
            }
            _ => {}
        }

        Request::Noop
    }

    fn keybindings(&self) -> &[(&str, &str)] {
        &[("↑", "Scroll up"), ("↓", "Scroll down")]
    }
}

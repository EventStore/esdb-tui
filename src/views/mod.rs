use crossterm::event::{KeyCode, KeyEvent};
use eventstore::ClientSettings;
use itertools::Itertools;
use std::collections::HashMap;
use std::io;
use std::io::Stdout;
use tokio::runtime::{Handle, Runtime};
use tui::backend::CrosstermBackend;
use tui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use tui::style::{Color, Modifier, Style};
use tui::text::{Span, Spans};
use tui::widgets::{Block, Borders, Clear, Paragraph, Tabs};
use tui::Frame;

pub mod dashboard;
pub mod projections;
pub mod stream_browser;

pub type B = CrosstermBackend<Stdout>;

static HEADERS: &[&'static str] = &[
    "Dashboard",
    "Streams Browser",
    "Projections",
    "Persistent Subscriptions",
];

static KEYBINDINGS: &[(&'static str, &'static str)] = &[
    ("TAB", "Next tab"),
    ("B/TAB", "Previous tab"),
    ("q", "Exit"),
];

pub struct Context {
    runtime: Runtime,
    view_ctx: ViewCtx,
    client: eventstore::Client,
    op_client: eventstore::operations::Client,
    proj_client: eventstore::ProjectionClient,
    selected_tab: usize,
    views: Vec<Box<dyn View>>,
    default_mappings: HashMap<String, String>,
    last_error: Option<eventstore::Error>,
}

#[derive(Clone)]
pub struct Env {
    handle: Handle,
    client: eventstore::Client,
    op_client: eventstore::operations::Client,
    proj_client: eventstore::ProjectionClient,
}

#[derive(Copy, Clone)]
pub struct ViewCtx {
    selected_style: Style,
    normal_style: Style,
}

impl Context {
    pub fn new(setts: ClientSettings) -> io::Result<Self> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()?;

        let (client, op_client, proj_client) = runtime
            .block_on(async move {
                let proj_client = eventstore::ProjectionClient::new(setts.clone());
                let client = eventstore::Client::new(setts)?;
                let op_client = eventstore::operations::Client::from(client.clone());

                Ok::<_, eventstore::Error>((client, op_client, proj_client))
            })
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

        let default_mappings = KEYBINDINGS
            .iter()
            .map(|(key, label)| (key.to_string(), label.to_string()))
            .collect();

        Ok(Self {
            default_mappings,
            runtime,
            client,
            op_client,
            proj_client,
            selected_tab: 0,
            last_error: None,
            views: vec![
                Box::new(dashboard::DashboardView::default()),
                Box::new(stream_browser::StreamsView::default()),
                Box::new(projections::ProjectionsViews::default()),
            ],
            view_ctx: ViewCtx {
                selected_style: Style::default().add_modifier(Modifier::REVERSED),
                normal_style: Style::default().add_modifier(Modifier::REVERSED),
            },
        })
    }

    fn mk_env(&self) -> Env {
        Env {
            handle: self.runtime.handle().clone(),
            client: self.client.clone(),
            op_client: self.op_client.clone(),
            proj_client: self.proj_client.clone(),
        }
    }

    pub fn on_key_pressed(&mut self, key: KeyEvent) -> Request {
        let env = self.mk_env();

        if self.last_error.is_some() {
            match key.code {
                KeyCode::Char('q' | 'Q') => {
                    return Request::Exit;
                }

                _ => {}
            }
        }

        match key.code {
            KeyCode::Tab => {
                if let Some(view) = self.views.get_mut(self.selected_tab) {
                    view.unload(&env);
                }

                self.selected_tab = (self.selected_tab + 1) % TABS.len();

                if let Some(view) = self.views.get_mut(self.selected_tab) {
                    if let Err(e) = view.load(&env) {
                        self.last_error = Some(e);
                    }
                }
            }
            KeyCode::BackTab => {
                if let Some(view) = self.views.get_mut(self.selected_tab) {
                    view.unload(&env);
                }

                if self.selected_tab == 0 {
                    self.selected_tab = TABS.len() - 1;
                } else {
                    self.selected_tab -= 1;
                }

                if let Some(view) = self.views.get_mut(self.selected_tab) {
                    if let Err(e) = view.load(&env) {
                        self.last_error = Some(e);
                    }
                }
            }
            _ => {
                if let Some(view) = self.views.get_mut(self.selected_tab) {
                    return view.on_key_pressed(key.code);
                }
            }
        }

        Request::Noop
    }

    pub fn refresh(&mut self) {
        let env = self.mk_env();
        if let Some(view) = self.views.get_mut(self.selected_tab) {
            if let Err(e) = view.refresh(&env) {
                self.last_error = Some(e);
            }
        }
    }

    pub fn draw(&mut self, frame: &mut Frame<B>) {
        let rects = Layout::default()
            .constraints([Constraint::Min(10), Constraint::Length(5)])
            .vertical_margin(0)
            .direction(Direction::Vertical)
            .split(frame.size());

        let titles = HEADERS
            .iter()
            .map(|t| {
                Spans::from(vec![Span::styled(
                    *t,
                    Style::default().fg(Color::LightGreen),
                )])
            })
            .collect();

        let tabs = Tabs::new(titles)
            .block(
                Block::default()
                    .borders(Borders::TOP | Borders::LEFT | Borders::RIGHT | Borders::BOTTOM)
                    .style(Style::default().bg(Color::DarkGray))
                    .title("EventStoreDB Administration Tool")
                    .title_alignment(Alignment::Right),
            )
            .select(self.selected_tab)
            .style(Style::default().fg(Color::Gray))
            .highlight_style(Style::default().add_modifier(Modifier::BOLD | Modifier::REVERSED));

        frame.render_widget(tabs, rects[0]);

        let mut mappings = self.default_mappings.clone();

        if let Some(view) = self.views.get_mut(self.selected_tab) {
            view.draw(self.view_ctx, frame, rects[0]);

            for (key, value) in view.keybindings() {
                mappings.insert(key.to_string(), value.to_string());
            }
        }

        let max_key = mappings
            .keys()
            .map(|k| k.chars().count())
            .max()
            .unwrap_or_default();

        let mut parts = vec![Vec::new(), Vec::new(), Vec::new()];
        let mut count = 0usize;

        for (mut key, mut label) in mappings.into_iter().sorted_by_key(|k| k.0.clone()).rev() {
            let key_count = key.chars().count();
            let idx = count % 3;

            if key_count < max_key {
                for _ in 0..max_key - key_count {
                    key.insert(0, ' ');
                }
            }

            let key_count = key.chars().count();
            let label_count = label.chars().count();

            for _ in 0..(20 - (key_count + label_count)) {
                label.push(' ');
            }

            parts[idx].push(Span::styled(key, Style::default().fg(Color::Green)));
            parts[idx].push(Span::styled(
                format!(" {}", label),
                Style::default().fg(Color::Gray),
            ));

            count += 1;
        }

        let paragraph = Paragraph::new(parts.into_iter().map(|xs| Spans(xs)).collect::<Vec<_>>())
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .style(Style::default().bg(Color::DarkGray)),
            )
            .alignment(Alignment::Left);

        frame.render_widget(paragraph, rects[1]);

        if let Some(e) = self.last_error.as_ref() {
            let block = Block::default()
                .title("Panic")
                .title_alignment(Alignment::Center)
                .borders(Borders::ALL)
                .style(Style::default().bg(Color::Black).fg(Color::Red));
            let area = centered_rect(40, 15, frame.size());
            frame.render_widget(Clear, area);
            frame.render_widget(block, area);

            let rect = Layout::default()
                .margin(2)
                .constraints([Constraint::Percentage(100)])
                .direction(Direction::Horizontal)
                .split(area)[0];

            let label = Paragraph::new(e.to_string()).style(Style::default().fg(Color::Gray));

            frame.render_widget(label, rect);
        }
    }

    pub fn init(&mut self) {
        if let Some(view) = self.views.get_mut(self.selected_tab) {
            let env = Env {
                handle: self.runtime.handle().clone(),
                client: self.client.clone(),
                op_client: self.op_client.clone(),
                proj_client: self.proj_client.clone(),
            };

            if let Err(e) = view.load(&env) {
                self.last_error = Some(e);
            }
        }
    }
}

pub trait View {
    fn load(&mut self, env: &Env) -> eventstore::Result<()>;
    fn unload(&mut self, env: &Env);
    fn refresh(&mut self, env: &Env) -> eventstore::Result<()>;
    fn draw(&mut self, ctx: ViewCtx, frame: &mut Frame<B>, area: Rect);
    fn on_key_pressed(&mut self, key: KeyCode) -> Request;
    fn keybindings(&self) -> &[(&str, &str)];
}

#[derive(Debug, Copy, Clone)]
pub enum MainTab {
    Dashboard,
    StreamsBrowser,
    Projections,
    PersistentSubscriptions,
}

static TABS: &[MainTab] = &[
    MainTab::Dashboard,
    MainTab::StreamsBrowser,
    MainTab::Projections,
    MainTab::PersistentSubscriptions,
];

pub enum Request {
    Noop,
    Refresh,
    Exit,
}

/// helper function to create a centered rect using up certain percentage of the available rect `r`
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            [
                Constraint::Percentage((100 - percent_y) / 2),
                Constraint::Percentage(percent_y),
                Constraint::Percentage((100 - percent_y) / 2),
            ]
            .as_ref(),
        )
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints(
            [
                Constraint::Percentage((100 - percent_x) / 2),
                Constraint::Percentage(percent_x),
                Constraint::Percentage((100 - percent_x) / 2),
            ]
            .as_ref(),
        )
        .split(popup_layout[1])[1]
}

fn render_line_numbers(content: &str) -> String {
    let line_count = content.lines().count();
    let num_width = line_count.to_string().chars().count();
    let mut buffer = String::new();

    for (idx, line) in content.lines().enumerate() {
        let line_num = idx + 1;
        let line_num_count = line_num.to_string().chars().count();

        for _ in 0..num_width - line_num_count {
            buffer.push(' ');
        }

        buffer.push_str(format!("{} | ", line_num).as_str());
        buffer.push_str(line);

        if line_num != line_count {
            buffer.push('\n');
        }
    }

    buffer
}

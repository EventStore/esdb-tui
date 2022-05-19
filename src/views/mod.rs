use crossterm::event::{KeyCode, KeyEvent};
use eventstore::ClientSettings;
use log::{debug, error};
use std::io;
use std::io::Stdout;
use std::time::{Duration, Instant};
use tokio::runtime::{Handle, Runtime};
use tui::backend::{Backend, CrosstermBackend};
use tui::style::{Color, Modifier, Style};
use tui::text::{Span, Spans};
use tui::widgets::{Block, Borders, Tabs};
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

pub struct Context {
    runtime: Runtime,
    view_ctx: ViewCtx,
    client: eventstore::Client,
    op_client: eventstore::operations::Client,
    proj_client: eventstore::ProjectionClient,
    selected_tab: usize,
    views: Vec<Box<dyn View>>,
    time: Instant,
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

        Ok(Self {
            runtime,
            client,
            op_client,
            proj_client,
            selected_tab: 0,
            time: Instant::now(),
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

    pub fn runtime(&self) -> &Runtime {
        &self.runtime
    }

    fn mk_env(&self) -> Env {
        Env {
            handle: self.runtime.handle().clone(),
            client: self.client.clone(),
            op_client: self.op_client.clone(),
            proj_client: self.proj_client.clone(),
        }
    }

    pub fn on_key_pressed(&mut self, key: KeyEvent) -> bool {
        let env = self.mk_env();

        let previous = self.selected_tab;

        match key.code {
            KeyCode::Char('q') => {
                return false;
            }
            KeyCode::Tab => {
                if let Some(view) = self.views.get_mut(self.selected_tab) {
                    view.unload(&env);
                }

                self.selected_tab = (self.selected_tab + 1) % TABS.len();

                if let Some(view) = self.views.get_mut(self.selected_tab) {
                    view.load(&env);
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
                    view.load(&env);
                }
            }
            _ => {
                if let Some(view) = self.views.get_mut(self.selected_tab) {
                    view.on_key_pressed(key.code);
                }
            }
        }

        if self.selected_tab == previous && self.time.elapsed() >= Duration::from_secs(1) {
            self.time = Instant::now();

            if let Some(view) = self.views.get_mut(self.selected_tab) {
                view.refresh(&env);
            }
        }

        true
    }

    pub fn refresh(&mut self) {
        let env = self.mk_env();
        if let Some(view) = self.views.get_mut(self.selected_tab) {
            view.refresh(&env);
        }
    }

    pub fn draw(&mut self, frame: &mut Frame<B>) {
        let size = frame.size();

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
                    .borders(Borders::ALL)
                    .style(Style::default().bg(Color::DarkGray))
                    .title("EventStoreDB Administration Tool")
                    .title_alignment(tui::layout::Alignment::Right),
            )
            .select(self.selected_tab)
            .style(Style::default().fg(Color::Gray))
            .highlight_style(Style::default().add_modifier(Modifier::BOLD | Modifier::REVERSED));

        frame.render_widget(tabs, size);

        if let Some(view) = self.views.get_mut(self.selected_tab) {
            view.draw(self.view_ctx, frame);
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

            view.load(&env);
        }
    }
}

pub trait View {
    fn load(&mut self, env: &Env);
    fn unload(&mut self, env: &Env);
    fn refresh(&mut self, env: &Env);
    fn draw(&mut self, ctx: ViewCtx, frame: &mut Frame<B>);
    fn on_key_pressed(&mut self, key: KeyCode) -> bool;
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

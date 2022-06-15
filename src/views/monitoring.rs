use std::{sync::Arc, time::Duration};

use eventstore::operations::{Stats, StatsOptions};
use eventstore_extras::stats::StatisticsExt;
use tokio::sync::{Mutex, RwLock};
use tui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    symbols::Marker,
    text::{Span, Spans},
    widgets::{Axis, Block, Borders, Chart, Dataset, GraphType, Paragraph},
    Frame,
};

use crate::models::Monitoring;

use super::{Env, View, B};

pub struct MonitoringView {
    model: Monitoring,
    stats_iter: Arc<RwLock<Option<Stats>>>,
}

impl Default for MonitoringView {
    fn default() -> Self {
        Self {
            model: Default::default(),
            stats_iter: Arc::new(RwLock::new(None)),
        }
    }
}

impl MonitoringView {
    fn draw_key_metrics(&mut self, frame: &mut tui::Frame<super::B>, area: Rect) {
        let mut spans = Vec::<Spans>::new();

        let epoch_num_label = self
            .model
            .last_epoch_number
            .map(|v| v.to_string())
            .unwrap_or_else(|| "N/A".to_string());

        let writer_chk_label = self
            .model
            .last_writer_checkpoint
            .map(|v| v.to_string())
            .unwrap_or_else(|| "N/A".to_string());

        let esdb_version = format!(
            "v{}.{}.{}",
            self.model.server_version.major(),
            self.model.server_version.minor(),
            self.model.server_version.patch()
        );

        let values = vec![
            ("Epoch number", epoch_num_label),
            ("Writer checkpoint", writer_chk_label),
            ("Elections", self.model.elections.to_string()),
            (
                "Out of syncs",
                self.model.out_of_sync_cluster_counter.to_string(),
            ),
            (
                "Unresponsive nodes",
                self.model.unresponsive_nodes.to_string(),
            ),
            ("Free memory", format!("{:.2} GB", self.model.free_mem)),
            ("Version", esdb_version),
        ];

        let max_chars = values
            .iter()
            .fold(0usize, |acc, (key, _)| acc.max(key.chars().count()));

        for (key, value) in values {
            let mut key = key.to_string();

            for _ in 0..max_chars - key.chars().count() {
                key.push(' ');
            }

            key.push_str(": ");

            spans.push(Spans(vec![Span::raw(key), Span::raw(value)]));
        }

        let paragraph = Paragraph::new(spans)
            .block(
                Block::default()
                    .borders(Borders::TOP)
                    .title("Key metrics")
                    .title_alignment(Alignment::Center),
            )
            .alignment(Alignment::Left);

        frame.render_widget(paragraph, area);
    }

    fn draw_env_metrics(&mut self, frame: &mut Frame<B>, area: Rect) {
        let sections = Layout::default()
            .constraints([Constraint::Percentage(80), Constraint::Percentage(20)].as_ref())
            .direction(Direction::Vertical)
            .margin(1)
            .split(area);

        let mut datasets = Vec::new();

        datasets.push(
            Dataset::default()
                .data(self.model.cpu_load.as_ref())
                .marker(Marker::Dot)
                .graph_type(GraphType::Line)
                .style(Style::default().fg(Color::Green)),
        );

        let time_bounds = self.model.time_bounds();
        let time_labels = vec![
            Span::raw(time_bounds[0].to_string()),
            Span::raw(time_bounds[1].to_string()),
        ];

        let chart = Chart::new(datasets)
            .block(
                Block::default()
                    .title("CPU Usage")
                    .title_alignment(Alignment::Right)
                    .borders(Borders::NONE),
            )
            .style(Style::default().bg(Color::DarkGray))
            .x_axis(
                Axis::default()
                    .title("Time (secs)")
                    .style(Style::default().fg(Color::White))
                    .labels(time_labels)
                    .bounds(self.model.time_period()),
            )
            .y_axis(
                Axis::default()
                    .title("Value")
                    .style(Style::default().fg(Color::White))
                    .labels(vec![Span::raw("0%"), Span::raw("100%")])
                    .bounds([0f64, 100f64]),
            );

        frame.render_widget(chart, sections[0]);

        // let mut legend = Vec::<Spans>::new();

        // legend.push(Spans(vec![
        //     Span::styled(" ", Style::default().bg(Color::Green)),
        //     Span::raw(" Writer checkpoint"),
        // ]));

        // let legend = Paragraph::new(legend);

        // frame.render_widget(legend, rects[1]);
    }

    fn draw_drive_metrics(&mut self, frame: &mut Frame<B>, area: Rect) {
        if let Some(drive) = self.model.drive.as_ref() {
            let sections = Layout::default()
                .constraints(
                    [
                        Constraint::Percentage(60),
                        Constraint::Percentage(10),
                        Constraint::Percentage(30),
                    ]
                    .as_ref(),
                )
                .direction(Direction::Horizontal)
                .margin(2)
                .split(area);

            let bytes_written_bounds = self.model.bytes_written_value_bounds();
            let bytes_written_diff = (bytes_written_bounds[1] - bytes_written_bounds[0]).round();

            let mut incr = 10;
            let scale = loop {
                if (bytes_written_diff - incr as f64) < 0f64 {
                    break (incr / 2) as f64;
                }

                incr *= 10;
            };

            let min_bound = if bytes_written_bounds[0] - scale < 0.0 {
                0f64
            } else {
                bytes_written_bounds[0] - scale
            };

            let max_bound = if bytes_written_bounds[0] - scale < 0.0 {
                bytes_written_bounds[1]
            } else {
                bytes_written_bounds[1] + scale
            };

            let bytes_written_bounds = [min_bound, max_bound];

            let mut datasets = Vec::new();
            datasets.push(
                Dataset::default()
                    .data(self.model.bytes_written.as_ref())
                    .marker(Marker::Dot)
                    .graph_type(GraphType::Line)
                    .style(Style::default().fg(Color::Green)),
            );

            let time_bounds = self.model.time_bounds();
            let time_labels = vec![
                Span::raw(time_bounds[0].to_string()),
                Span::raw(time_bounds[1].to_string()),
            ];

            let chart = Chart::new(datasets)
                .block(
                    Block::default()
                        .title("Bytes written")
                        .title_alignment(Alignment::Right)
                        .borders(Borders::NONE),
                )
                .style(Style::default().bg(Color::DarkGray))
                .x_axis(
                    Axis::default()
                        .title("Time (secs)")
                        .style(Style::default().fg(Color::White))
                        .labels(time_labels)
                        .bounds(self.model.time_period()),
                )
                .y_axis(
                    Axis::default()
                        .title("Value")
                        .style(Style::default().fg(Color::White))
                        .labels(vec![
                            Span::raw(format!("{:.2}", bytes_written_bounds[0])),
                            Span::raw(format!("{:.2}", bytes_written_bounds[1])),
                        ])
                        .bounds(bytes_written_bounds),
                );

            frame.render_widget(chart, sections[0]);

            let total = drive.stats.total_bytes as f64 / 1_073_741_824f64;
            let available = drive.stats.available_bytes as f64 / 1_073_741_824f64;
            let used = drive.stats.used_bytes as f64 / 1_073_741_824f64;
            let values = vec![
                ("Directory", drive.path.clone()),
                ("Total", format!("{:.2} GB", total)),
                ("Available", format!("{:.2} GB", available)),
                ("Used", format!("{:.2} GB ({})", used, drive.stats.usage)),
            ];

            let mut spans = Vec::new();

            let max_chars = values
                .iter()
                .fold(0usize, |acc, (key, _)| acc.max(key.chars().count()));

            for (key, value) in values {
                let mut key = key.to_string();

                for _ in 0..max_chars - key.chars().count() {
                    key.push(' ');
                }

                key.push_str(": ");

                spans.push(Spans(vec![Span::raw(key), Span::raw(value)]));
            }
            let paragraph = Paragraph::new(spans)
                .block(
                    Block::default()
                        .borders(Borders::TOP)
                        .title("Drive metrics")
                        .title_alignment(Alignment::Center),
                )
                .alignment(Alignment::Left);

            frame.render_widget(paragraph, sections[2]);
        }
    }
}

impl View for MonitoringView {
    fn load(&mut self, env: &Env) -> eventstore::Result<()> {
        let client = env.op_client.clone();

        self.model.server_version = env
            .handle
            .block_on(async move {
                let info = client.server_version().await?;

                Ok(info.map(|i| i.version()))
            })?
            .unwrap_or_default();

        self.refresh(env)
    }

    fn refresh(&mut self, env: &Env) -> eventstore::Result<()> {
        let client = env.op_client.clone();
        let stats_ref = self.stats_iter.clone();

        let (gossip, stats) = env.handle.block_on(async move {
            let members = client.read_gossip().await?;

            let mut stats_ref = stats_ref.write().await;

            if stats_ref.is_none() {
                let options = StatsOptions::default().refresh_time(Duration::from_secs(2));
                *stats_ref = Some(client.stats(&options).await?);
            }

            let stats = stats_ref
                .as_mut()
                .unwrap()
                .next()
                .await?
                .parse_statistics()?;

            Ok((members, stats))
        })?;

        self.model.update(stats, gossip);

        Ok(())
    }

    fn draw(
        &mut self,
        ctx: super::ViewCtx,
        frame: &mut tui::Frame<super::B>,
        area: tui::layout::Rect,
    ) {
        let vert_rects = Layout::default()
            .constraints([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)].as_ref())
            .direction(Direction::Vertical)
            .margin(2)
            .split(area);

        let top_sections = Layout::default()
            .constraints(
                [
                    Constraint::Percentage(60),
                    Constraint::Percentage(10),
                    Constraint::Percentage(30),
                ]
                .as_ref(),
            )
            .direction(Direction::Horizontal)
            .margin(2)
            .split(vert_rects[0]);

        self.draw_env_metrics(frame, top_sections[0]);
        self.draw_key_metrics(frame, top_sections[2]);
        self.draw_drive_metrics(frame, vert_rects[1]);

        // let mut datasets = Vec::<Dataset>::new();
        //
        // let writer_bounds = self.model.writer_checkpoint_value_bounds();
        // let writer_diff = (writer_bounds[1] - writer_bounds[0]).round();
        //
        // let mut incr = 10;
        // let scale = loop {
        //     if (writer_diff - incr as f64) < 0f64 {
        //         break (incr / 2) as f64;
        //     }
        //
        //     incr *= 10;
        // };
        // let writer_bounds = [writer_bounds[0] - scale, writer_bounds[1] + scale];
        //
        // datasets.push(
        //     Dataset::default()
        //         .data(self.model.writer_checkpoints.as_ref())
        //         .marker(Marker::Dot)
        //         .graph_type(GraphType::Line)
        //         .style(Style::default().fg(Color::Green)),
        // );
        //
        // let time_bounds = self.model.time_bounds();
        // let mut time_labels = Vec::new();
        //
        // for x in time_bounds[0]..time_bounds[1] {
        //     if x % 2 == 0 {
        //         time_labels.push(Span::raw(x.to_string()));
        //     }
        // }
        //
        // let chart = Chart::new(datasets)
        //     .block(
        //         Block::default()
        //             .title("Monitoring")
        //             .title_alignment(Alignment::Right)
        //             .borders(Borders::TOP),
        //     )
        //     .style(Style::default().bg(Color::DarkGray))
        //     .x_axis(
        //         Axis::default()
        //             .title("Time (secs)")
        //             .style(Style::default().fg(Color::White))
        //             .labels(time_labels)
        //             .bounds(self.model.time_period()),
        //     )
        //     .y_axis(
        //         Axis::default()
        //             .title("Value")
        //             .style(Style::default().fg(Color::White))
        //             .labels(vec![
        //                 Span::raw((writer_bounds[0] as u64).to_string()),
        //                 Span::raw((writer_bounds[1] as u64).to_string()),
        //             ])
        //             .bounds(writer_bounds),
        //     );
        //
        // frame.render_widget(chart, rects[0]);
        //
        // let mut legend = Vec::<Spans>::new();
        //
        // legend.push(Spans(vec![
        //     Span::styled(" ", Style::default().bg(Color::Green)),
        //     Span::raw(" Writer checkpoint"),
        // ]));
        //
        // let legend = Paragraph::new(legend);
        //
        // frame.render_widget(legend, rects[1]);
    }
}

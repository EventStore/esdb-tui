use tui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    symbols::Marker,
    text::{Span, Spans},
    widgets::{Axis, Block, Borders, Chart, Dataset, GraphType, Paragraph},
};

use crate::models::Monitoring;

use super::{Env, View};

#[derive(Default)]
pub struct MonitoringView {
    model: Monitoring,
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

        let values = vec![
            ("Epoch number", epoch_num_label),
            ("Writer checkpoint", writer_chk_label),
            ("Elections", self.model.elections.to_string()),
            (
                "Out of syncs",
                self.model.out_of_sync_cluster_counter.to_string(),
            ),
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
                    .borders(Borders::ALL)
                    .title("Key metrics")
                    .title_alignment(Alignment::Right),
            )
            .alignment(Alignment::Left);

        frame.render_widget(paragraph, area);
    }
}

impl View for MonitoringView {
    fn load(&mut self, env: &Env) -> eventstore::Result<()> {
        self.refresh(env)
    }

    fn refresh(&mut self, env: &Env) -> eventstore::Result<()> {
        let client = env.op_client.clone();

        let gossip = env.handle.block_on(async move {
            let members = client.read_gossip().await?;

            Ok(members)
        })?;

        self.model.update(gossip);

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
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
            .direction(Direction::Horizontal)
            .margin(2)
            .split(vert_rects[0]);

        self.draw_key_metrics(frame, top_sections[1]);

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

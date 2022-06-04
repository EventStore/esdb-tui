use tui::{
    layout::{Alignment, Constraint, Direction, Layout},
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
        let rects = Layout::default()
            .constraints([Constraint::Percentage(90), Constraint::Percentage(10)].as_ref())
            .direction(Direction::Vertical)
            .margin(2)
            .split(area);

        let mut datasets = Vec::<Dataset>::new();

        let writer_bounds = self.model.writer_checkpoint_value_bounds();
        let writer_diff = (writer_bounds[1] - writer_bounds[0]).round();

        let mut incr = 10;
        let scale = loop {
            if (writer_diff - incr as f64) < 0f64 {
                break (incr / 2) as f64;
            }

            incr *= 10;
        };
        let writer_bounds = [writer_bounds[0] - scale, writer_bounds[1] + scale];

        datasets.push(
            Dataset::default()
                .data(self.model.writer_checkpoints.as_ref())
                .marker(Marker::Dot)
                .graph_type(GraphType::Line)
                .style(Style::default().fg(Color::Green)),
        );

        let time_bounds = self.model.time_bounds();
        let mut time_labels = Vec::new();

        for x in time_bounds[0]..time_bounds[1] {
            if x % 2 == 0 {
                time_labels.push(Span::raw(x.to_string()));
            }
        }

        let chart = Chart::new(datasets)
            .block(
                Block::default()
                    .title("Monitoring")
                    .title_alignment(Alignment::Right)
                    .borders(Borders::TOP),
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
                        Span::raw((writer_bounds[0] as u64).to_string()),
                        Span::raw((writer_bounds[1] as u64).to_string()),
                    ])
                    .bounds(writer_bounds),
            );

        frame.render_widget(chart, rects[0]);

        let mut legend = Vec::<Spans>::new();

        legend.push(Spans(vec![
            Span::styled(" ", Style::default().bg(Color::Green)),
            Span::raw(" Writer checkpoint"),
        ]));

        let legend = Paragraph::new(legend);

        frame.render_widget(legend, rects[1]);
    }
}

use tui::{
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Style},
    symbols::Marker,
    text::Span,
    widgets::{Axis, Block, Borders, Chart, Dataset, GraphType},
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
        self.model.update();

        Ok(())
    }

    fn draw(
        &mut self,
        ctx: super::ViewCtx,
        frame: &mut tui::Frame<super::B>,
        area: tui::layout::Rect,
    ) {
        let rects = Layout::default()
            .constraints([Constraint::Percentage(80), Constraint::Percentage(20)].as_ref())
            .direction(Direction::Vertical)
            .margin(2)
            .split(area);

        let mut datasets = Vec::<Dataset>::new();

        datasets.push(
            Dataset::default()
                .data(self.model.epoch_numbers.as_ref())
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
                    .labels(vec![Span::raw("-20"), Span::raw("20")])
                    .bounds([-20f64, 20f64]),
            );

        frame.render_widget(chart, rects[0]);
    }
}

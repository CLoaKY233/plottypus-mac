use plottypus_core::{History, Scale, Thermal, bits_per_sec};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Paragraph};

use crate::braille::{peak_column, render_cells};
use crate::layout::Panel;
use crate::spark;
use crate::theme::Theme;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    None,
    Percent,
    Bits,
    Celsius,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphInk {
    Flat,
    Load(Thermal),
}

#[must_use]
pub fn panel_block<'a>(
    _panel: Panel,
    title: Line<'a>,
    focused: bool,
    expanded: bool,
    theme: &Theme,
) -> Block<'a> {
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(theme.border(focused || expanded))
        .style(Style::default().bg(theme.bg).fg(theme.fg))
        .title(title);
    let mark = if expanded { " × " } else { " ↗ " };
    block.title(Line::from(Span::styled(mark, theme.title())).right_aligned())
}

#[derive(Debug, Clone, Copy)]
pub struct Graph<'a> {
    pub history: &'a History,
    pub accent: Color,
    pub theme: &'a Theme,
    pub scale: Scale,
    pub axis: Axis,
    pub ink: GraphInk,
}

impl Graph<'_> {
    fn max(&self) -> f32 {
        self.history.scale(self.scale)
    }

    fn thermal(&self) -> Thermal {
        match self.ink {
            GraphInk::Flat => Thermal::Nominal,
            GraphInk::Load(thermal) => thermal,
        }
    }
}

pub fn render_scaled_graph(frame: &mut Frame, area: Rect, g: Graph<'_>) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let max = g.max();
    let gutter = axis_gutter(g.axis, area.width, area.height);
    let plot = Rect {
        x: area.x.saturating_add(gutter),
        y: area.y,
        width: area.width.saturating_sub(gutter),
        height: area.height,
    };
    if plot.width == 0 {
        return;
    }
    if plot.height == 1 {
        frame.render_widget(
            spark::widget_scaled(g.history, plot.width, max, Style::default().fg(g.accent)),
            plot,
        );
        return;
    }
    let thermal = g.thermal();
    let rows = render_cells(g.history, plot.width, plot.height, max);
    for (i, row) in rows.iter().enumerate() {
        let y = plot.y.saturating_add(i as u16);
        if y >= plot.y.saturating_add(plot.height) {
            break;
        }
        let spans: Vec<Span> = row
            .iter()
            .map(|cell| {
                if cell.glyph == '\u{2800}' {
                    Span::raw(" ")
                } else {
                    Span::styled(
                        cell.glyph.to_string(),
                        g.theme.stain(g.accent, cell.intensity, thermal),
                    )
                }
            })
            .collect();
        frame.render_widget(
            Paragraph::new(Line::from(spans)),
            Rect {
                x: plot.x,
                y,
                width: plot.width,
                height: 1,
            },
        );
    }
    if g.ink != GraphInk::Flat
        && plot.height >= 2
        && let Some(col) = peak_column(g.history, plot.width, max)
    {
        let pip = Rect {
            x: plot.x + col as u16,
            y: plot.y,
            width: 1,
            height: 1,
        };
        frame.render_widget(
            Paragraph::new(Span::styled("▲", Style::default().fg(g.theme.title))),
            pip,
        );
    }
    if gutter > 0 {
        render_axis_ticks(frame, area, gutter, max, g.axis, g.theme);
    }
}

#[must_use]
pub fn axis_gutter(axis: Axis, width: u16, height: u16) -> u16 {
    if height < 2 {
        return 0;
    }
    let need: u16 = match axis {
        Axis::None => 0,
        Axis::Percent | Axis::Celsius => 5,
        Axis::Bits => 7,
    };
    if need == 0 || width <= need.saturating_add(6) {
        0
    } else {
        need
    }
}

fn render_axis_ticks(
    frame: &mut Frame,
    area: Rect,
    gutter: u16,
    max: f32,
    axis: Axis,
    theme: &Theme,
) {
    let ticks: &[(f32, u16)] = if area.height >= 5 {
        &[
            (1.0, area.y),
            (0.75, area.y + area.height / 4),
            (0.5, area.y + area.height / 2),
            (0.25, area.y + area.height * 3 / 4),
            (0.0, area.y + area.height.saturating_sub(1)),
        ]
    } else if area.height >= 3 {
        &[
            (1.0, area.y),
            (0.5, area.y + area.height / 2),
            (0.0, area.y + area.height.saturating_sub(1)),
        ]
    } else {
        &[(1.0, area.y)]
    };
    let width = gutter.min(area.width);
    if width == 0 {
        return;
    }
    let w = usize::from(width);
    for (frac, y) in ticks {
        if *y >= area.y + area.height {
            continue;
        }
        let label = axis_label(max * frac, axis);
        frame.render_widget(
            Paragraph::new(Span::styled(format!("{label:>w$}"), theme.dim())),
            Rect {
                x: area.x,
                y: *y,
                width,
                height: 1,
            },
        );
    }
}

fn axis_label(value: f32, axis: Axis) -> String {
    match axis {
        Axis::Percent => format!("{:.0}%", (value * 100.0).round()),
        Axis::Bits => bits_per_sec(value.max(0.0) as u64),
        Axis::Celsius => format!("{:.0}°", value.round()),
        Axis::None => String::new(),
    }
}

pub fn render_fill_bar(frame: &mut Frame, area: Rect, ratio: f32, color: Color) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let ratio = ratio.clamp(0.0, 1.0);
    let filled = ((f32::from(area.width) * ratio).round() as u16).min(area.width);
    let mut line = String::new();
    for x in 0..area.width {
        line.push(if x < filled { '━' } else { '─' });
    }
    frame.render_widget(
        Paragraph::new(Span::styled(line, Style::default().fg(color))),
        Rect {
            x: area.x,
            y: area.y,
            width: area.width,
            height: 1,
        },
    );
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use ratatui::text::Span;

    #[test]
    fn expanded_block_has_close_mark() {
        let theme = Theme::default();
        let block = panel_block(
            Panel::Cpu,
            Line::from(Span::raw(" cpu ")),
            true,
            true,
            &theme,
        );
        let _ = block;
    }

    #[test]
    fn percent_axis_labels() {
        assert_eq!(axis_label(1.0, Axis::Percent), "100%");
        assert_eq!(axis_label(0.5, Axis::Percent), "50%");
    }

    #[test]
    fn gutter_only_when_the_plot_still_fits() {
        assert_eq!(axis_gutter(Axis::Celsius, 40, 8), 5);
        assert_eq!(axis_gutter(Axis::Percent, 40, 8), 5);
        assert_eq!(axis_gutter(Axis::None, 40, 8), 0);
        assert_eq!(axis_gutter(Axis::Celsius, 10, 8), 0);
        assert_eq!(axis_gutter(Axis::Celsius, 40, 1), 0);
    }

    #[test]
    fn celsius_ticks_do_not_sit_on_braille() {
        use plottypus_core::History;
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;

        let mut history = History::default();
        for _ in 0..48 {
            history.push(80.0);
        }
        let backend = TestBackend::new(28, 6);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                render_scaled_graph(
                    frame,
                    frame.area(),
                    Graph {
                        history: &history,
                        accent: Theme::default().temp,
                        theme: &Theme::default(),
                        scale: Scale::Fixed(100.0),
                        axis: Axis::Celsius,
                        ink: GraphInk::Flat,
                    },
                );
            })
            .unwrap();
        let buf = terminal.backend().buffer();
        let mut out = String::new();
        for y in 0..buf.area.height {
            for x in 0..buf.area.width {
                out.push_str(buf[(x, y)].symbol());
            }
            out.push('\n');
        }
        assert!(out.contains('°'), "{out}");
        for y in 0..buf.area.height {
            for x in 0..5 {
                let ch = buf[(x, y)].symbol();
                assert!(
                    ch.chars().all(|c| !('\u{2800}'..='\u{28FF}').contains(&c)),
                    "braille in gutter y={y} x={x}: {out}"
                );
            }
        }
    }
}

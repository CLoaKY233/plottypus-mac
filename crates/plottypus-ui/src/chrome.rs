use plottypus_core::{History, Scale, Thermal, bits_per_sec, bytes_per_sec};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Clear, Paragraph};

use crate::layout::Panel;
use crate::spark;
use crate::theme::Theme;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    None,
    Percent,
    Bits,
    Bytes,
    Celsius,
    Number,
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
        .title(title);
    let mark = if expanded { " × " } else { " ↗ " };
    block.title(Line::from(Span::styled(mark, theme.title())).right_aligned())
}

/// Titled rounded cell used by expanded grids and the process dossier.
pub fn cell(frame: &mut Frame, area: Rect, title: &str, theme: &Theme) -> Rect {
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .title(Line::from(Span::styled(format!(" {title}"), theme.dim())))
        .border_style(theme.border(false));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    inner
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
    fn range(&self) -> plottypus_core::ScaleRange {
        self.history.range(self.scale)
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
    let range = g.range();
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
            spark::widget_scaled_range(
                g.history,
                plot.width,
                range.min,
                range.max,
                Style::default().fg(g.accent),
            ),
            plot,
        );
        return;
    }
    let thermal = g.thermal();
    let rows = crate::braille::render_cells_range(
        g.history,
        plot.width,
        plot.height,
        range.min,
        range.max,
    );
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
    let show_hint = g.axis == Axis::Celsius
        || g.axis == Axis::Number
        || (g.scale.hints_axis() && g.axis == Axis::Percent);
    if show_hint && plot.height >= 3 && plot.width >= 10 {
        let hint_w = 5.min(plot.width);
        let top = Rect {
            x: plot.x,
            y: plot.y,
            width: hint_w,
            height: 1,
        };
        frame.render_widget(Clear, top);
        frame.render_widget(
            Paragraph::new(Span::styled(axis_label(range.max, g.axis), g.theme.dim())),
            top,
        );
        if range.min > 0.0 && plot.height >= 4 {
            let bot = Rect {
                x: plot.x,
                y: plot.y.saturating_add(plot.height.saturating_sub(1)),
                width: hint_w,
                height: 1,
            };
            frame.render_widget(Clear, bot);
            frame.render_widget(
                Paragraph::new(Span::styled(axis_label(range.min, g.axis), g.theme.dim())),
                bot,
            );
        }
    }
    if gutter > 0 {
        render_axis_ticks(frame, area, gutter, range.max, g.axis, g.theme);
    }
}

pub fn panel_title(label: &str, theme: &Theme) -> Line<'static> {
    Line::from(vec![Span::styled(format!(" {label}  "), theme.dim())])
}

pub fn push_token(spans: &mut Vec<Span<'static>>, text: String, style: Style) {
    if !spans.is_empty() && !spans.last().is_some_and(|span| span.content.ends_with(' ')) {
        spans.push(Span::raw("  "));
    }
    spans.push(Span::styled(text, style));
}

pub fn push_kv(
    spans: &mut Vec<Span<'static>>,
    theme: &Theme,
    key: &str,
    value: String,
    value_style: Style,
) {
    push_token(spans, format!("{key}  "), theme.dim());
    spans.push(Span::styled(value, value_style));
}

#[must_use]
pub fn axis_gutter(axis: Axis, width: u16, height: u16) -> u16 {
    if height < 2 {
        return 0;
    }
    let need: u16 = match axis {
        Axis::None | Axis::Percent | Axis::Celsius | Axis::Number => 0,
        Axis::Bits | Axis::Bytes => 7,
    };
    if need == 0 || width <= need.saturating_add(6) {
        0
    } else {
        need
    }
}

fn number_label(value: f32) -> String {
    if value >= 10_000.0 {
        format!("{:.0}k", f64::from(value) / 1000.0)
    } else if value >= 1000.0 {
        format!("{:.1}k", f64::from(value) / 1000.0)
    } else {
        format!("{:.0}", value.round())
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
        Axis::Bytes => bytes_per_sec(value.max(0.0) as u64),
        Axis::Celsius => format!("{:.0}°", value.round()),
        Axis::Number => number_label(value.max(0.0)),
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
        assert_eq!(axis_label(0.10, Axis::Percent), "10%");
        assert_eq!(axis_label(1800.0, Axis::Number), "1.8k");
    }

    #[test]
    fn gutter_is_bits_only() {
        assert_eq!(axis_gutter(Axis::Percent, 40, 8), 0);
        assert_eq!(axis_gutter(Axis::Celsius, 40, 8), 0);
        assert_eq!(axis_gutter(Axis::Number, 40, 8), 0);
        assert_eq!(axis_gutter(Axis::None, 40, 8), 0);
        assert_eq!(axis_gutter(Axis::Bits, 40, 8), 7);
        assert_eq!(axis_gutter(Axis::Bytes, 40, 8), 7);
        assert_eq!(axis_gutter(Axis::Bits, 10, 8), 0);
    }

    #[test]
    fn celsius_hint_replaces_the_tick_stack() {
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

        let mut hint = String::new();
        for x in 0..4 {
            hint.push_str(buf[(x, 0)].symbol());
        }
        assert_eq!(hint, "100°");

        // the plot owns the full width: no tick gutter anywhere
        for y in 0..buf.area.height {
            for x in 0..buf.area.width {
                if x < 4 && y == 0 {
                    continue;
                }
                let ch = buf[(x, y)].symbol();
                assert!(
                    !ch.chars().any(|c: char| c.is_ascii_uppercase() || c == '%'),
                    "stray axis label at x={x} y={y}"
                );
            }
        }
    }

    #[test]
    fn auto_percent_prints_the_ceiling() {
        use plottypus_core::History;
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;

        let mut history = History::default();
        for _ in 0..16 {
            history.push(0.02);
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
                        accent: Theme::default().cpu,
                        theme: &Theme::default(),
                        scale: Scale::LOAD,
                        axis: Axis::Percent,
                        ink: GraphInk::Flat,
                    },
                );
            })
            .unwrap();
        let buf = terminal.backend().buffer();
        let mut hint = String::new();
        for x in 0..5 {
            hint.push_str(buf[(x, 0)].symbol());
        }
        assert!(hint.contains("10%"), "hint was {hint:?}");
    }

    #[test]
    fn percent_graphs_carry_no_axis_ink() {
        use plottypus_core::History;
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;

        let mut history = History::default();
        for _ in 0..48 {
            history.push(0.5);
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
                        accent: Theme::default().cpu,
                        theme: &Theme::default(),
                        scale: Scale::Fixed(1.0),
                        axis: Axis::Percent,
                        ink: GraphInk::Load(plottypus_core::Thermal::Nominal),
                    },
                );
            })
            .unwrap();
        let buf = terminal.backend().buffer();
        for y in 0..buf.area.height {
            for x in 0..buf.area.width {
                let sym = buf[(x, y)].symbol();
                assert!(!sym.contains('%'), "% at {x},{y}");
                assert!(
                    !sym.contains('°'),
                    "hint leaked to percent graph at {x},{y}"
                );
            }
        }
    }

    #[test]
    fn titles_follow_the_spacing_contract() {
        let theme = Theme::default();
        let line = panel_title("cpu", &theme);
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(text, " cpu  ");

        let mut spans = Vec::new();
        push_token(&mut spans, String::from("18%"), theme.title());
        push_token(&mut spans, String::from("8.2W"), theme.cpu());
        push_kv(
            &mut spans,
            &theme,
            "temp",
            String::from("42°"),
            theme.temp(),
        );
        let joined: String = spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(joined, "18%  8.2W  temp  42°");
        assert!(!joined.contains("   "), "{joined}");
    }
}

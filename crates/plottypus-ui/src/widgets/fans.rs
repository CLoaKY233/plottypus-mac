use plottypus_core::{FanMetric, History};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::chrome::{
    Axis, Graph, GraphInk, panel_block, panel_title, push_token, render_scaled_graph,
};

use crate::layout::{Degrade, Panel};
use crate::theme::Theme;
use crate::widgets::AppView;
use plottypus_core::Scale;

pub fn render(frame: &mut Frame, area: Rect, view: &AppView<'_>, theme: &Theme) {
    let block = panel_block(
        Panel::Fans,
        title(view, theme),
        view.is_focused(Panel::Fans),
        view.is_expanded(Panel::Fans),
        theme,
    );
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.width == 0 || inner.height == 0 {
        return;
    }
    render_compact(frame, inner, view, theme);
}

fn render_compact(frame: &mut Frame, area: Rect, view: &AppView<'_>, theme: &Theme) {
    let named = named_temps(view);
    let fans = present_fans(view);
    if named.is_empty() && fans.is_empty() {
        return;
    }
    let graph = compact_temp_history(view);
    if area.height < 2 || view.degrade != Degrade::Full || graph.is_none() {
        render_headline(frame, area, &named, fans, theme);
        return;
    }
    let rows = Layout::vertical([Constraint::Length(1), Constraint::Fill(1)]).split(area);
    render_headline(frame, rows[0], &named, fans, theme);
    if let Some(history) = graph {
        render_scaled_graph(
            frame,
            rows[1],
            Graph {
                history,
                accent: theme.temp,
                theme,
                scale: Scale::Fixed(100.0),
                axis: Axis::None,
                ink: GraphInk::Load(view.snapshot.thermal),
            },
        );
    }
}

fn title(view: &AppView<'_>, theme: &Theme) -> Line<'static> {
    let mut spans = panel_title("sens", theme);
    let temp = title_temp(view);
    let fans = present_fans(view);
    if temp.is_none() && fans.is_empty() {
        spans.spans.push(Span::styled("—".to_owned(), theme.dim()));
        return spans;
    }
    if let Some(c) = temp {
        push_token(&mut spans.spans, format!("{c:.0}°"), theme.temp());
    }
    if !fans.is_empty() {
        spans.spans.extend(fan_speed_spans(fans, theme));
    }
    spans
}

fn present_fans<'a>(view: &'a AppView<'_>) -> &'a [FanMetric] {
    if view.snapshot.fans.is_present() {
        &view.snapshot.fans.fans
    } else {
        &[]
    }
}

fn fan_speed_spans(fans: &[FanMetric], theme: &Theme) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    for (i, fan) in fans.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled("  ", theme.dim()));
        }
        spans.push(Span::styled(format!("{} rpm", fan.rpm), theme.title()));
    }
    spans
}

fn fan_speeds_width(fans: &[FanMetric]) -> u16 {
    if fans.is_empty() {
        return 0;
    }
    let text = fans
        .iter()
        .map(|f| format!("{} rpm", f.rpm))
        .collect::<Vec<_>>()
        .join("  ");
    u16::try_from(text.chars().count().saturating_add(1)).unwrap_or(0)
}

fn render_headline(
    frame: &mut Frame,
    area: Rect,
    named: &[(String, f32)],
    fans: &[FanMetric],
    theme: &Theme,
) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let fan_w = fan_speeds_width(fans);
    if !fans.is_empty() && area.width > fan_w.saturating_add(8) {
        let cols = Layout::horizontal([Constraint::Fill(1), Constraint::Length(fan_w)]).split(area);
        if !named.is_empty() {
            render_temp_line(frame, cols[0], named, theme);
        }
        frame.render_widget(
            Paragraph::new(Line::from(fan_speed_spans(fans, theme)).right_aligned()),
            cols[1],
        );
        return;
    }
    if named.is_empty() {
        frame.render_widget(
            Paragraph::new(Line::from(fan_speed_spans(fans, theme))),
            area,
        );
    } else {
        render_temp_line(frame, area, named, theme);
    }
}

fn compact_temp_history<'a>(view: &'a AppView<'_>) -> Option<&'a History> {
    if !view.cpu_temp_history.is_empty() {
        Some(view.cpu_temp_history)
    } else if !view.gpu_temp_history.is_empty() {
        Some(view.gpu_temp_history)
    } else {
        None
    }
}

fn title_temp(view: &AppView<'_>) -> Option<f32> {
    view.snapshot
        .sensors
        .cpu_c
        .or(view.snapshot.cpu.temp_c)
        .or(view.snapshot.sensors.hotspot_c)
        .or(view.snapshot.sensors.gpu_c)
        .or_else(|| view.snapshot.sensors.readings.first().map(|r| r.celsius))
}

fn named_temps(view: &AppView<'_>) -> Vec<(String, f32)> {
    let sensors = &view.snapshot.sensors;
    let mut out = Vec::new();
    if let Some(c) = sensors.e_c {
        out.push((String::from("e"), c));
    }
    if let Some(c) = sensors.p_c {
        out.push((String::from("p"), c));
    }
    if let Some(c) = sensors.s_c {
        out.push((String::from("s"), c));
    }
    if out.is_empty()
        && let Some(c) = sensors.cpu_c.or(view.snapshot.cpu.temp_c)
    {
        out.push((String::from("cpu"), c));
    }
    if let Some(c) = sensors.gpu_c {
        out.push((String::from("gpu"), c));
    }
    if let Some(c) = sensors.hotspot_c {
        out.push((String::from("hot"), c));
    }
    if out.is_empty()
        && let Some(r) = sensors.readings.first()
    {
        out.push((r.name.clone(), r.celsius));
    }
    out
}

fn render_temp_line(frame: &mut Frame, area: Rect, temps: &[(String, f32)], theme: &Theme) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let mut spans = Vec::new();
    for (i, (name, c)) in temps.iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw("  "));
        }
        spans.push(Span::styled(
            format!("{name} {c:.0}°"),
            Style::default().fg(theme.temp_color(*c)),
        ));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::widgets::tests_support::fixture;
    use plottypus_core::{FanSnapshot, GpuSnapshot, TempReading};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn line_text(line: &Line<'_>) -> String {
        line.spans.iter().map(|s| s.content.as_ref()).collect()
    }

    fn paint(view: &AppView<'_>, width: u16, height: u16) -> String {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| render(frame, frame.area(), view, &Theme::default()))
            .unwrap();
        let buf = terminal.backend().buffer();
        let mut out = String::new();
        for y in 0..buf.area.height {
            for x in 0..buf.area.width {
                out.push_str(buf[(x, y)].symbol());
            }
            out.push('\n');
        }
        out
    }

    fn thermal_fixture() -> crate::widgets::tests_support::Fixture {
        let mut fx = fixture("");
        fx.snap.cpu.scaled = 0.18;
        fx.snap.sensors.cpu_c = Some(42.0);
        fx.snap.sensors.gpu_c = Some(51.0);
        fx.snap.sensors.hotspot_c = Some(60.0);
        fx.snap.sensors.readings = vec![
            TempReading {
                name: String::from("cpu"),
                celsius: 42.0,
            },
            TempReading {
                name: String::from("nand"),
                celsius: 38.0,
            },
        ];
        fx.snap.gpu = Some(GpuSnapshot {
            scaled: 0.12,
            temp_c: Some(51.0),
            ..GpuSnapshot::default()
        });
        fx.snap.fans = FanSnapshot {
            fans: vec![
                FanMetric {
                    name: String::from("Fan 1"),
                    rpm: 1200,
                    max_rpm: 6000,
                },
                FanMetric {
                    name: String::from("Fan 2"),
                    rpm: 1850,
                    max_rpm: 6000,
                },
            ],
        };
        fx
    }

    #[test]
    fn title_lists_both_fans() {
        let mut fx = fixture("");
        fx.snap.fans = FanSnapshot {
            fans: vec![
                FanMetric {
                    name: String::from("Fan 1"),
                    rpm: 1200,
                    max_rpm: 6000,
                },
                FanMetric {
                    name: String::from("Fan 2"),
                    rpm: 1850,
                    max_rpm: 6000,
                },
            ],
        };
        let text = line_text(&title(&fx.view(), &Theme::default()));
        assert!(text.contains("sens"), "{text}");
        assert!(text.contains("1200 rpm"), "{text}");
        assert!(text.contains("1850 rpm"), "{text}");
    }

    #[test]
    fn title_dash_when_absent() {
        let fx = fixture("");
        let text = line_text(&title(&fx.view(), &Theme::default()));
        assert!(text.contains("sens"), "{text}");
        assert!(text.contains('—'), "{text}");
    }

    #[test]
    fn title_temp_and_both_fans() {
        let mut fx = fixture("");
        fx.snap.sensors.cpu_c = Some(52.0);
        fx.snap.sensors.hotspot_c = Some(52.0);
        fx.snap.fans = FanSnapshot {
            fans: vec![
                FanMetric {
                    name: String::from("Fan 1"),
                    rpm: 1200,
                    max_rpm: 6000,
                },
                FanMetric {
                    name: String::from("Fan 2"),
                    rpm: 1850,
                    max_rpm: 6000,
                },
            ],
        };
        let text = line_text(&title(&fx.view(), &Theme::default()));
        assert!(text.contains("52°"), "{text}");
        assert!(text.contains("1200"), "{text}");
        assert!(text.contains("1850"), "{text}");
    }

    #[test]
    fn title_shows_temp_without_fans() {
        let mut fx = fixture("");
        fx.snap.sensors.cpu_c = Some(42.0);
        let text = line_text(&title(&fx.view(), &Theme::default()));
        assert!(text.contains("sens"), "{text}");
        assert!(text.contains("42°"), "{text}");
        assert!(!text.contains("rpm"), "{text}");
        assert!(!text.contains('—'), "{text}");
    }

    #[test]
    fn compact_is_headline_and_graph() {
        let fx = thermal_fixture();
        let text = paint(&fx.view(), 48, 8);
        assert!(text.contains("cpu"), "{text}");
        assert!(text.contains("42°"), "{text}");
        assert!(text.contains("gpu"), "{text}");
        assert!(text.contains("51°"), "{text}");
        assert!(text.contains("1200 rpm"), "{text}");
        assert!(text.contains("1850 rpm"), "{text}");
        assert!(!text.contains("nand"), "{text}");
        assert!(!text.contains("related"), "{text}");
    }
}

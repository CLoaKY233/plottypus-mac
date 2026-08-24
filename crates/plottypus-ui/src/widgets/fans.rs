use plottypus_core::FanMetric;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::chrome::{Axis, panel_block, render_fill_bar, render_scaled_graph};
use crate::layout::Panel;
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
    if view.is_expanded(Panel::Fans) {
        render_expanded(frame, inner, view, theme);
        return;
    }
    render_compact(frame, inner, view, theme);
}

fn render_compact(frame: &mut Frame, area: Rect, view: &AppView<'_>, theme: &Theme) {
    let named = named_temps(view);
    let fans = present_fans(view);
    if named.is_empty() && fans.is_empty() {
        render_scaled_graph(
            frame,
            area,
            view.cpu_temp_history,
            theme.temp,
            theme,
            Scale::Fixed(100.0),
            Axis::None,
        );
        return;
    }
    if area.height < 2 {
        render_headline(frame, area, &named, fans, theme);
        return;
    }
    let rows = Layout::vertical([Constraint::Length(1), Constraint::Fill(1)]).split(area);
    render_headline(frame, rows[0], &named, fans, theme);
    render_scaled_graph(
        frame,
        rows[1],
        view.cpu_temp_history,
        theme.temp,
        theme,
        Scale::Fixed(100.0),
        Axis::None,
    );
}

fn title(view: &AppView<'_>, theme: &Theme) -> Line<'static> {
    let mut spans = vec![Span::styled(" sens  ", theme.dim())];
    let temp = title_temp(view);
    let fans = present_fans(view);
    if temp.is_none() && fans.is_empty() {
        spans.push(Span::styled("— ", theme.dim()));
        return Line::from(spans);
    }
    if let Some(c) = temp {
        spans.push(Span::styled(format!("{c:.0}°"), theme.temp()));
    }
    if !fans.is_empty() {
        if temp.is_some() {
            spans.push(Span::raw("  "));
        }
        spans.extend(fan_speed_spans(fans, theme));
    }
    spans.push(Span::raw(" "));
    Line::from(spans)
}

fn render_expanded(frame: &mut Frame, area: Rect, view: &AppView<'_>, theme: &Theme) {
    let extras = extra_temps(view);
    let fans = present_fans(view);
    let show_gpu = view.snapshot.sensors.gpu_c.is_some()
        || view.snapshot.gpu.and_then(|g| g.temp_c).is_some()
        || !view.gpu_temp_history.is_empty();

    let label_h = 2u16.saturating_add(u16::from(show_gpu));
    let min_graphs = 2u16.saturating_add(if show_gpu { 2 } else { 0 });
    let fan_h = reserved_fan_height(fans.len(), area.height);
    let leftover = area.height.saturating_sub(
        label_h
            .saturating_add(min_graphs)
            .saturating_add(fan_h),
    );
    let extra_h = leftover.min(u16::try_from(extras.len()).unwrap_or(0));

    let mut constraints = vec![
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Fill(2),
    ];
    if show_gpu {
        constraints.push(Constraint::Length(1));
        constraints.push(Constraint::Fill(2));
    }
    if fan_h > 0 {
        constraints.push(Constraint::Length(fan_h));
    }
    if extra_h > 0 {
        constraints.push(Constraint::Length(extra_h));
    }

    let parts = Layout::vertical(constraints).split(area);
    let mut i = 0;
    frame.render_widget(Paragraph::new(related_line(view, theme)), parts[i]);
    i += 1;
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(" cpu temp", theme.dim()))),
        parts[i],
    );
    i += 1;
    render_scaled_graph(
        frame,
        parts[i],
        view.cpu_temp_history,
        theme.temp,
        theme,
        Scale::Fixed(100.0),
        Axis::Celsius,
    );
    i += 1;
    if show_gpu {
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(" gpu temp", theme.dim()))),
            parts[i],
        );
        i += 1;
        render_scaled_graph(
            frame,
            parts[i],
            view.gpu_temp_history,
            theme.gpu,
            theme,
            Scale::Fixed(100.0),
            Axis::Celsius,
        );
        i += 1;
    }
    if fan_h > 0 {
        render_fans(frame, parts[i], fans, theme);
    }
    if extra_h > 0 {
        let extra_i = i + usize::from(fan_h > 0);
        render_temp_list(
            frame,
            parts[extra_i],
            &extras[..usize::from(extra_h)],
            theme,
        );
    }
}

fn related_line(view: &AppView<'_>, theme: &Theme) -> Line<'static> {
    let mut spans = vec![
        Span::styled(" related  ", theme.dim()),
        Span::styled("cpu ", theme.dim()),
        Span::styled(
            format!("{:.0}%", view.snapshot.cpu.active.clamp(0.0, 1.0) * 100.0),
            theme.cpu(),
        ),
    ];
    if let Some(gpu) = view.snapshot.gpu {
        spans.push(Span::styled("   gpu ", theme.dim()));
        spans.push(Span::styled(
            format!("{:.0}%", gpu.scaled.clamp(0.0, 1.0) * 100.0),
            theme.gpu(),
        ));
    }
    Line::from(spans)
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
        spans.push(Span::styled(fan.rpm.to_string(), theme.title()));
    }
    spans
}

fn fan_speeds_width(fans: &[FanMetric]) -> u16 {
    if fans.is_empty() {
        return 0;
    }
    let text: String = fans
        .iter()
        .map(|f| f.rpm.to_string())
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
        frame.render_widget(Paragraph::new(Line::from(fan_speed_spans(fans, theme))), area);
    } else {
        render_temp_line(frame, area, named, theme);
    }
}

fn reserved_fan_height(n: usize, total: u16) -> u16 {
    if n == 0 || total == 0 {
        return 0;
    }
    let n = u16::try_from(n).unwrap_or(1);
    if total >= n.saturating_mul(2).saturating_add(10) {
        n.saturating_mul(2)
    } else {
        n.min(total)
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

fn extra_temps(view: &AppView<'_>) -> Vec<(String, f32)> {
    view.snapshot
        .sensors
        .readings
        .iter()
        .filter(|r| !is_headline_temp(&r.name))
        .map(|r| (r.name.clone(), r.celsius))
        .collect()
}

fn is_headline_temp(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n == "cpu"
        || n == "gpu"
        || n == "hot"
        || n == "efficiency"
        || n == "performance"
        || n == "super"
        || n.contains("hotspot")
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

fn render_temp_list(frame: &mut Frame, area: Rect, temps: &[(String, f32)], theme: &Theme) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let take = usize::from(area.height).min(temps.len());
    let lines: Vec<Line> = temps
        .iter()
        .take(take)
        .map(|(name, c)| {
            Line::from(Span::styled(
                format!("{name} {c:.0}°"),
                Style::default().fg(theme.temp_color(*c)),
            ))
        })
        .collect();
    frame.render_widget(Paragraph::new(lines), area);
}

fn render_fans(frame: &mut Frame, area: Rect, fans: &[FanMetric], theme: &Theme) {
    if area.width == 0 || area.height == 0 || fans.is_empty() {
        return;
    }
    let n = fans.len().max(1);
    let row_h = (area.height / n as u16).max(1);
    let constraints: Vec<Constraint> = fans
        .iter()
        .map(|_| Constraint::Length(row_h.clamp(1, 2)))
        .collect();
    let rows = Layout::vertical(constraints).split(area);
    for (fan, row) in fans.iter().zip(rows.iter().copied()) {
        if row.height >= 2 {
            let parts = Layout::vertical([Constraint::Length(1), Constraint::Length(1)]).split(row);
            frame.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::styled(format!(" {}  ", fan.name), theme.dim()),
                    Span::styled(format!("{} rpm", fan.rpm), theme.title()),
                ])),
                parts[0],
            );
            render_fill_bar(frame, parts[1], fan.ratio(), theme.fan);
        } else {
            let bar_w = row.width.saturating_sub(18);
            let cols = Layout::horizontal([Constraint::Fill(1), Constraint::Length(bar_w.max(4))])
                .split(row);
            frame.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::styled(format!(" {} ", fan.name), theme.dim()),
                    Span::styled(format!("{}", fan.rpm), theme.title()),
                ])),
                cols[0],
            );
            if cols.len() > 1 {
                render_fill_bar(frame, cols[1], fan.ratio(), theme.fan);
            }
        }
    }
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
        fx.snap.cpu.active = 0.18;
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
        assert!(text.contains("1200"), "{text}");
        assert!(text.contains("1850"), "{text}");
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
    fn named_temps_are_headlines() {
        let fx = thermal_fixture();
        let named = named_temps(&fx.view());
        let extras = extra_temps(&fx.view());
        assert!(
            named
                .iter()
                .any(|(n, c)| n == "cpu" && (*c - 42.0).abs() < f32::EPSILON)
        );
        assert!(named.iter().any(|(n, _)| n == "gpu"));
        assert!(named.iter().any(|(n, _)| n == "hot"));
        assert!(extras.iter().any(|(n, _)| n == "nand"));
        assert!(!extras.iter().any(|(n, _)| n == "cpu"));
    }

    #[test]
    fn compact_is_headline_and_graph() {
        let fx = thermal_fixture();
        let text = paint(&fx.view(), 48, 8);
        assert!(text.contains("cpu"), "{text}");
        assert!(text.contains("42°"), "{text}");
        assert!(text.contains("gpu"), "{text}");
        assert!(text.contains("51°"), "{text}");
        assert!(text.contains("1200"), "{text}");
        assert!(text.contains("1850"), "{text}");
        assert!(!text.contains("nand"), "{text}");
        assert!(!text.contains("related"), "{text}");
    }

    #[test]
    fn expanded_fills_with_graphs_related_fans_and_extras() {
        let mut fx = thermal_fixture();
        fx.expanded = Some(Panel::Fans);
        let text = paint(&fx.view(), 48, 24);
        assert!(text.contains("related"), "{text}");
        assert!(text.contains("18%"), "{text}");
        assert!(text.contains("12%"), "{text}");
        assert!(text.contains("cpu temp"), "{text}");
        assert!(text.contains("gpu temp"), "{text}");
        assert!(text.contains("Fan 1"), "{text}");
        assert!(text.contains("Fan 2"), "{text}");
        assert!(text.contains("1200"), "{text}");
        assert!(text.contains("1850"), "{text}");
        assert!(text.contains("nand"), "{text}");
        assert!(text.contains("38°"), "{text}");
    }

    #[test]
    fn expanded_skips_empty_fan_and_gpu_slots() {
        let mut fx = fixture("");
        fx.expanded = Some(Panel::Fans);
        fx.snap.sensors.cpu_c = Some(42.0);
        let text = paint(&fx.view(), 48, 20);
        assert!(text.contains("related"), "{text}");
        assert!(text.contains("cpu temp"), "{text}");
        assert!(!text.contains("gpu temp"), "{text}");
        assert!(!text.contains("rpm"), "{text}");
        assert!(!text.contains("nand"), "{text}");
    }

    #[test]
    fn fans_keep_a_row_when_the_box_is_short() {
        assert_eq!(reserved_fan_height(0, 8), 0);
        assert_eq!(reserved_fan_height(2, 24), 4);
        assert_eq!(reserved_fan_height(2, 12), 2);
        assert_eq!(reserved_fan_height(1, 0), 0);
    }
}

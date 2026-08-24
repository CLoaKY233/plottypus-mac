use plottypus_core::FanMetric;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::chrome::{panel_block, render_fill_bar};
use crate::layout::Panel;
use crate::theme::Theme;
use crate::widgets::AppView;

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

    let named = named_temps(view);
    let extras = extra_temps(view);
    let fans = &view.snapshot.fans.fans;

    let named_h = u16::from(!named.is_empty());
    let after_named = inner.height.saturating_sub(named_h);
    let fan_n = u16::try_from(fans.len()).unwrap_or(0);
    let fan_h = if fan_n == 0 {
        0
    } else if after_named >= fan_n.saturating_mul(2) {
        fan_n.saturating_mul(2)
    } else {
        after_named.min(fan_n)
    };
    let extra_h = after_named
        .saturating_sub(fan_h)
        .min(u16::try_from(extras.len()).unwrap_or(0));

    let mut constraints = Vec::new();
    if named_h > 0 {
        constraints.push(Constraint::Length(1));
    }
    if extra_h > 0 {
        constraints.push(Constraint::Length(extra_h));
    }
    if fan_h > 0 {
        constraints.push(Constraint::Length(fan_h));
    }
    if constraints.is_empty() {
        return;
    }

    let parts = Layout::vertical(constraints).split(inner);
    let extra_i = usize::from(named_h > 0);
    let fan_i = extra_i + usize::from(extra_h > 0);
    if named_h > 0 {
        render_temp_line(frame, parts[0], &named, theme);
    }
    if extra_h > 0 {
        render_temp_list(
            frame,
            parts[extra_i],
            &extras[..usize::from(extra_h)],
            theme,
        );
    }
    if fan_h > 0 {
        render_fans(frame, parts[fan_i], fans, theme);
    }
}

fn title(view: &AppView<'_>, theme: &Theme) -> Line<'static> {
    let mut spans = vec![Span::styled(" sens  ", theme.dim())];
    let temp = title_temp(view);
    let rpm = view.snapshot.fans.is_present().then(|| {
        view.snapshot
            .fans
            .fans
            .iter()
            .map(|f| f.rpm)
            .max()
            .unwrap_or(0)
    });
    match (temp, rpm) {
        (None, None) => spans.push(Span::styled("— ", theme.dim())),
        (Some(c), None) => {
            spans.push(Span::styled(format!("{c:.0}° "), theme.temp()));
        }
        (None, Some(rpm)) => {
            spans.push(Span::styled(format!("{rpm} rpm "), theme.title()));
        }
        (Some(c), Some(rpm)) => {
            spans.push(Span::styled(format!("{c:.0}°"), theme.temp()));
            spans.push(Span::raw("  "));
            spans.push(Span::styled(format!("{rpm} rpm "), theme.title()));
        }
    }
    Line::from(spans)
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
    if let Some(c) = sensors.cpu_c.or(view.snapshot.cpu.temp_c) {
        out.push((String::from("cpu"), c));
    }
    if let Some(c) = sensors.gpu_c {
        out.push((String::from("gpu"), c));
    }
    if let Some(c) = sensors.hotspot_c {
        out.push((String::from("hot"), c));
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
    n == "cpu" || n == "gpu" || n == "hot" || n.contains("hotspot")
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
mod tests {
    use super::*;
    use crate::widgets::tests_support::fixture;
    use plottypus_core::FanSnapshot;

    fn line_text(line: &Line<'_>) -> String {
        line.spans.iter().map(|s| s.content.as_ref()).collect()
    }

    #[test]
    fn title_peak_rpm() {
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
    fn title_shows_temp_without_fans() {
        let mut fx = fixture("");
        fx.snap.sensors.cpu_c = Some(42.0);
        let text = line_text(&title(&fx.view(), &Theme::default()));
        assert!(text.contains("sens"), "{text}");
        assert!(text.contains("42°"), "{text}");
        assert!(!text.contains("rpm"), "{text}");
        assert!(!text.contains('—'), "{text}");
    }
}

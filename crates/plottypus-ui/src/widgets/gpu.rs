use plottypus_core::{Scale, percent_display, watts_display};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::chrome::{Axis, panel_block, render_scaled_graph};
use crate::layout::Panel;
use crate::theme::Theme;
use crate::widgets::AppView;

pub fn render(frame: &mut Frame, area: Rect, view: &AppView<'_>, theme: &Theme) {
    let block = panel_block(
        Panel::Gpu,
        title(view, theme),
        view.is_focused(Panel::Gpu),
        view.is_expanded(Panel::Gpu),
        theme,
    );
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.width == 0 || inner.height == 0 {
        return;
    }
    if view.is_expanded(Panel::Gpu) {
        render_expanded(frame, inner, view, theme);
        return;
    }
    let spec = spec_line(view, theme);
    let (plot, spec_row) = if inner.height >= 4 && !spec.spans.is_empty() {
        let rows = Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).split(inner);
        (rows[0], Some(rows[1]))
    } else {
        (inner, None)
    };
    render_scaled_graph(
        frame,
        plot,
        view.gpu_history,
        theme.gpu,
        theme,
        Scale::Fixed(1.0),
        Axis::Percent,
    );
    if let Some(row) = spec_row {
        frame.render_widget(Paragraph::new(spec), row);
    }
}

fn render_expanded(frame: &mut Frame, area: Rect, view: &AppView<'_>, theme: &Theme) {
    let rows = Layout::vertical([
        Constraint::Length(2),
        Constraint::Fill(2),
        Constraint::Fill(1),
        Constraint::Fill(2),
    ])
    .split(area);
    frame.render_widget(Paragraph::new(vec![spec_line(view, theme), load_line(view, theme)]), rows[0]);
    render_scaled_graph(
        frame,
        rows[1],
        view.gpu_history,
        theme.gpu,
        theme,
        Scale::Fixed(1.0),
        Axis::Percent,
    );
    if rows.len() > 2 {
        render_scaled_graph(
            frame,
            rows[2],
            view.gpu_temp_history,
            theme.temp,
            theme,
            Scale::Fixed(100.0),
            Axis::Celsius,
        );
    }
    if rows.len() > 3 {
        render_related_procs(frame, rows[3], view, theme);
    }
}

fn load_line(view: &AppView<'_>, theme: &Theme) -> Line<'static> {
    let gpu = view.snapshot.gpu.map_or(0.0, |g| g.scaled);
    Line::from(vec![
        Span::styled(" util  ", theme.dim()),
        Span::styled(ready_pct(view.ready, gpu), theme.title()),
        Span::styled("   cpu  ", theme.dim()),
        Span::styled(ready_pct(view.ready, view.snapshot.cpu.active), theme.cpu()),
    ])
}

fn spec_line(view: &AppView<'_>, theme: &Theme) -> Line<'static> {
    let items = spec_items(view);
    if items.is_empty() {
        return Line::from("");
    }
    Line::from(Span::styled(format!(" {}", items.join("  ")), theme.dim()))
}

fn render_related_procs(frame: &mut Frame, area: Rect, view: &AppView<'_>, theme: &Theme) {
    let mut procs = view.snapshot.processes.clone();
    procs.sort_by(|a, b| b.cpu.total_cmp(&a.cpu));
    let take = usize::from(area.height.saturating_sub(1)).min(procs.len()).min(8);
    let mut lines = vec![Line::from(Span::styled(
        " busiest (cpu — per-process gpu % is not exposed without IOReport)",
        theme.dim(),
    ))];
    for proc in procs.into_iter().take(take) {
        lines.push(Line::from(vec![
            Span::styled(format!(" {:>6}  ", proc.pid), theme.dim()),
            Span::styled(format!("{:<16}", {
                let mut n: String = proc.name.chars().take(16).collect();
                if proc.name.chars().count() > 16 {
                    n.pop();
                    n.push('…');
                }
                n
            }), theme.fg()),
            Span::styled(format!(" {:>5.1}", proc.cpu), theme.cpu()),
        ]));
    }
    frame.render_widget(Paragraph::new(lines), area);
}

fn title(view: &AppView<'_>, theme: &Theme) -> Line<'static> {
    let mut spans = vec![Span::styled(" gpu  ", theme.dim())];
    match view.snapshot.gpu {
        None => spans.push(Span::styled("—", theme.dim())),
        Some(gpu) => {
            spans.push(Span::styled(
                ready_pct(view.ready, gpu.scaled),
                theme.title(),
            ));
            if view.ready
                && let Some(watts) = gpu.watts
            {
                spans.push(Span::raw("  "));
                spans.push(Span::styled(watts_display(watts), theme.gpu()));
            }
        }
    }
    if let Some(temp) = view
        .snapshot
        .gpu
        .and_then(|g| g.temp_c)
        .or(view.snapshot.sensors.gpu_c)
    {
        spans.push(Span::raw("  "));
        spans.push(Span::styled(format!("{temp:.0}°"), theme.temp()));
    }
    spans.push(Span::raw(" "));
    Line::from(spans)
}

fn spec_items(view: &AppView<'_>) -> Vec<String> {
    let Some(gpu) = view.snapshot.gpu else {
        return Vec::new();
    };
    let mut items = Vec::new();
    if let Some(mhz) = gpu.freq_mhz.filter(|mhz| *mhz > 0) {
        items.push(freq_label(mhz));
    }
    if let Some(watts) = gpu.ane_watts.filter(|w| *w > 0.0) {
        items.push(format!("ane {}", watts_display(watts)));
    }
    if view.snapshot.soc.gpu_cores > 0 {
        items.push(format!("{}c", view.snapshot.soc.gpu_cores));
    }
    items
}

fn ready_pct(ready: bool, ratio: f32) -> String {
    if ready {
        percent_display(ratio)
    } else {
        String::from("…")
    }
}

fn freq_label(mhz: u32) -> String {
    if mhz >= 1000 {
        format!("{:.1}GHz", f64::from(mhz) / 1000.0)
    } else {
        format!("{mhz}MHz")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widgets::tests_support::fixture;
    use plottypus_core::GpuSnapshot;

    fn line_text(line: &Line<'_>) -> String {
        line.spans.iter().map(|s| s.content.as_ref()).collect()
    }

    #[test]
    fn title_dash_when_missing() {
        let fx = fixture("");
        let text = line_text(&title(&fx.view(), &Theme::default()));
        assert!(text.contains("gpu"));
        assert!(text.contains('—'));
    }

    #[test]
    fn title_percent_and_watts() {
        let mut fx = fixture("");
        fx.ready = true;
        fx.snap.gpu = Some(GpuSnapshot {
            scaled: 0.12,
            watts: Some(1.1),
            freq_mhz: Some(461),
            ane_watts: Some(0.4),
            ..GpuSnapshot::default()
        });
        let theme = Theme::default();
        let text = line_text(&title(&fx.view(), &theme));
        assert!(text.contains("12%"));
        assert!(text.contains("1.1W"));
        let specs = spec_items(&fx.view());
        assert_eq!(specs, ["461MHz", "ane 0.4W", "16c"]);
    }

    #[test]
    fn title_includes_gpu_temp() {
        let mut fx = fixture("");
        fx.ready = true;
        fx.snap.gpu = Some(GpuSnapshot {
            scaled: 0.12,
            ..GpuSnapshot::default()
        });
        fx.snap.sensors.gpu_c = Some(38.0);
        let text = line_text(&title(&fx.view(), &Theme::default()));
        assert!(text.contains("38°"), "{text}");
        fx.snap.sensors.gpu_c = None;
        fx.snap.gpu = Some(GpuSnapshot {
            scaled: 0.12,
            temp_c: Some(51.0),
            ..GpuSnapshot::default()
        });
        let text = line_text(&title(&fx.view(), &Theme::default()));
        assert!(text.contains("51°"), "{text}");
    }

    #[test]
    fn title_ellipsis_until_ready() {
        let mut fx = fixture("");
        fx.ready = false;
        fx.snap.gpu = Some(GpuSnapshot {
            scaled: 0.4,
            watts: Some(2.0),
            ..GpuSnapshot::default()
        });
        let text = line_text(&title(&fx.view(), &Theme::default()));
        assert!(text.contains('…'));
        assert!(!text.contains('%'));
    }

    #[test]
    fn specs_stay_sparse() {
        let mut fx = fixture("");
        fx.snap.gpu = Some(GpuSnapshot {
            freq_mhz: Some(0),
            ane_watts: Some(0.0),
            ..GpuSnapshot::default()
        });
        fx.snap.soc.gpu_cores = 0;
        assert!(spec_items(&fx.view()).is_empty());
    }
}

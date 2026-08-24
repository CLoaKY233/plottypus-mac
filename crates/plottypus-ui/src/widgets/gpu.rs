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
    let (plot, spec_row) = if inner.height >= 4 && line_has_text(&spec) {
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
    let mut header = Vec::new();
    let spec = spec_line(view, theme);
    if line_has_text(&spec) {
        header.push(spec);
    }
    header.push(load_line(view, theme));
    let header_h = u16::try_from(header.len()).unwrap_or(1).min(area.height);
    let remain = area.height.saturating_sub(header_h);
    let rows = if remain >= 6 {
        Layout::vertical([
            Constraint::Length(header_h),
            Constraint::Fill(1),
            Constraint::Fill(1),
        ])
        .split(area)
    } else {
        Layout::vertical([Constraint::Length(header_h), Constraint::Fill(1)]).split(area)
    };
    frame.render_widget(Paragraph::new(header), rows[0]);
    render_gpu_graphs(frame, rows[1], view, theme);
    if let Some(procs) = rows.get(2) {
        render_related_procs(frame, *procs, view, theme);
    }
}

fn render_gpu_graphs(frame: &mut Frame, area: Rect, view: &AppView<'_>, theme: &Theme) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let (util, temp) = if area.width >= 36 {
        let cols = Layout::horizontal([Constraint::Fill(1), Constraint::Fill(1)]).split(area);
        (cols[0], Some(cols[1]))
    } else if area.height >= 4 {
        let rows = Layout::vertical([Constraint::Fill(1), Constraint::Fill(1)]).split(area);
        (rows[0], rows.get(1).copied())
    } else {
        (area, None)
    };
    render_scaled_graph(
        frame,
        util,
        view.gpu_history,
        theme.gpu,
        theme,
        Scale::Fixed(1.0),
        Axis::Percent,
    );
    if let Some(temp) = temp {
        render_scaled_graph(
            frame,
            temp,
            view.gpu_temp_history,
            theme.temp,
            theme,
            Scale::Fixed(100.0),
            Axis::Celsius,
        );
    }
}

fn load_line(view: &AppView<'_>, theme: &Theme) -> Line<'static> {
    let gpu = view.snapshot.gpu.map_or(0.0, |g| g.scaled);
    Line::from(vec![
        Span::styled(" util  ", theme.dim()),
        Span::styled(ready_pct(view.ready, gpu), theme.title()),
        Span::styled("   cpu  ", theme.dim()),
        Span::styled(ready_pct(view.ready, view.snapshot.cpu.scaled), theme.cpu()),
    ])
}

fn spec_line(view: &AppView<'_>, theme: &Theme) -> Line<'static> {
    let items = spec_items(view);
    if items.is_empty() {
        return Line::default();
    }
    Line::from(Span::styled(format!(" {}", items.join("  ")), theme.dim()))
}

fn render_related_procs(frame: &mut Frame, area: Rect, view: &AppView<'_>, theme: &Theme) {
    if area.height == 0 {
        return;
    }
    frame.render_widget(
        Paragraph::new(related_proc_lines(view, area.height, theme)),
        area,
    );
}

fn related_proc_lines(view: &AppView<'_>, rows: u16, theme: &Theme) -> Vec<Line<'static>> {
    let mut procs: Vec<_> = view.snapshot.processes.iter().collect();
    procs.sort_by(|a, b| b.cpu.total_cmp(&a.cpu));
    let take = usize::from(rows.saturating_sub(1)).min(procs.len());
    let mut lines = vec![Line::from(Span::styled(
        " related  cpu · no per-process gpu %",
        theme.dim(),
    ))];
    for proc in procs.into_iter().take(take) {
        lines.push(Line::from(vec![
            Span::styled(format!(" {:>6}  ", proc.pid), theme.dim()),
            Span::styled(format!("{:<16}", truncate(&proc.name, 16)), theme.fg()),
            Span::styled(format!(" {:>5.1}", proc.cpu), theme.cpu()),
        ]));
    }
    lines
}

fn truncate(name: &str, width: usize) -> String {
    let mut out: String = name.chars().take(width).collect();
    if name.chars().count() > width {
        out.pop();
        out.push('…');
    }
    out
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
    if let Some(temp) = gpu_temp_c(view) {
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

fn gpu_temp_c(view: &AppView<'_>) -> Option<f32> {
    view.snapshot
        .gpu
        .and_then(|g| g.temp_c)
        .or(view.snapshot.sensors.gpu_c)
        .or_else(|| {
            view.snapshot.sensors.readings.iter().find_map(|r| {
                r.name
                    .to_ascii_lowercase()
                    .contains("gpu")
                    .then_some(r.celsius)
            })
        })
}

fn line_has_text(line: &Line<'_>) -> bool {
    line.spans.iter().any(|s| !s.content.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widgets::tests_support::{fixture, process};
    use plottypus_core::{GpuSnapshot, TempReading};

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
    fn title_temp_without_gpu_snapshot() {
        let mut fx = fixture("");
        fx.snap.gpu = None;
        fx.snap.sensors.gpu_c = Some(44.0);
        let text = line_text(&title(&fx.view(), &Theme::default()));
        assert!(text.contains("44°"), "{text}");
        assert!(text.contains('—'), "{text}");
        fx.snap.sensors.gpu_c = None;
        fx.snap.sensors.readings = vec![TempReading {
            name: String::from("GPU die"),
            celsius: 39.0,
        }];
        let text = line_text(&title(&fx.view(), &Theme::default()));
        assert!(text.contains("39°"), "{text}");
    }

    #[test]
    fn related_procs_omit_unmeasured_gpu() {
        let mut fx = fixture("");
        let mut hog = process(904, "Xcode", 12.5);
        hog.gpu = 77.0;
        fx.snap.processes = vec![hog];
        let theme = Theme::default();
        let text: String = related_proc_lines(&fx.view(), 4, &theme)
            .iter()
            .map(line_text)
            .collect::<Vec<_>>()
            .join("\n");
        assert!(text.contains("Xcode"), "{text}");
        assert!(text.contains("12.5"), "{text}");
        assert!(text.contains("no per-process gpu"), "{text}");
        assert!(!text.contains("77"), "{text}");
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

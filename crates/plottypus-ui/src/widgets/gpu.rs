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
    if view.snapshot.gpu.is_none() {
        return;
    }

    let specs = spec_items(view);
    let (graph, spec_col, spec_row) = place_specs(inner, !specs.is_empty());
    render_scaled_graph(
        frame,
        graph,
        view.gpu_history,
        theme.gpu,
        theme,
        Scale::Fixed(1.0),
        Axis::Percent,
    );
    if let Some(col) = spec_col {
        let take = usize::from(col.height);
        let lines: Vec<Line> = specs
            .iter()
            .take(take)
            .map(|s| Line::from(Span::styled(s.clone(), theme.dim())))
            .collect();
        frame.render_widget(Paragraph::new(lines), col);
    } else if let Some(row) = spec_row {
        frame.render_widget(
            Paragraph::new(Span::styled(specs.join("  "), theme.dim())),
            row,
        );
    }
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
            if let Some(temp) = gpu.temp_c.or(view.snapshot.sensors.gpu_c) {
                spans.push(Span::raw("  "));
                spans.push(Span::styled(format!("{temp:.0}°"), theme.temp()));
            }
        }
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

fn place_specs(area: Rect, has_specs: bool) -> (Rect, Option<Rect>, Option<Rect>) {
    if !has_specs {
        return (area, None, None);
    }
    if area.width >= 28 {
        let cols = Layout::horizontal([
            Constraint::Fill(1),
            Constraint::Length(1),
            Constraint::Length(12),
        ])
        .split(area);
        return (cols[0], Some(cols[2]), None);
    }
    if area.height >= 2 {
        let rows = Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).split(area);
        return (rows[0], None, Some(rows[1]));
    }
    (area, None, None)
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

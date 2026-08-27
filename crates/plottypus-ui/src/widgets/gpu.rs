use plottypus_core::{Scale, percent_display, watts_display};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::chrome::{
    Axis, Graph, GraphInk, panel_block, panel_title, push_token, render_scaled_graph,
};

use crate::layout::{Degrade, Panel};
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
    let spec = spec_line(view, theme);
    let (plot, spec_row) =
        if inner.height >= 4 && view.degrade != Degrade::Minimal && line_has_text(&spec) {
            let rows = Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).split(inner);
            (rows[0], Some(rows[1]))
        } else {
            (inner, None)
        };
    render_scaled_graph(
        frame,
        plot,
        Graph {
            history: view.gpu_history,
            accent: theme.gpu,
            theme,
            scale: Scale::LOAD,
            axis: Axis::Percent,
            ink: GraphInk::Load(view.snapshot.thermal),
        },
    );
    if let Some(row) = spec_row {
        frame.render_widget(Paragraph::new(spec), row);
    }
}

fn spec_line(view: &AppView<'_>, theme: &Theme) -> Line<'static> {
    let items = spec_items(view);
    let mut spans = Vec::new();
    for item in items {
        push_token(&mut spans, item, theme.dim());
    }
    Line::from(spans)
}

fn title(view: &AppView<'_>, theme: &Theme) -> Line<'static> {
    let mut spans = panel_title("gpu", theme);
    match view.snapshot.gpu {
        None => spans.spans.push(Span::styled("—", theme.dim())),
        Some(gpu) => {
            push_token(
                &mut spans.spans,
                ready_pct(view.ready, gpu.scaled),
                theme.title(),
            );
            if view.ready
                && let Some(watts) = gpu.watts
            {
                push_token(&mut spans.spans, watts_display(watts), theme.gpu());
            }
        }
    }
    if let Some(temp) = gpu_temp_c(view) {
        push_token(&mut spans.spans, format!("{temp:.0}°"), theme.temp());
    }
    spans
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
    use crate::widgets::tests_support::fixture;
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

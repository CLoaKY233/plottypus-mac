use plottypus_core::{Scale, Thermal, percent_display, watts_display};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};

use crate::chrome::{
    Axis, Graph, GraphInk, panel_block, panel_title, push_token, render_scaled_graph,
};
use crate::layout::Panel;
use crate::theme::Theme;
use crate::widgets::AppView;

pub fn render(frame: &mut Frame, area: Rect, view: &AppView<'_>, theme: &Theme) {
    let block = panel_block(
        Panel::Cpu,
        title(view, theme),
        view.is_focused(Panel::Cpu),
        view.is_expanded(Panel::Cpu),
        theme,
    );
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.width == 0 || inner.height == 0 {
        return;
    }

    render_scaled_graph(
        frame,
        inner,
        Graph {
            history: view.cpu_history,
            accent: theme.cpu,
            theme,
            scale: Scale::LOAD,
            axis: Axis::Percent,
            ink: GraphInk::Load(view.snapshot.thermal),
        },
    );
}

fn title(view: &AppView<'_>, theme: &Theme) -> Line<'static> {
    let mut spans = panel_title("cpu", theme).spans;
    push_token(
        &mut spans,
        ready_pct(view.ready, view.snapshot.cpu.scaled),
        theme.title(),
    );
    busy_span(view, theme, &mut spans);
    if view.ready
        && let Some(watts) = view.snapshot.cpu.watts
    {
        push_token(&mut spans, watts_display(watts), theme.cpu());
    }
    if let Some(temp) = view.snapshot.cpu.temp_c.or(view.snapshot.sensors.cpu_c) {
        push_token(&mut spans, format!("{temp:.0}°"), theme.temp());
    }
    if !view.snapshot.thermal.is_nominal() {
        push_token(
            &mut spans,
            thermal_word(view.snapshot.thermal).to_owned(),
            theme.thermal(view.snapshot.thermal),
        );
    }
    Line::from(spans)
}

fn busy_span(view: &AppView<'_>, theme: &Theme, spans: &mut Vec<Span<'static>>) {
    let scaled = view.snapshot.cpu.scaled;
    let active = view.snapshot.cpu.active;
    if view.ready && (scaled - active).abs() > 0.01 {
        push_token(
            spans,
            format!("busy {}", percent_display(active)),
            theme.dim(),
        );
    }
}

fn ready_pct(ready: bool, ratio: f32) -> String {
    if ready {
        percent_display(ratio)
    } else {
        String::from("…")
    }
}

fn thermal_word(thermal: Thermal) -> &'static str {
    match thermal {
        Thermal::Nominal => "nominal",
        Thermal::Fair => "fair",
        Thermal::Serious => "serious",
        Thermal::Critical => "critical",
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::widgets::tests_support::fixture;
    use plottypus_core::Thermal;

    fn line_text(line: &Line<'_>) -> String {
        line.spans.iter().map(|s| s.content.as_ref()).collect()
    }

    #[test]
    fn title_ellipsis_until_ready() {
        let mut fx = fixture("");
        fx.ready = false;
        fx.snap.cpu.active = 0.184;
        fx.snap.cpu.watts = Some(8.24);
        let text = line_text(&title(&fx.view(), &Theme::default()));
        assert!(text.contains("cpu"));
        assert!(text.contains('…'));
        assert!(!text.contains('%'));
        assert!(!text.contains('W'));
    }

    #[test]
    fn title_percent_and_watts() {
        let mut fx = fixture("");
        fx.ready = true;
        fx.snap.cpu.scaled = 0.184;
        fx.snap.cpu.active = 0.184;
        fx.snap.cpu.watts = Some(8.24);
        let text = line_text(&title(&fx.view(), &Theme::default()));
        assert!(text.contains("cpu"));
        assert!(text.contains("18%"));
        assert!(text.contains("8.2W"));
        assert!(!text.contains("busy"), "{text}");
    }

    #[test]
    fn busy_rides_along_when_scaled_diverges() {
        let mut fx = fixture("");
        fx.ready = true;
        fx.snap.cpu.scaled = 0.18;
        fx.snap.cpu.active = 0.41;
        let text = line_text(&title(&fx.view(), &Theme::default()));
        assert!(text.contains("18%"), "{text}");
        assert!(text.contains("busy 41%"), "{text}");
    }

    #[test]
    fn title_marks_thermal_when_not_nominal() {
        let mut fx = fixture("");
        fx.ready = true;
        fx.snap.thermal = Thermal::Fair;
        let text = line_text(&title(&fx.view(), &Theme::default()));
        assert!(text.contains("fair"), "{text}");
        fx.snap.thermal = Thermal::Nominal;
        let text = line_text(&title(&fx.view(), &Theme::default()));
        assert!(!text.contains("nominal"), "{text}");
        assert!(!text.contains("fair"), "{text}");
    }

    #[test]
    fn tiny_temp_title_contains_degree() {
        let mut fx = fixture("");
        fx.ready = true;
        fx.snap.cpu.temp_c = Some(42.0);
        let text = line_text(&title(&fx.view(), &Theme::default()));
        assert!(text.contains("42°"), "{text}");
        fx.snap.cpu.temp_c = None;
        fx.snap.sensors.cpu_c = Some(38.0);
        let text = line_text(&title(&fx.view(), &Theme::default()));
        assert!(text.contains("38°"), "{text}");
    }

    #[test]
    fn ready_pct_matches_header() {
        assert_eq!(ready_pct(false, 0.5), "…");
        assert_eq!(ready_pct(true, 0.184), "18%");
    }
}

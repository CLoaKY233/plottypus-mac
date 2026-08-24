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
            history: view.cpu_history,
            accent: theme.cpu,
            theme,
            scale: Scale::Fixed(1.0),
            axis: Axis::Percent,
            ink: GraphInk::Load(view.snapshot.thermal),
        },
    );
    if let Some(row) = spec_row {
        frame.render_widget(Paragraph::new(spec), row);
    }
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

fn spec_line(view: &AppView<'_>, theme: &Theme) -> Line<'static> {
    let mut spans = Vec::new();
    let name = view.snapshot.soc.name.trim();
    if !name.is_empty() {
        push_token(&mut spans, name.to_owned(), theme.fg());
    }
    if let Some(cores) = core_label(
        view.snapshot.soc.e_cores,
        view.snapshot.soc.p_cores,
        view.snapshot.soc.s_cores,
    ) {
        push_token(&mut spans, cores, theme.dim());
    }
    if let Some(mhz) = view.snapshot.cpu.freq_mhz.filter(|mhz| *mhz > 0) {
        push_token(&mut spans, freq_label(mhz), theme.dim());
    }
    if view.frozen {
        push_token(&mut spans, String::from("paused"), theme.dim());
    }
    Line::from(spans)
}

fn ready_pct(ready: bool, ratio: f32) -> String {
    if ready {
        percent_display(ratio)
    } else {
        String::from("…")
    }
}

fn core_label(e_cores: u8, p_cores: u8, s_cores: u8) -> Option<String> {
    let mut parts = Vec::new();
    if e_cores > 0 {
        parts.push(format!("{e_cores}E"));
    }
    if p_cores > 0 {
        parts.push(format!("{p_cores}P"));
    }
    if s_cores > 0 {
        parts.push(format!("{s_cores}S"));
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join(" + "))
    }
}

fn freq_label(mhz: u32) -> String {
    if mhz >= 1000 {
        format!("{:.1}GHz", f64::from(mhz) / 1000.0)
    } else {
        format!("{mhz}MHz")
    }
}

fn line_has_text(line: &Line<'_>) -> bool {
    line.spans.iter().any(|s| !s.content.is_empty())
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
    fn specs_show_soc_and_hide_nominal() {
        let mut fx = fixture("");
        fx.snap.cpu.temp_c = Some(42.0);
        fx.snap.cpu.freq_mhz = Some(3200);
        fx.snap.thermal = Thermal::Nominal;
        let text = line_text(&spec_line(&fx.view(), &Theme::default()));
        assert!(text.contains("M4 Pro"));
        assert!(text.contains("4E + 8P"));
        assert!(text.contains("3.2GHz"));
    }

    #[test]
    fn specs_mark_paused_and_thermal() {
        let mut fx = fixture("");
        fx.frozen = true;
        fx.snap.thermal = Thermal::Fair;
        let text = line_text(&spec_line(&fx.view(), &Theme::default()));
        assert!(text.contains("paused"));
    }

    #[test]
    fn core_label_omits_zeros() {
        assert_eq!(core_label(4, 8, 0).as_deref(), Some("4E + 8P"));
        assert_eq!(core_label(4, 0, 0).as_deref(), Some("4E"));
        assert_eq!(core_label(0, 8, 0).as_deref(), Some("8P"));
        assert_eq!(core_label(0, 12, 6).as_deref(), Some("12P + 6S"));
        assert_eq!(core_label(0, 0, 0), None);
    }

    #[test]
    fn ready_pct_matches_header() {
        assert_eq!(ready_pct(false, 0.5), "…");
        assert_eq!(ready_pct(true, 0.184), "18%");
    }
}

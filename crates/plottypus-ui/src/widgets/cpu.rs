use plottypus_core::{Scale, Thermal, percent_display, watts_display};
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

    let specs = spec_lines(view, theme);
    if view.is_expanded(Panel::Cpu) {
        render_expanded(frame, inner, view, theme, &specs);
        return;
    }

    let (plot, specs_area) = split_specs(inner, &specs);
    render_scaled_graph(
        frame,
        plot,
        view.cpu_history,
        theme.cpu,
        theme,
        Scale::Fixed(1.0),
        Axis::Percent,
    );
    if let Some(specs_area) = specs_area {
        let take = usize::from(specs_area.height);
        frame.render_widget(
            Paragraph::new(specs.into_iter().take(take).collect::<Vec<_>>()),
            specs_area,
        );
    }
}

fn render_expanded(
    frame: &mut Frame,
    area: Rect,
    view: &AppView<'_>,
    theme: &Theme,
    specs: &[Line<'static>],
) {
    let core_n = view.snapshot.cpu.cores.len().min(24);
    let core_h = u16::try_from(core_n).unwrap_or(0).min(area.height.saturating_sub(6));
    let rows = if core_h > 0 {
        Layout::vertical([
            Constraint::Length(u16::try_from(specs.len()).unwrap_or(1).min(3)),
            Constraint::Fill(1),
            Constraint::Length(core_h),
        ])
        .split(area)
    } else {
        Layout::vertical([Constraint::Length(2), Constraint::Fill(1)]).split(area)
    };
    if !specs.is_empty() {
        frame.render_widget(
            Paragraph::new(specs.to_vec()),
            rows[0],
        );
    }
    if rows.len() > 1 {
        render_scaled_graph(
            frame,
            rows[1],
            view.cpu_history,
            theme.cpu,
            theme,
            Scale::Fixed(1.0),
            Axis::Percent,
        );
    }
    if rows.len() > 2 {
        render_core_list(frame, rows[2], view, theme);
    }
}

fn render_core_list(frame: &mut Frame, area: Rect, view: &AppView<'_>, theme: &Theme) {
    let take = usize::from(area.height).min(view.snapshot.cpu.cores.len());
    let lines: Vec<Line> = view
        .snapshot
        .cpu
        .cores
        .iter()
        .take(take)
        .map(|core| {
            let tag = match core.kind {
                plottypus_core::ClusterKind::Efficiency => "E",
                plottypus_core::ClusterKind::Performance => "P",
                plottypus_core::ClusterKind::Super => "S",
            };
            let bar = meter(core.active, 16);
            Line::from(vec![
                Span::styled(format!(" {tag}{:<2} ", core.index), theme.dim()),
                Span::styled(bar, theme.cpu()),
                Span::styled(
                    format!(" {:>3}", percent_display(core.active)),
                    theme.title(),
                ),
            ])
        })
        .collect();
    frame.render_widget(Paragraph::new(lines), area);
}

fn meter(ratio: f32, width: usize) -> String {
    let filled = ((ratio.clamp(0.0, 1.0) * width as f32).round() as usize).min(width);
    let mut out = String::new();
    for i in 0..width {
        out.push(if i < filled { '█' } else { '░' });
    }
    out
}

fn title(view: &AppView<'_>, theme: &Theme) -> Line<'static> {
    let mut spans = vec![
        Span::styled(" cpu  ", theme.dim()),
        Span::styled(
            ready_pct(view.ready, view.snapshot.cpu.active),
            theme.title(),
        ),
    ];
    if view.ready
        && let Some(watts) = view.snapshot.cpu.watts
    {
        spans.push(Span::raw("  "));
        spans.push(Span::styled(watts_display(watts), theme.cpu()));
    }
    if let Some(temp) = view.snapshot.cpu.temp_c.or(view.snapshot.sensors.cpu_c) {
        spans.push(Span::raw("  "));
        spans.push(Span::styled(format!("{temp:.0}°"), theme.temp()));
    }
    spans.push(Span::raw(" "));
    Line::from(spans)
}

fn spec_lines(view: &AppView<'_>, theme: &Theme) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    let name = view.snapshot.soc.name.trim();
    if !name.is_empty() {
        lines.push(Line::from(Span::styled(name.to_owned(), theme.fg())));
    }
    if let Some(cores) = core_label(view.snapshot.soc.e_cores, view.snapshot.soc.p_cores) {
        lines.push(Line::from(Span::styled(cores, theme.dim())));
    }
    if let Some(temp) = view.snapshot.cpu.temp_c {
        lines.push(Line::from(Span::styled(format!("{temp:.0}°"), theme.dim())));
    }
    if let Some(mhz) = view.snapshot.cpu.freq_mhz.filter(|mhz| *mhz > 0) {
        lines.push(Line::from(Span::styled(freq_label(mhz), theme.dim())));
    }
    if let Some(word) = thermal_word(view.snapshot.thermal) {
        lines.push(Line::from(Span::styled(
            word.to_owned(),
            theme.thermal(view.snapshot.thermal),
        )));
    }
    if view.frozen {
        lines.push(Line::from(Span::styled("paused", theme.dim())));
    }
    lines
}

fn split_specs(area: Rect, specs: &[Line<'static>]) -> (Rect, Option<Rect>) {
    if specs.is_empty() || area.width < 50 {
        return (area, None);
    }
    let cols = Layout::horizontal([
        Constraint::Fill(1),
        Constraint::Length(1),
        Constraint::Length(22),
    ])
    .split(area);
    (cols[0], Some(cols[2]))
}

fn ready_pct(ready: bool, ratio: f32) -> String {
    if ready {
        percent_display(ratio)
    } else {
        String::from("…")
    }
}

fn core_label(e_cores: u8, p_cores: u8) -> Option<String> {
    match (e_cores, p_cores) {
        (0, 0) => None,
        (e, 0) => Some(format!("{e}E")),
        (0, p) => Some(format!("{p}P")),
        (e, p) => Some(format!("{e}E + {p}P")),
    }
}

fn freq_label(mhz: u32) -> String {
    if mhz >= 1000 {
        format!("{:.1}GHz", f64::from(mhz) / 1000.0)
    } else {
        format!("{mhz}MHz")
    }
}

fn thermal_word(thermal: Thermal) -> Option<&'static str> {
    match thermal {
        Thermal::Nominal => None,
        Thermal::Fair => Some("fair"),
        Thermal::Serious => Some("serious"),
        Thermal::Critical => Some("critical"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widgets::tests_support::fixture;
    use plottypus_core::Thermal;

    fn line_text(line: &Line<'_>) -> String {
        line.spans.iter().map(|s| s.content.as_ref()).collect()
    }

    fn lines_text(lines: &[Line<'_>]) -> String {
        lines.iter().map(line_text).collect::<Vec<_>>().join("\n")
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
        fx.snap.cpu.active = 0.184;
        fx.snap.cpu.watts = Some(8.24);
        let text = line_text(&title(&fx.view(), &Theme::default()));
        assert!(text.contains("cpu"));
        assert!(text.contains("18%"));
        assert!(text.contains("8.2W"));
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
        let text = lines_text(&spec_lines(&fx.view(), &Theme::default()));
        assert!(text.contains("M4 Pro"));
        assert!(text.contains("4E + 8P"));
        assert!(text.contains("42°"));
        assert!(text.contains("3.2GHz"));
        assert!(!text.contains("nominal"));
    }

    #[test]
    fn specs_mark_paused_and_thermal() {
        let mut fx = fixture("");
        fx.frozen = true;
        fx.snap.thermal = Thermal::Fair;
        let text = lines_text(&spec_lines(&fx.view(), &Theme::default()));
        assert!(text.contains("paused"));
        assert!(text.contains("fair"));
        assert!(!text.contains("nominal"));
    }

    #[test]
    fn core_label_omits_zeros() {
        assert_eq!(core_label(4, 8).as_deref(), Some("4E + 8P"));
        assert_eq!(core_label(4, 0).as_deref(), Some("4E"));
        assert_eq!(core_label(0, 8).as_deref(), Some("8P"));
        assert_eq!(core_label(0, 0), None);
    }

    #[test]
    fn ready_pct_matches_header() {
        assert_eq!(ready_pct(false, 0.5), "…");
        assert_eq!(ready_pct(true, 0.184), "18%");
    }
}

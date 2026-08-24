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

    if view.is_expanded(Panel::Cpu) {
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
        view.cpu_history,
        theme.cpu,
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
    header.push(stat_line(view, theme));
    let header_h = u16::try_from(header.len()).unwrap_or(1).min(area.height);
    let remain = area.height.saturating_sub(header_h);
    let has_detail = !view.snapshot.cpu.cores.is_empty() || !view.snapshot.processes.is_empty();
    let rows = if has_detail && remain >= 6 {
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
    render_scaled_graph(
        frame,
        rows[1],
        view.cpu_history,
        theme.cpu,
        theme,
        Scale::Fixed(1.0),
        Axis::Percent,
    );
    if let Some(detail) = rows.get(2) {
        render_expanded_detail(frame, *detail, view, theme);
    }
}

fn render_expanded_detail(frame: &mut Frame, area: Rect, view: &AppView<'_>, theme: &Theme) {
    let has_cores = !view.snapshot.cpu.cores.is_empty();
    let has_procs = !view.snapshot.processes.is_empty();
    match (has_cores, has_procs) {
        (true, true) if area.width >= 48 => {
            let cols = Layout::horizontal([Constraint::Fill(3), Constraint::Fill(2)]).split(area);
            render_core_list(frame, cols[0], view, theme);
            render_top_procs(frame, cols[1], view, theme);
        }
        (true, true) => {
            let rows = Layout::vertical([Constraint::Fill(1), Constraint::Fill(1)]).split(area);
            render_core_list(frame, rows[0], view, theme);
            render_top_procs(frame, rows[1], view, theme);
        }
        (true, false) => render_core_list(frame, area, view, theme),
        (false, true) => render_top_procs(frame, area, view, theme),
        (false, false) => {}
    }
}

fn stat_line(view: &AppView<'_>, theme: &Theme) -> Line<'static> {
    let mut parts = vec![Span::styled(" load  ", theme.dim())];
    parts.push(Span::styled(
        ready_pct(view.ready, view.snapshot.cpu.active),
        theme.title(),
    ));
    if let Some(t) = view.snapshot.cpu.temp_c.or(view.snapshot.sensors.cpu_c) {
        parts.push(Span::styled("   temp  ", theme.dim()));
        parts.push(Span::styled(format!("{t:.0}°"), theme.temp()));
    }
    if let Some(hot) = view.snapshot.sensors.hotspot_c {
        parts.push(Span::styled("   hot  ", theme.dim()));
        parts.push(Span::styled(format!("{hot:.0}°"), theme.temp()));
    }
    if let Some(word) = thermal_word(view.snapshot.thermal) {
        parts.push(Span::styled("   thermal  ", theme.dim()));
        parts.push(Span::styled(
            word.to_owned(),
            theme.thermal(view.snapshot.thermal),
        ));
    }
    Line::from(parts)
}

fn render_top_procs(frame: &mut Frame, area: Rect, view: &AppView<'_>, theme: &Theme) {
    if area.height == 0 {
        return;
    }
    let mut procs: Vec<_> = view.snapshot.processes.iter().collect();
    procs.sort_by(|a, b| b.cpu.total_cmp(&a.cpu));
    let take = usize::from(area.height.saturating_sub(1)).min(procs.len());
    let mut lines = vec![Line::from(Span::styled(" busiest", theme.dim()))];
    for proc in procs.into_iter().take(take) {
        lines.push(Line::from(vec![
            Span::styled(format!(" {:>6}  ", proc.pid), theme.dim()),
            Span::styled(format!("{:<16}", truncate(&proc.name, 16)), theme.fg()),
            Span::styled(format!(" {:>5.1}", proc.cpu), theme.cpu()),
        ]));
    }
    frame.render_widget(Paragraph::new(lines), area);
}

fn truncate(name: &str, width: usize) -> String {
    let mut out: String = name.chars().take(width).collect();
    if name.chars().count() > width {
        out.pop();
        out.push('…');
    }
    out
}

fn render_core_list(frame: &mut Frame, area: Rect, view: &AppView<'_>, theme: &Theme) {
    let cores = &view.snapshot.cpu.cores;
    if area.width == 0 || area.height == 0 || cores.is_empty() {
        return;
    }
    let rows_n = usize::from(area.height);
    let cols_n = cores.len().div_ceil(rows_n).max(1);
    let col_w = (usize::from(area.width) / cols_n).max(1);
    let mut rows: Vec<Vec<Span>> = vec![Vec::new(); rows_n];
    for (i, core) in cores.iter().take(rows_n.saturating_mul(cols_n)).enumerate() {
        rows[i % rows_n].extend(core_spans(core, col_w, theme));
    }
    let lines: Vec<Line> = rows.into_iter().map(Line::from).collect();
    frame.render_widget(Paragraph::new(lines), area);
}

fn core_spans(
    core: &plottypus_core::CoreSample,
    width: usize,
    theme: &Theme,
) -> Vec<Span<'static>> {
    let tag = match core.kind {
        plottypus_core::ClusterKind::Efficiency => "E",
        plottypus_core::ClusterKind::Performance => "P",
        plottypus_core::ClusterKind::Super => "S",
    };
    let label = format!(" {tag}{:<2} ", core.index);
    let pct = format!(" {:>4}", percent_display(core.active));
    let meter_w = width.saturating_sub(label.chars().count() + pct.chars().count());
    let mut spans = vec![Span::styled(label, theme.dim())];
    if meter_w >= 4 {
        spans.push(Span::styled(meter(core.active, meter_w), theme.cpu()));
    }
    spans.push(Span::styled(pct, theme.title()));
    spans
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

fn spec_line(view: &AppView<'_>, theme: &Theme) -> Line<'static> {
    let mut spans = Vec::new();
    let name = view.snapshot.soc.name.trim();
    if !name.is_empty() {
        spans.push(Span::styled(format!(" {name}  "), theme.fg()));
    }
    if let Some(cores) = core_label(view.snapshot.soc.e_cores, view.snapshot.soc.p_cores) {
        spans.push(Span::styled(format!("{cores}  "), theme.dim()));
    }
    if let Some(mhz) = view.snapshot.cpu.freq_mhz.filter(|mhz| *mhz > 0) {
        spans.push(Span::styled(format!("{}  ", freq_label(mhz)), theme.dim()));
    }
    if view.frozen {
        spans.push(Span::styled("paused", theme.dim()));
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

fn line_has_text(line: &Line<'_>) -> bool {
    line.spans.iter().any(|s| !s.content.is_empty())
}

#[cfg(test)]
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
        let text = line_text(&spec_line(&fx.view(), &Theme::default()));
        assert!(text.contains("M4 Pro"));
        assert!(text.contains("4E + 8P"));
        assert!(text.contains("3.2GHz"));
        let stats = line_text(&stat_line(&fx.view(), &Theme::default()));
        assert!(stats.contains("42°"));
        assert!(!stats.contains("nominal"), "{stats}");
    }

    #[test]
    fn specs_mark_paused_and_thermal() {
        let mut fx = fixture("");
        fx.frozen = true;
        fx.snap.thermal = Thermal::Fair;
        let text = line_text(&spec_line(&fx.view(), &Theme::default()));
        assert!(text.contains("paused"));
        let stats = line_text(&stat_line(&fx.view(), &Theme::default()));
        assert!(stats.contains("fair"));
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

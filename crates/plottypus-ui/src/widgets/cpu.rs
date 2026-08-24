use plottypus_core::{
    ClusterKind, CoreSample, History, Scale, Thermal, percent_display, watts_display,
};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::chrome::{
    Axis, Graph, GraphInk, panel_block, panel_title, push_kv, push_token, render_fill_bar,
    render_scaled_graph,
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

fn render_expanded(frame: &mut Frame, area: Rect, view: &AppView<'_>, theme: &Theme) {
    let mut header = Vec::new();
    let spec = spec_line(view, theme);
    if line_has_text(&spec) {
        header.push(spec);
    }
    header.push(stat_line(view, theme));
    let header_h = u16::try_from(header.len()).unwrap_or(1).min(area.height);
    let remain = area.height.saturating_sub(header_h);
    let has_detail = has_zone_detail(view) || !view.snapshot.processes.is_empty();
    let rows = if has_detail && remain >= 6 {
        Layout::vertical([
            Constraint::Length(header_h),
            Constraint::Fill(1),
            Constraint::Fill(2),
        ])
        .split(area)
    } else {
        Layout::vertical([Constraint::Length(header_h), Constraint::Fill(1)]).split(area)
    };
    frame.render_widget(Paragraph::new(header), rows[0]);
    render_scaled_graph(
        frame,
        rows[1],
        Graph {
            history: view.cpu_history,
            accent: theme.cpu,
            theme,
            scale: Scale::Fixed(1.0),
            axis: Axis::Percent,
            ink: GraphInk::Load(view.snapshot.thermal),
        },
    );
    if let Some(detail) = rows.get(2) {
        render_expanded_detail(frame, *detail, view, theme);
    }
}

fn render_expanded_detail(frame: &mut Frame, area: Rect, view: &AppView<'_>, theme: &Theme) {
    let zones = cpu_zones(view);
    let has_zones = !zones.is_empty();
    let has_procs = !view.snapshot.processes.is_empty();
    match (has_zones, has_procs) {
        (true, true) if area.width >= 52 => {
            let cols = Layout::horizontal([Constraint::Fill(3), Constraint::Fill(2)]).split(area);
            render_zone_row(frame, cols[0], &zones, view, theme);
            render_top_procs(frame, cols[1], view, theme);
        }
        (true, true) => {
            let rows = Layout::vertical([Constraint::Fill(2), Constraint::Fill(1)]).split(area);
            render_zone_row(frame, rows[0], &zones, view, theme);
            render_top_procs(frame, rows[1], view, theme);
        }
        (true, false) => render_zone_row(frame, area, &zones, view, theme),
        (false, true) => render_top_procs(frame, area, view, theme),
        (false, false) => {}
    }
}

struct ZoneCard<'a> {
    kind: ClusterKind,
    solo: bool,
    load: f32,
    temp: Option<f32>,
    cores: Vec<CoreSample>,
    history: Option<&'a History>,
}

fn has_zone_detail(view: &AppView<'_>) -> bool {
    !view.snapshot.cpu.cores.is_empty()
        || view.snapshot.sensors.e_c.is_some()
        || view.snapshot.sensors.p_c.is_some()
        || view.snapshot.sensors.s_c.is_some()
}

fn cpu_zones<'a>(view: &'a AppView<'_>) -> Vec<ZoneCard<'a>> {
    let cores = &view.snapshot.cpu.cores;
    let mut kinds: Vec<ClusterKind> = ClusterKind::ALL
        .into_iter()
        .filter(|kind| {
            cores.iter().any(|c| c.kind == *kind)
                || view.snapshot.sensors.zone_temp(*kind).is_some()
        })
        .collect();
    let solo = kinds.len() <= 1;
    if kinds.is_empty() && !cores.is_empty() {
        kinds.push(ClusterKind::Performance);
    }
    let mut zones: Vec<ZoneCard<'a>> = kinds
        .into_iter()
        .map(|kind| zone_card(view, kind, solo, cores))
        .collect();
    if !solo
        && zones.iter().all(|z| z.temp.is_none())
        && let Some(temp) = view
            .snapshot
            .cpu
            .temp_c
            .or(view.snapshot.sensors.best_cpu_c())
    {
        zones.push(ZoneCard {
            kind: ClusterKind::Performance,
            solo: true,
            load: view.snapshot.cpu.scaled,
            temp: Some(temp),
            cores: Vec::new(),
            history: nonempty(view.cpu_temp_history),
        });
    }
    zones
}

fn zone_card<'a>(
    view: &'a AppView<'_>,
    kind: ClusterKind,
    solo: bool,
    cores: &[CoreSample],
) -> ZoneCard<'a> {
    let zone_cores: Vec<CoreSample> = cores.iter().copied().filter(|c| c.kind == kind).collect();
    let load = cluster_load(view, kind).unwrap_or_else(|| mean_scaled(&zone_cores));
    let temp = if solo {
        view.snapshot
            .sensors
            .zone_temp(kind)
            .or(view.snapshot.cpu.temp_c)
            .or(view.snapshot.sensors.best_cpu_c())
    } else {
        view.snapshot.sensors.zone_temp(kind)
    };
    let history = if solo {
        nonempty(view.cpu_temp_history).or_else(|| nonempty(view.zone_temp_history(kind)))
    } else {
        nonempty(view.zone_temp_history(kind))
    };
    ZoneCard {
        kind,
        solo,
        load,
        temp,
        cores: zone_cores,
        history,
    }
}

fn cluster_load(view: &AppView<'_>, kind: ClusterKind) -> Option<f32> {
    let cluster = match kind {
        ClusterKind::Efficiency => view.snapshot.cpu.e_cluster,
        ClusterKind::Performance => view.snapshot.cpu.p_cluster,
        ClusterKind::Super => view.snapshot.cpu.s_cluster,
    };
    cluster.map(|c| c.scaled)
}

fn mean_scaled(cores: &[CoreSample]) -> f32 {
    if cores.is_empty() {
        0.0
    } else {
        cores.iter().map(|c| c.scaled).sum::<f32>() / cores.len() as f32
    }
}

fn nonempty(history: &History) -> Option<&History> {
    if history.is_empty() {
        None
    } else {
        Some(history)
    }
}

fn render_zone_row(
    frame: &mut Frame,
    area: Rect,
    zones: &[ZoneCard<'_>],
    view: &AppView<'_>,
    theme: &Theme,
) {
    if area.width == 0 || area.height == 0 || zones.is_empty() {
        return;
    }
    let constraints: Vec<Constraint> = zones.iter().map(|_| Constraint::Fill(1)).collect();
    let cols = Layout::horizontal(constraints).split(area);
    for (zone, col) in zones.iter().zip(cols.iter().copied()) {
        render_zone_card(frame, col, zone, view, theme);
    }
}

fn render_zone_card(
    frame: &mut Frame,
    area: Rect,
    zone: &ZoneCard<'_>,
    view: &AppView<'_>,
    theme: &Theme,
) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let show_cores = view.show_cores && !zone.cores.is_empty();
    let show_bar = area.width >= 8 && area.height >= 2;
    let show_graph = zone.history.is_some() && area.height >= 6;
    let mut parts = vec![Constraint::Length(1)];
    if show_bar {
        parts.push(Constraint::Length(1));
    }
    if show_graph {
        parts.push(Constraint::Fill(1));
    }
    if show_cores {
        parts.push(Constraint::Fill(2));
    }
    let rows = Layout::vertical(parts).split(area);
    frame.render_widget(Paragraph::new(zone_title(zone, theme)), rows[0]);
    let mut i = 1;
    if show_bar && let Some(bar) = rows.get(i) {
        render_fill_bar(frame, *bar, zone.load, theme.cpu);
        i += 1;
    }
    if show_graph
        && let Some(history) = zone.history
        && let Some(plot) = rows.get(i)
    {
        render_scaled_graph(
            frame,
            *plot,
            Graph {
                history,
                accent: theme.temp,
                theme,
                scale: Scale::Fixed(100.0),
                axis: Axis::Celsius,
                ink: GraphInk::Load(view.snapshot.thermal),
            },
        );
        i += 1;
    }
    if show_cores && let Some(row) = rows.get(i) {
        render_core_list(frame, *row, &zone.cores, zone.solo, theme);
    }
}

fn zone_title(zone: &ZoneCard<'_>, theme: &Theme) -> Line<'static> {
    let name = if zone.solo { "cpu" } else { zone.kind.word() };
    let mut parts = vec![
        Span::styled(format!(" {name}  "), theme.dim()),
        Span::styled(percent_display(zone.load), theme.title()),
    ];
    if let Some(temp) = zone.temp {
        parts.push(Span::styled("  ", theme.dim()));
        parts.push(Span::styled(format!("{temp:.0}°"), theme.temp()));
    }
    Line::from(parts)
}

fn stat_line(view: &AppView<'_>, theme: &Theme) -> Line<'static> {
    let mut parts = Vec::new();
    push_kv(
        &mut parts,
        theme,
        "load",
        ready_pct(view.ready, view.snapshot.cpu.scaled),
        theme.title(),
    );
    busy_span_explicit(view, theme, &mut parts);
    if let Some(t) = view.snapshot.cpu.temp_c.or(view.snapshot.sensors.cpu_c) {
        push_kv(&mut parts, theme, "temp", format!("{t:.0}°"), theme.temp());
    }
    if let Some(hot) = view.snapshot.sensors.hotspot_c {
        push_kv(&mut parts, theme, "hot", format!("{hot:.0}°"), theme.temp());
    }
    if let Some(word) = thermal_word(view.snapshot.thermal) {
        push_kv(
            &mut parts,
            theme,
            "thermal",
            word.to_owned(),
            theme.thermal(view.snapshot.thermal),
        );
    }
    Line::from(parts)
}

fn busy_span_explicit(view: &AppView<'_>, theme: &Theme, parts: &mut Vec<Span<'static>>) {
    let scaled = view.snapshot.cpu.scaled;
    let active = view.snapshot.cpu.active;
    if view.ready && (scaled - active).abs() > 0.01 {
        push_kv(parts, theme, "busy", percent_display(active), theme.fg());
    }
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

fn render_core_list(
    frame: &mut Frame,
    area: Rect,
    cores: &[CoreSample],
    solo: bool,
    theme: &Theme,
) {
    if area.width == 0 || area.height == 0 || cores.is_empty() {
        return;
    }
    let rows_n = usize::from(area.height);
    let cols_n = cores.len().div_ceil(rows_n).max(1);
    let col_w = (usize::from(area.width) / cols_n).max(1);
    let mut rows: Vec<Vec<Span>> = vec![Vec::new(); rows_n];
    for (i, core) in cores.iter().take(rows_n.saturating_mul(cols_n)).enumerate() {
        rows[i % rows_n].extend(core_spans(core, col_w, solo, theme));
    }
    let lines: Vec<Line> = rows.into_iter().map(Line::from).collect();
    frame.render_widget(Paragraph::new(lines), area);
}

fn core_spans(core: &CoreSample, width: usize, solo: bool, theme: &Theme) -> Vec<Span<'static>> {
    let tag = if solo { "C" } else { core.kind.tag() };
    let label = format!(" {tag}{:<2} ", core.index);
    let pct = format!(" {:>4}", percent_display(core.scaled));
    let meter_w = width.saturating_sub(label.chars().count() + pct.chars().count());
    let mut spans = vec![Span::styled(label, theme.dim())];
    if meter_w >= 2 {
        spans.push(Span::styled(meter(core.scaled, meter_w), theme.cpu()));
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
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::layout::Panel;
    use crate::widgets::tests_support::fixture;
    use plottypus_core::{ClusterKind, CoreSample, Thermal};

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

    #[test]
    fn clustered_zones_keep_temps_off_cores() {
        let mut fx = fixture("");
        fx.expanded = Some(Panel::Cpu);
        fx.snap.cpu.cores = vec![
            CoreSample {
                kind: ClusterKind::Efficiency,
                index: 0,
                scaled: 0.2,
                active: 0.2,
            },
            CoreSample {
                kind: ClusterKind::Performance,
                index: 0,
                scaled: 0.8,
                active: 0.8,
            },
        ];
        fx.snap.sensors.e_c = Some(36.0);
        fx.snap.sensors.p_c = Some(51.0);
        let view = fx.view();
        let zones = cpu_zones(&view);
        assert_eq!(zones.len(), 2);
        assert!(!zones[0].solo);
        assert_eq!(zones[0].kind, ClusterKind::Efficiency);
        assert_eq!(zones[0].temp, Some(36.0));
        assert_eq!(zones[1].temp, Some(51.0));
        let title = line_text(&zone_title(&zones[0], &Theme::default()));
        assert!(title.contains("efficiency"), "{title}");
        assert!(title.contains("36°"), "{title}");
        assert!(title.contains("20%"), "{title}");
    }

    #[test]
    fn unmapped_cores_are_cpu_not_fake_p() {
        let mut fx = fixture("");
        fx.snap.cpu.cores = vec![CoreSample {
            kind: ClusterKind::Performance,
            index: 0,
            scaled: 0.4,
            active: 0.4,
        }];
        fx.snap.cpu.temp_c = Some(42.0);
        let view = fx.view();
        let zones = cpu_zones(&view);
        assert_eq!(zones.len(), 1);
        assert!(zones[0].solo);
        let title = line_text(&zone_title(&zones[0], &Theme::default()));
        assert!(title.contains("cpu"), "{title}");
        assert!(title.contains("42°"), "{title}");
        let spans = core_spans(&zones[0].cores[0], 16, true, &Theme::default());
        let text: String = spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(text.contains("C0"), "{text}");
        assert!(!text.contains("P0"), "{text}");
        assert!(!text.contains('°'), "{text}");
    }

    #[test]
    fn expanded_paints_zone_bars() {
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;

        let mut fx = fixture("");
        fx.expanded = Some(Panel::Cpu);
        fx.snap.cpu.scaled = 0.4;
        fx.snap.cpu.active = 0.4;
        fx.snap.cpu.cores = vec![
            CoreSample {
                kind: ClusterKind::Performance,
                index: 0,
                scaled: 0.8,
                active: 0.8,
            },
            CoreSample {
                kind: ClusterKind::Super,
                index: 0,
                scaled: 0.3,
                active: 0.3,
            },
        ];
        fx.snap.sensors.p_c = Some(48.0);
        fx.snap.sensors.s_c = Some(52.0);
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| render(frame, frame.area(), &fx.view(), &Theme::default()))
            .unwrap();
        let buf = terminal.backend().buffer();
        let mut text = String::new();
        for y in 0..buf.area.height {
            for x in 0..buf.area.width {
                text.push_str(buf[(x, y)].symbol());
            }
            text.push('\n');
        }
        assert!(text.contains("performance"), "{text}");
        assert!(text.contains("super"), "{text}");
        assert!(text.contains("48°"), "{text}");
        assert!(text.contains("52°"), "{text}");
        assert!(text.contains('━') || text.contains('█'), "{text}");
        assert!(text.contains("P0"), "{text}");
        assert!(text.contains("S0"), "{text}");
    }
}

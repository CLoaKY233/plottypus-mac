use plottypus_core::{
    ClusterKind, CoreSample, History, Pressure, Scale, Thermal, bits_per_sec, bytes_short,
    percent_display, watts_display,
};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, Paragraph};

use crate::chrome::{
    Axis, Graph, GraphInk, cell, push_kv, push_token, render_fill_bar, render_scaled_graph,
};
use crate::layout::Panel;
use crate::theme::Theme;
use crate::widgets::AppView;

pub fn render(frame: &mut Frame, area: Rect, view: &AppView<'_>, theme: &Theme, panel: Panel) {
    match panel {
        Panel::Cpu => cpu(frame, area, view, theme),
        Panel::Gpu => gpu(frame, area, view, theme),
        Panel::Mem => mem(frame, area, view, theme),
        Panel::Net => net(frame, area, view, theme),
        Panel::Disk => disk(frame, area, view, theme),
        Panel::Fans => sensors(frame, area, view, theme),
        Panel::Processes => super::processes::render(frame, area, view, theme),
    }
}

fn rows_of(area: Rect, weights: &[u16]) -> Vec<Rect> {
    let constraints: Vec<Constraint> = weights.iter().map(|w| Constraint::Fill(*w)).collect();
    Layout::vertical(constraints).split(area).to_vec()
}

fn cols_of(area: Rect, weights: &[u16]) -> Vec<Rect> {
    let constraints: Vec<Constraint> = weights.iter().map(|w| Constraint::Fill(*w)).collect();
    Layout::horizontal(constraints).split(area).to_vec()
}

fn big(theme: &Theme) -> Style {
    theme.title().add_modifier(Modifier::BOLD)
}

fn kv_cell(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    pairs: &[(&str, String)],
    value_style: Style,
    theme: &Theme,
) {
    let inner = cell(frame, area, title, theme);
    let mut spans = Vec::new();
    for (key, value) in pairs {
        push_kv(&mut spans, theme, key, value.clone(), value_style);
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), inner);
}

#[allow(clippy::too_many_arguments)]
fn graph_cell(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    history: &History,
    accent: ratatui::style::Color,
    thermal: Thermal,
    theme: &Theme,
    scale: Scale,
    axis: Axis,
) {
    let inner = cell(frame, area, title, theme);
    render_scaled_graph(
        frame,
        inner,
        Graph {
            history,
            accent,
            theme,
            scale,
            axis,
            ink: GraphInk::Load(thermal),
        },
    );
}

fn bar_at_bottom(frame: &mut Frame, area: Rect, ratio: f32, color: ratatui::style::Color) {
    let row = Rect {
        x: area.x,
        y: area.y.saturating_add(area.height.saturating_sub(1)),
        width: area.width,
        height: 1,
    };
    render_fill_bar(frame, row, ratio, color);
}

fn bar_row(frame: &mut Frame, area: Rect, ratio: f32, color: ratatui::style::Color) {
    bar_at_bottom(frame, area, ratio, color);
}

fn cpu(frame: &mut Frame, area: Rect, view: &AppView<'_>, theme: &Theme) {
    let rows = rows_of(area, &[4, 9, 7]);
    let stats = cols_of(rows[0], &[2, 1, 1]);

    let load_inner = cell(frame, stats[0], "load", theme);
    let mut load_spans = Vec::new();
    push_token(
        &mut load_spans,
        ready_pct(view.ready, view.snapshot.cpu.scaled),
        big(theme),
    );
    let active = view.snapshot.cpu.active;
    if view.ready && (view.snapshot.cpu.scaled - active).abs() > 0.01 {
        push_token(
            &mut load_spans,
            format!("busy {}", percent_display(active)),
            theme.dim(),
        );
    }
    frame.render_widget(Paragraph::new(Line::from(load_spans)), load_inner);
    bar_row(frame, load_inner, view.snapshot.cpu.scaled, theme.cpu);

    kv_cell(
        frame,
        stats[1],
        "power",
        &[("cpu", watts_or(view.snapshot.cpu.watts))],
        theme.cpu(),
        theme,
    );

    let clock = view
        .snapshot
        .cpu
        .freq_mhz
        .filter(|mhz| *mhz > 0)
        .map_or_else(|| String::from("—"), freq_label);
    kv_cell(
        frame,
        stats[2],
        "clock",
        &[("all cores", clock)],
        theme.fg(),
        theme,
    );

    graph_cell(
        frame,
        rows[1],
        "cpu",
        view.cpu_history,
        theme.cpu,
        view.snapshot.thermal,
        theme,
        Scale::Fixed(1.0),
        Axis::Percent,
    );

    let kinds: Vec<ClusterKind> = ClusterKind::ALL
        .into_iter()
        .filter(|kind| has_cores(view, *kind))
        .collect();
    if kinds.is_empty() || rows[2].height < 4 {
        return;
    }
    let cols = cols_of(rows[2], &vec![1; kinds.len()]);
    for (kind, col) in kinds.iter().zip(cols.iter().copied()) {
        cluster_cell(frame, col, *kind, view, theme);
    }
}

fn cluster_load(view: &AppView<'_>, kind: ClusterKind) -> Option<f32> {
    let c = &view.snapshot.cpu;
    match kind {
        ClusterKind::Efficiency => c.e_cluster.as_ref().map(|x| x.scaled),
        ClusterKind::Performance => c.p_cluster.as_ref().map(|x| x.scaled),
        ClusterKind::Super => c.s_cluster.as_ref().map(|x| x.scaled),
    }
}

fn has_cores(view: &AppView<'_>, kind: ClusterKind) -> bool {
    view.snapshot.cpu.cores.iter().any(|core| core.kind == kind)
}

fn cluster_cell(
    frame: &mut Frame,
    area: Rect,
    kind: ClusterKind,
    view: &AppView<'_>,
    theme: &Theme,
) {
    let inner = cell(frame, area, kind.word(), theme);
    let parts = rows_of(inner, &[1, 1, 4]);
    let load = cluster_load(view, kind).unwrap_or(0.0);
    let temp = view
        .snapshot
        .sensors
        .zone_temp(kind)
        .map_or_else(|| String::from("—"), |c| format!("{c:.0}°"));
    let spans = vec![
        Span::styled(temp, theme.temp()),
        Span::styled("  ", theme.dim()),
        Span::styled(percent_display(load), theme.title()),
    ];
    frame.render_widget(Paragraph::new(Line::from(spans)), parts[0]);
    render_fill_bar(frame, parts[1], load, theme.cpu);

    let mut cores: Vec<CoreSample> = view
        .snapshot
        .cpu
        .cores
        .iter()
        .copied()
        .filter(|core| core.kind == kind)
        .collect();
    cores.sort_by_key(|core| core.index);
    if !cores.is_empty() && parts[2].height > 0 {
        render_core_grid(frame, parts[2], &cores, theme);
    }
}

fn render_core_grid(frame: &mut Frame, area: Rect, cores: &[CoreSample], theme: &Theme) {
    let height = usize::from(area.height).max(1);
    let mut lines: Vec<Line<'static>> = vec![Line::default(); height];
    for (i, core) in cores.iter().enumerate() {
        let row = i % height;
        let style = if core.scaled > 0.75 {
            theme.cpu()
        } else {
            theme.dim()
        };
        let span = Span::styled(
            format!(
                " {}{:<2}{:>5}",
                core.kind.tag(),
                core.index,
                percent_display(core.scaled)
            ),
            style,
        );
        if let Some(slot) = lines.get_mut(row) {
            slot.spans.push(span);
        }
    }
    frame.render_widget(Paragraph::new(lines), area);
}

fn gpu(frame: &mut Frame, area: Rect, view: &AppView<'_>, theme: &Theme) {
    let Some(gpu) = view.snapshot.gpu else {
        let inner = cell(frame, area, "gpu", theme);
        frame.render_widget(Clear, inner);
        frame.render_widget(
            Paragraph::new(Span::styled("no readings on this machine", theme.dim())),
            inner,
        );
        return;
    };
    let rows = rows_of(area, &[4, 9]);
    let stats = cols_of(rows[0], &[1, 1, 1]);
    kv_cell(
        frame,
        stats[0],
        "util",
        &[("render", ready_pct(view.ready, gpu.scaled))],
        theme.gpu(),
        theme,
    );
    kv_cell(
        frame,
        stats[1],
        "power",
        &[
            ("gpu", watts_or(gpu.watts)),
            ("ane", watts_or(gpu.ane_watts)),
        ],
        theme.fg(),
        theme,
    );
    let clock = gpu
        .freq_mhz
        .filter(|mhz| *mhz > 0)
        .map_or_else(|| String::from("—"), freq_label);
    let temp = gpu
        .temp_c
        .map_or_else(|| String::from("—"), |c| format!("{c:.0}°"));
    kv_cell(
        frame,
        stats[2],
        "clock",
        &[("freq", clock), ("temp", temp)],
        theme.fg(),
        theme,
    );

    let cols = cols_of(rows[1], &[3, 2]);
    graph_cell(
        frame,
        cols[0],
        "gpu util",
        view.gpu_history,
        theme.gpu,
        view.snapshot.thermal,
        theme,
        Scale::Fixed(1.0),
        Axis::Percent,
    );
    graph_cell(
        frame,
        cols[1],
        "gpu temp",
        view.gpu_temp_history,
        theme.temp,
        view.snapshot.thermal,
        theme,
        Scale::Fixed(100.0),
        Axis::Celsius,
    );
}

fn mem(frame: &mut Frame, area: Rect, view: &AppView<'_>, theme: &Theme) {
    let m = &view.snapshot.memory;
    let rows = rows_of(area, &[3, 9, 5]);
    let stats = cols_of(rows[0], &[3, 2, 2]);
    let total = bytes_short(m.total_bytes);
    kv_cell(
        frame,
        stats[0],
        "memory",
        &[
            ("used", format!("{} / {}", bytes_short(m.used_bytes), total)),
            ("pressure", pressure_word(m.pressure).to_owned()),
        ],
        theme.mem(),
        theme,
    );
    kv_cell(
        frame,
        stats[1],
        "swap",
        &[(
            "used",
            format!(
                "{} / {}",
                bytes_short(m.swap_used_bytes),
                bytes_short(m.swap_total_bytes)
            ),
        )],
        theme.fg(),
        theme,
    );
    kv_cell(
        frame,
        stats[2],
        "cached",
        &[("files", bytes_short(m.cache_bytes))],
        theme.fg(),
        theme,
    );

    graph_cell(
        frame,
        rows[1],
        "memory",
        view.mem_history,
        theme.mem,
        view.snapshot.thermal,
        theme,
        Scale::Fixed(1.0),
        Axis::Percent,
    );

    let app = m
        .used_bytes
        .saturating_sub(m.wired_bytes)
        .saturating_sub(m.compressed_bytes);
    let parts = cols_of(rows[2], &[1, 1, 1]);
    composition_cell(
        frame,
        parts[0],
        "wired",
        m.wired_bytes,
        m.total_bytes,
        theme,
    );
    composition_cell(
        frame,
        parts[1],
        "compressed",
        m.compressed_bytes,
        m.total_bytes,
        theme,
    );
    composition_cell(frame, parts[2], "app", app, m.total_bytes, theme);
}

fn composition_cell(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    bytes: u64,
    total: u64,
    theme: &Theme,
) {
    let inner = cell(frame, area, title, theme);
    let ratio = if total == 0 {
        0.0
    } else {
        bytes as f32 / total as f32
    };
    let parts = rows_of(inner, &[1, 3]);
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(bytes_short(bytes), theme.title()))),
        parts[0],
    );
    render_fill_bar(frame, parts[1], ratio, theme.mem);
}

fn net(frame: &mut Frame, area: Rect, view: &AppView<'_>, theme: &Theme) {
    let iface = view.snapshot.network.iface.clone();
    let cols = cols_of(area, &[1, 1]);

    let down_inner = cell(frame, cols[0], &format!("down · {iface}"), theme);
    let parts = rows_of(down_inner, &[1, 8]);
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            format!("↓ {}", bits_per_sec(view.snapshot.network.rx_bps)),
            theme.net(),
        ))),
        parts[0],
    );
    render_scaled_graph(
        frame,
        parts[1],
        Graph {
            history: view.net_rx_history,
            accent: theme.net,
            theme,
            scale: Scale::Auto { floor: 8_000.0 },
            axis: Axis::Bits,
            ink: GraphInk::Flat,
        },
    );

    let up_inner = cell(frame, cols[1], &format!("up · {iface}"), theme);
    let parts = rows_of(up_inner, &[1, 8]);
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            format!("↑ {}", bits_per_sec(view.snapshot.network.tx_bps)),
            theme.net(),
        ))),
        parts[0],
    );
    render_scaled_graph(
        frame,
        parts[1],
        Graph {
            history: view.net_tx_history,
            accent: theme.net,
            theme,
            scale: Scale::Auto { floor: 8_000.0 },
            axis: Axis::Bits,
            ink: GraphInk::Flat,
        },
    );
}

fn disk(frame: &mut Frame, area: Rect, view: &AppView<'_>, theme: &Theme) {
    let cols = cols_of(area, &[2, 3]);
    let vols_inner = cell(frame, cols[0], "volumes", theme);
    let mut lines = Vec::new();
    for vol in &view.snapshot.disk.volumes {
        lines.push(Line::from(vec![
            Span::styled(vol.name.clone(), theme.fg()),
            Span::styled(
                format!(
                    "   {} / {}",
                    bytes_short(vol.used_bytes),
                    bytes_short(vol.total_bytes)
                ),
                theme.dim(),
            ),
        ]));
    }
    if lines.is_empty() {
        lines.push(Line::from(Span::styled(" none found", theme.dim())));
    }
    frame.render_widget(Paragraph::new(lines), vols_inner);

    let right = rows_of(cols[1], &[1, 3]);
    kv_cell(
        frame,
        right[0],
        "activity",
        &[
            ("read", per_sec(view.snapshot.disk.read_bps)),
            ("write", per_sec(view.snapshot.disk.write_bps)),
        ],
        theme.disk(),
        theme,
    );
    graph_cell(
        frame,
        right[1],
        "primary volume",
        view.disk_history,
        theme.disk,
        view.snapshot.thermal,
        theme,
        Scale::Fixed(1.0),
        Axis::Percent,
    );
}

fn sensors(frame: &mut Frame, area: Rect, view: &AppView<'_>, theme: &Theme) {
    let fans = &view.snapshot.fans.fans;
    let n = fans.len().min(4);
    let fan_rows = if n == 0 { 0 } else { 3 };
    let show_gpu = view.snapshot.sensors.gpu_c.is_some() || !view.gpu_temp_history.is_empty();
    let rows = rows_of(
        area,
        &[u16::from(fan_rows != 0) * 3 + u16::from(fan_rows == 0), 10],
    );
    let _ = fan_rows;

    if n > 0 {
        let cols = cols_of(rows[0], &vec![1; n]);
        for (fan, col) in fans.iter().zip(cols.iter().copied()) {
            let inner = cell(frame, col, &fan.name, theme);
            frame.render_widget(
                Paragraph::new(Line::from(Span::styled(
                    format!("{} rpm", fan.rpm),
                    big(theme),
                ))),
                inner,
            );
            bar_at_bottom(frame, inner, fan.ratio(), theme.fan);
        }
        let graphs = if show_gpu {
            let cols = cols_of(rows[1], &[1, 1]);
            graph_cell(
                frame,
                cols[0],
                "cpu temp",
                view.cpu_temp_history,
                theme.temp,
                view.snapshot.thermal,
                theme,
                Scale::Fixed(100.0),
                Axis::Celsius,
            );
            graph_cell(
                frame,
                cols[1],
                "gpu temp",
                view.gpu_temp_history,
                theme.gpu,
                view.snapshot.thermal,
                theme,
                Scale::Fixed(100.0),
                Axis::Celsius,
            );
            None
        } else {
            Some(rows[1])
        };
        if let Some(area) = graphs {
            graph_cell(
                frame,
                area,
                "cpu temp",
                view.cpu_temp_history,
                theme.temp,
                view.snapshot.thermal,
                theme,
                Scale::Fixed(100.0),
                Axis::Celsius,
            );
        }
        return;
    }

    let extras: Vec<(String, f32)> = view
        .snapshot
        .sensors
        .readings
        .iter()
        .map(|r| (r.name.clone(), r.celsius))
        .collect();
    let cols = cols_of(area, &[3, 2]);
    let list_inner = cell(frame, cols[0], "readings", theme);
    let lines: Vec<Line<'static>> = extras
        .iter()
        .take(usize::from(list_inner.height))
        .map(|(name, c)| {
            Line::from(Span::styled(
                format!(" {name}  {c:.0}°"),
                Style::default().fg(theme.temp_color(*c)),
            ))
        })
        .collect();
    frame.render_widget(Paragraph::new(lines), list_inner);
    graph_cell(
        frame,
        cols[1],
        "cpu temp",
        view.cpu_temp_history,
        theme.temp,
        view.snapshot.thermal,
        theme,
        Scale::Fixed(100.0),
        Axis::Celsius,
    );
}

fn ready_pct(ready: bool, ratio: f32) -> String {
    if ready {
        percent_display(ratio)
    } else {
        String::from("…")
    }
}

fn watts_or(watts: Option<f32>) -> String {
    match watts {
        Some(w) if w > 0.0 => watts_display(w),
        _ => String::from("—"),
    }
}

fn freq_label(mhz: u32) -> String {
    if mhz >= 1000 {
        format!("{:.1}GHz", f64::from(mhz) / 1000.0)
    } else {
        format!("{mhz}MHz")
    }
}

fn pressure_word(pressure: Pressure) -> &'static str {
    match pressure {
        Pressure::Nominal => "nominal",
        Pressure::Warn => "warn",
        Pressure::Critical => "critical",
    }
}

fn per_sec(bps: u64) -> String {
    format!("{}/s", bytes_short(bps))
}

use plottypus_core::{
    ClusterKind, CoreSample, History, Pressure, Scale, Thermal, bits_per_sec, bytes_per_sec,
    bytes_short, percent_display, watts_display,
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

fn cpu(frame: &mut Frame, area: Rect, view: &AppView<'_>, theme: &Theme) {
    let kinds: Vec<ClusterKind> = ClusterKind::ALL
        .into_iter()
        .filter(|kind| has_cores(view, *kind))
        .collect();
    let show_clusters = !kinds.is_empty() && area.height >= 12;
    let rows = if show_clusters {
        Layout::vertical([
            Constraint::Length(5),
            Constraint::Fill(2),
            Constraint::Fill(1),
        ])
        .split(area)
        .to_vec()
    } else {
        Layout::vertical([Constraint::Length(5.min(area.height)), Constraint::Fill(1)])
            .split(area)
            .to_vec()
    };
    cpu_stats(frame, rows[0], view, theme);
    cpu_graphs(frame, rows[1], view, theme);
    if !show_clusters {
        return;
    }
    let cols = cols_of(rows[2], &vec![1; kinds.len()]);
    for (kind, col) in kinds.iter().zip(cols.iter().copied()) {
        cluster_cell(frame, col, *kind, view, theme);
    }
}

fn cpu_graphs(frame: &mut Frame, area: Rect, view: &AppView<'_>, theme: &Theme) {
    let has_temp = view.snapshot.cpu.temp_c.is_some()
        || view.snapshot.sensors.best_cpu_c().is_some()
        || !view.cpu_temp_history.is_empty();
    if has_temp {
        let cols = cols_of(area, &[3, 2]);
        graph_cell(
            frame,
            cols[0],
            "cpu",
            view.cpu_history,
            theme.cpu,
            view.snapshot.thermal,
            theme,
            Scale::LOAD,
            Axis::Percent,
        );
        graph_cell(
            frame,
            cols[1],
            "cpu temp",
            view.cpu_temp_history,
            theme.temp,
            view.snapshot.thermal,
            theme,
            Scale::TEMP,
            Axis::Celsius,
        );
        return;
    }
    graph_cell(
        frame,
        area,
        "cpu",
        view.cpu_history,
        theme.cpu,
        view.snapshot.thermal,
        theme,
        Scale::LOAD,
        Axis::Percent,
    );
}

fn cpu_stats(frame: &mut Frame, area: Rect, view: &AppView<'_>, theme: &Theme) {
    let watts = view.snapshot.cpu.watts.filter(|w| *w > 0.0);
    let freq = view.snapshot.cpu.freq_mhz.filter(|mhz| *mhz > 0);
    let temp = view
        .snapshot
        .cpu
        .temp_c
        .or(view.snapshot.sensors.best_cpu_c());
    let thermal = view.snapshot.thermal;
    let mut stat_w = vec![2_u16];
    if watts.is_some() {
        stat_w.push(1);
    }
    if freq.is_some() {
        stat_w.push(1);
    }
    if temp.is_some() {
        stat_w.push(1);
    }
    if !thermal.is_nominal() {
        stat_w.push(1);
    }
    let stats = cols_of(area, &stat_w);
    let mut i = 0;
    cpu_load_cell(frame, stats[i], view, theme);
    i += 1;
    if let Some(watts) = watts
        && let Some(slot) = stats.get(i).copied()
    {
        kv_cell(
            frame,
            slot,
            "power",
            &[("cpu", watts_display(watts))],
            theme.cpu(),
            theme,
        );
        i += 1;
    }
    if let Some(mhz) = freq
        && let Some(slot) = stats.get(i).copied()
    {
        kv_cell(
            frame,
            slot,
            "clock",
            &[("all cores", freq_label(mhz))],
            theme.fg(),
            theme,
        );
        i += 1;
    }
    if let Some(c) = temp
        && let Some(slot) = stats.get(i).copied()
    {
        kv_cell(
            frame,
            slot,
            "temp",
            &[("package", format!("{c:.0}°"))],
            theme.temp(),
            theme,
        );
        i += 1;
    }
    if !thermal.is_nominal()
        && let Some(slot) = stats.get(i).copied()
    {
        kv_cell(
            frame,
            slot,
            "thermal",
            &[("state", thermal_word(thermal).to_owned())],
            theme.thermal(thermal),
            theme,
        );
    }
}

fn cpu_load_cell(frame: &mut Frame, area: Rect, view: &AppView<'_>, theme: &Theme) {
    let inner = cell(frame, area, "load", theme);
    let mut spans = Vec::new();
    push_token(
        &mut spans,
        ready_pct(view.ready, view.snapshot.cpu.scaled),
        big(theme),
    );
    let active = view.snapshot.cpu.active;
    if view.ready && (view.snapshot.cpu.scaled - active).abs() > 0.01 {
        push_token(
            &mut spans,
            format!("busy {}", percent_display(active)),
            theme.dim(),
        );
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), inner);
    bar_at_bottom(frame, inner, view.snapshot.cpu.scaled, theme.cpu);
}

fn cluster_load(view: &AppView<'_>, kind: ClusterKind) -> f32 {
    let from_cluster = match kind {
        ClusterKind::Efficiency => view.snapshot.cpu.e_cluster.as_ref().map(|x| x.scaled),
        ClusterKind::Performance => view.snapshot.cpu.p_cluster.as_ref().map(|x| x.scaled),
        ClusterKind::Super => view.snapshot.cpu.s_cluster.as_ref().map(|x| x.scaled),
    };
    if let Some(load) = from_cluster {
        return load;
    }
    let mut sum = 0.0_f32;
    let mut n = 0_u32;
    for core in &view.snapshot.cpu.cores {
        if core.kind == kind {
            sum += core.scaled;
            n += 1;
        }
    }
    if n == 0 { 0.0 } else { sum / n as f32 }
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
    let show_cores = view.show_cores && view.snapshot.cpu.cores.iter().any(|c| c.kind == kind);
    let parts = if show_cores && inner.height >= 4 {
        rows_of(inner, &[1, 1, 4])
    } else {
        rows_of(inner, &[1, 1])
    };
    let load = cluster_load(view, kind);
    let mut spans = Vec::new();
    push_token(&mut spans, percent_display(load), theme.title());
    if let Some(c) = view.snapshot.sensors.zone_temp(kind) {
        push_token(&mut spans, format!("{c:.0}°"), theme.temp());
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), parts[0]);
    render_fill_bar(frame, parts[1], load, theme.cpu);

    if !show_cores || parts.len() < 3 {
        return;
    }
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
    let Some(sample) = view.snapshot.gpu else {
        let inner = cell(frame, area, "gpu", theme);
        frame.render_widget(Clear, inner);
        frame.render_widget(
            Paragraph::new(Span::styled("no readings on this machine", theme.dim())),
            inner,
        );
        return;
    };
    let temp = sample.temp_c.or(view.snapshot.sensors.gpu_c);
    let rows = rows_of(area, &[4, 9]);
    gpu_stats(frame, rows[0], view, sample, theme);
    gpu_graphs(
        frame,
        rows[1],
        view,
        temp.is_some() || !view.gpu_temp_history.is_empty(),
        theme,
    );
}

fn gpu_stats(
    frame: &mut Frame,
    area: Rect,
    view: &AppView<'_>,
    sample: plottypus_core::GpuSnapshot,
    theme: &Theme,
) {
    let watts = sample.watts.filter(|w| *w > 0.0);
    let ane = sample.ane_watts.filter(|w| *w > 0.0);
    let freq = sample.freq_mhz.filter(|mhz| *mhz > 0);
    let temp = sample.temp_c.or(view.snapshot.sensors.gpu_c);
    let cores = view.snapshot.soc.gpu_cores;
    let mut stat_w = vec![1_u16];
    if watts.is_some() || ane.is_some() {
        stat_w.push(1);
    }
    if freq.is_some() {
        stat_w.push(1);
    }
    if temp.is_some() {
        stat_w.push(1);
    }
    if cores > 0 {
        stat_w.push(1);
    }
    let stats = cols_of(area, &stat_w);
    let mut i = 0;
    kv_cell(
        frame,
        stats[i],
        "util",
        &[("render", ready_pct(view.ready, sample.scaled))],
        theme.gpu(),
        theme,
    );
    i += 1;
    if (watts.is_some() || ane.is_some())
        && let Some(slot) = stats.get(i).copied()
    {
        let mut pairs = Vec::new();
        if let Some(w) = watts {
            pairs.push(("gpu", watts_display(w)));
        }
        if let Some(w) = ane {
            pairs.push(("ane", watts_display(w)));
        }
        kv_cell(frame, slot, "power", &pairs, theme.fg(), theme);
        i += 1;
    }
    if let Some(mhz) = freq
        && let Some(slot) = stats.get(i).copied()
    {
        kv_cell(
            frame,
            slot,
            "clock",
            &[("freq", freq_label(mhz))],
            theme.fg(),
            theme,
        );
        i += 1;
    }
    if let Some(c) = temp
        && let Some(slot) = stats.get(i).copied()
    {
        kv_cell(
            frame,
            slot,
            "temp",
            &[("gpu", format!("{c:.0}°"))],
            theme.temp(),
            theme,
        );
        i += 1;
    }
    if cores > 0
        && let Some(slot) = stats.get(i).copied()
    {
        kv_cell(
            frame,
            slot,
            "cores",
            &[("gpu", format!("{cores}c"))],
            theme.fg(),
            theme,
        );
    }
}

fn gpu_graphs(frame: &mut Frame, area: Rect, view: &AppView<'_>, with_temp: bool, theme: &Theme) {
    if with_temp {
        let cols = cols_of(area, &[3, 2]);
        graph_cell(
            frame,
            cols[0],
            "gpu util",
            view.gpu_history,
            theme.gpu,
            view.snapshot.thermal,
            theme,
            Scale::LOAD,
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
            Scale::TEMP,
            Axis::Celsius,
        );
        return;
    }
    graph_cell(
        frame,
        area,
        "gpu util",
        view.gpu_history,
        theme.gpu,
        view.snapshot.thermal,
        theme,
        Scale::LOAD,
        Axis::Percent,
    );
}

fn mem(frame: &mut Frame, area: Rect, view: &AppView<'_>, theme: &Theme) {
    let m = &view.snapshot.memory;
    let show_swap = m.swap_used_bytes > 0 || m.swap_total_bytes > 0;
    let show_cache = m.cache_bytes > 0;
    let app = m
        .used_bytes
        .saturating_sub(m.wired_bytes)
        .saturating_sub(m.compressed_bytes);
    let mut parts: Vec<(&str, u64)> = Vec::new();
    if m.wired_bytes > 0 {
        parts.push(("wired", m.wired_bytes));
    }
    if m.compressed_bytes > 0 {
        parts.push(("compressed", m.compressed_bytes));
    }
    if app > 0 {
        parts.push(("app", app));
    }
    let show_parts = !parts.is_empty();

    let mut weights = vec![3_u16, 9];
    if show_parts {
        weights.push(5);
    }
    let rows = rows_of(area, &weights);

    let mut stat_w = vec![3_u16];
    if show_swap {
        stat_w.push(2);
    }
    if show_cache {
        stat_w.push(2);
    }
    let stats = cols_of(rows[0], &stat_w);
    let mut i = 0;
    kv_cell(
        frame,
        stats[i],
        "memory",
        &[
            (
                "used",
                format!(
                    "{} / {}",
                    bytes_short(m.used_bytes),
                    bytes_short(m.total_bytes)
                ),
            ),
            ("pressure", pressure_word(m.pressure).to_owned()),
        ],
        theme.mem(),
        theme,
    );
    i += 1;
    if show_swap && let Some(area) = stats.get(i).copied() {
        kv_cell(
            frame,
            area,
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
        i += 1;
    }
    if show_cache && let Some(area) = stats.get(i).copied() {
        kv_cell(
            frame,
            area,
            "cached",
            &[("files", bytes_short(m.cache_bytes))],
            theme.fg(),
            theme,
        );
    }

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

    if show_parts && rows.len() > 2 {
        let cols = cols_of(rows[2], &vec![1; parts.len()]);
        for ((title, bytes), col) in parts.into_iter().zip(cols) {
            composition_cell(frame, col, title, bytes, m.total_bytes, theme);
        }
    }
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
    let title_down = if iface.is_empty() {
        String::from("down")
    } else {
        format!("down · {iface}")
    };
    let title_up = if iface.is_empty() {
        String::from("up")
    } else {
        format!("up · {iface}")
    };
    let cols = cols_of(area, &[1, 1]);

    let down_inner = cell(frame, cols[0], &title_down, theme);
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

    let up_inner = cell(frame, cols[1], &title_up, theme);
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
    let vol_rows = if view.snapshot.disk.volumes.is_empty() {
        1
    } else {
        view.snapshot
            .disk
            .volumes
            .len()
            .min(usize::from(vols_inner.height.max(1)))
    };
    if view.snapshot.disk.volumes.is_empty() {
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(" none found", theme.dim()))),
            vols_inner,
        );
    } else if vol_rows > 0 && vols_inner.height > 0 {
        let each = (vols_inner.height / vol_rows as u16).max(1);
        let mut y = vols_inner.y;
        for vol in view.snapshot.disk.volumes.iter().take(vol_rows) {
            if y >= vols_inner.y.saturating_add(vols_inner.height) {
                break;
            }
            let h = each.min(
                vols_inner
                    .y
                    .saturating_add(vols_inner.height)
                    .saturating_sub(y),
            );
            let slot = Rect {
                x: vols_inner.x,
                y,
                width: vols_inner.width,
                height: h,
            };
            frame.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::styled(vol.name.clone(), theme.fg()),
                    Span::styled(
                        format!(
                            "   {} / {}",
                            bytes_short(vol.used_bytes),
                            bytes_short(vol.total_bytes)
                        ),
                        theme.dim(),
                    ),
                ])),
                Rect { height: 1, ..slot },
            );
            if slot.height >= 2 {
                render_fill_bar(
                    frame,
                    Rect {
                        y: slot.y.saturating_add(1),
                        height: 1,
                        ..slot
                    },
                    vol.ratio(),
                    theme.disk,
                );
            }
            y = y.saturating_add(h);
        }
    }

    let right = rows_of(cols[1], &[1, 4]);
    kv_cell(
        frame,
        right[0],
        "activity",
        &[
            ("read", bytes_per_sec(view.snapshot.disk.read_bps)),
            ("write", bytes_per_sec(view.snapshot.disk.write_bps)),
        ],
        theme.disk(),
        theme,
    );
    let io = cols_of(right[1], &[1, 1]);
    graph_cell(
        frame,
        io[0],
        "read io",
        view.disk_read_history,
        theme.disk,
        Thermal::Nominal,
        theme,
        Scale::Auto { floor: 1_024.0 },
        Axis::Bytes,
    );
    graph_cell(
        frame,
        io[1],
        "write io",
        view.disk_write_history,
        theme.disk,
        Thermal::Nominal,
        theme,
        Scale::Auto { floor: 1_024.0 },
        Axis::Bytes,
    );
}

fn sensors(frame: &mut Frame, area: Rect, view: &AppView<'_>, theme: &Theme) {
    let fans = &view.snapshot.fans.fans;
    let n = fans.len().min(4);
    let plots = TempPlots {
        package: view.snapshot.sensors.best_cpu_c().is_some() || !view.cpu_temp_history.is_empty(),
        discrete: view.snapshot.sensors.gpu_c.is_some()
            || view.snapshot.gpu.and_then(|g| g.temp_c).is_some()
            || !view.gpu_temp_history.is_empty(),
    };
    let extras: Vec<(String, f32)> = view
        .snapshot
        .sensors
        .readings
        .iter()
        .map(|r| (r.name.clone(), r.celsius))
        .collect();
    let show_readings = !extras.is_empty();
    let graph_count = u16::from(plots.package) + u16::from(plots.discrete);

    let mut row_w = Vec::new();
    if n > 0 {
        row_w.push(5_u16);
    }
    if graph_count > 0 || show_readings {
        row_w.push(6);
    }
    if row_w.is_empty() {
        return;
    }
    let rows = rows_of(area, &row_w);
    let mut row = 0;
    if n > 0 {
        let cols = cols_of(rows[row], &vec![1; n]);
        for (i, (fan, col)) in fans.iter().zip(cols.iter().copied()).enumerate() {
            fan_cell(frame, col, fan, view.fan_histories.get(i), theme);
        }
        row += 1;
    }
    if row >= rows.len() {
        return;
    }
    let bottom = rows[row];
    match (graph_count > 0, show_readings) {
        (true, true) => {
            let cols = cols_of(bottom, &[3, 2]);
            render_temp_graphs(frame, cols[0], view, plots, theme);
            render_readings(frame, cols[1], &extras, theme);
        }
        (true, false) => {
            render_temp_graphs(frame, bottom, view, plots, theme);
        }
        (false, true) => render_readings(frame, bottom, &extras, theme),
        (false, false) => {}
    }
}

#[derive(Clone, Copy)]
struct TempPlots {
    package: bool,
    discrete: bool,
}

fn render_temp_graphs(
    frame: &mut Frame,
    area: Rect,
    view: &AppView<'_>,
    plots: TempPlots,
    theme: &Theme,
) {
    match (plots.package, plots.discrete) {
        (true, true) => {
            let cols = cols_of(area, &[1, 1]);
            graph_cell(
                frame,
                cols[0],
                "cpu temp",
                view.cpu_temp_history,
                theme.temp,
                view.snapshot.thermal,
                theme,
                Scale::TEMP,
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
                Scale::TEMP,
                Axis::Celsius,
            );
        }
        (true, false) => graph_cell(
            frame,
            area,
            "cpu temp",
            view.cpu_temp_history,
            theme.temp,
            view.snapshot.thermal,
            theme,
            Scale::TEMP,
            Axis::Celsius,
        ),
        (false, true) => graph_cell(
            frame,
            area,
            "gpu temp",
            view.gpu_temp_history,
            theme.gpu,
            view.snapshot.thermal,
            theme,
            Scale::TEMP,
            Axis::Celsius,
        ),
        (false, false) => {}
    }
}

fn fan_cell(
    frame: &mut Frame,
    area: Rect,
    fan: &plottypus_core::FanMetric,
    history: Option<&History>,
    theme: &Theme,
) {
    let inner = cell(frame, area, &fan.name, theme);
    let show_graph = history.is_some_and(|h| !h.is_empty()) && inner.height >= 3;
    let parts = if show_graph {
        rows_of(inner, &[1, 4])
    } else {
        vec![inner]
    };
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            format!("{} rpm", fan.rpm),
            big(theme),
        ))),
        parts[0],
    );
    if show_graph
        && let Some(history) = history
        && parts.len() > 1
    {
        render_scaled_graph(
            frame,
            parts[1],
            Graph {
                history,
                accent: theme.fan,
                theme,
                scale: Scale::FAN,
                axis: Axis::Number,
                ink: GraphInk::Flat,
            },
        );
    }
    bar_at_bottom(frame, inner, fan.ratio(), theme.fan);
}

fn render_readings(frame: &mut Frame, area: Rect, extras: &[(String, f32)], theme: &Theme) {
    let list_inner = cell(frame, area, "readings", theme);
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

fn thermal_word(thermal: Thermal) -> &'static str {
    match thermal {
        Thermal::Nominal => "nominal",
        Thermal::Fair => "fair",
        Thermal::Serious => "serious",
        Thermal::Critical => "critical",
    }
}

fn pressure_word(pressure: Pressure) -> &'static str {
    match pressure {
        Pressure::Nominal => "nominal",
        Pressure::Warn => "warn",
        Pressure::Critical => "critical",
    }
}

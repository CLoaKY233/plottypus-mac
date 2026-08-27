use plottypus_core::{
    ClusterKind, CoreSample, History, Pressure, Scale, Thermal, bits_per_sec, bytes_per_sec,
    bytes_short, percent_display, watts_display,
};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::chrome::{
    Axis, Graph, GraphInk, cell_titled, panel_block, panel_title, push_token, render_fill_bar,
    render_scaled_graph,
};
use crate::layout::Panel;
use crate::theme::Theme;
use crate::widgets::AppView;
use crate::widgets::grid::{Band, CellKind, CellSpec, CellTitle, Pack, pack};

const ID_CPU: u8 = 0;
const ID_SUPER_LOAD: u8 = 1;
const ID_PERF_LOAD: u8 = 2;
const ID_EFF_LOAD: u8 = 3;
const ID_GPU_UTIL: u8 = 4;
const ID_SUPER_ZONE: u8 = 10;
const ID_PERF_ZONE: u8 = 11;
const ID_EFF_ZONE: u8 = 12;
const ID_PACKAGE: u8 = 13;
const ID_GPU_TEMP: u8 = 14;
const ID_HOP_CPU: u8 = 20;
const ID_HOP_GPU: u8 = 21;
const ID_HOP_SENS: u8 = 22;
const ID_HOP_FAN: u8 = 23;
const ID_HOP_PROC: u8 = 24;
const ID_HOP_DISK: u8 = 25;
const ID_HOP_NET: u8 = 26;
const ID_SUPER_STRIP: u8 = 30;
const ID_PERF_STRIP: u8 = 31;
const ID_EFF_STRIP: u8 = 32;
const ID_FAN0: u8 = 40;
const ID_READINGS: u8 = 50;
const ID_MEM: u8 = 60;
const ID_NET_DOWN: u8 = 70;
const ID_NET_UP: u8 = 71;
const ID_DISK_READ: u8 = 80;
const ID_DISK_WRITE: u8 = 81;
const ID_VOLUMES: u8 = 82;

pub fn render(frame: &mut Frame, area: Rect, view: &AppView<'_>, theme: &Theme, panel: Panel) {
    if panel == Panel::Processes {
        super::processes::render(frame, area, view, theme);
        return;
    }
    let block = panel_block(
        panel,
        outer_title(panel, view, theme),
        view.is_focused(panel),
        true,
        theme,
    );
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.width == 0 || inner.height == 0 {
        return;
    }
    if panel == Panel::Gpu && view.snapshot.gpu.is_none() {
        frame.render_widget(
            Paragraph::new(Span::styled("no readings on this machine", theme.dim())),
            inner,
        );
        return;
    }
    let placed = pack(inner, &bands_for(panel, view));
    paint_pack(frame, &placed, view, theme, panel);
}

#[must_use]
pub fn hop_hit(area: Rect, view: &AppView<'_>, col: u16, row: u16) -> Option<Panel> {
    let panel = view.expanded?;
    if panel == Panel::Processes {
        return None;
    }
    let inner = inset(area);
    pack(inner, &bands_for(panel, view)).hop_at(col, row)
}

fn inset(area: Rect) -> Rect {
    Rect {
        x: area.x.saturating_add(1),
        y: area.y.saturating_add(1),
        width: area.width.saturating_sub(2),
        height: area.height.saturating_sub(2),
    }
}

fn bands_for(panel: Panel, view: &AppView<'_>) -> Vec<Band> {
    match panel {
        Panel::Cpu => cpu_bands(view),
        Panel::Gpu => gpu_bands(view),
        Panel::Fans => sens_bands(view),
        Panel::Mem => mem_bands(view),
        Panel::Net => net_bands(view),
        Panel::Disk => disk_bands(view),
        Panel::Processes => Vec::new(),
    }
}

fn live(opt: Option<f32>, history: &History) -> bool {
    opt.is_some() || !history.is_empty()
}

fn graph_spec(id: u8, label: &'static str, value: Option<String>, present: bool) -> CellSpec {
    CellSpec {
        id,
        kind: CellKind::Graph,
        title: CellTitle {
            label,
            value,
            hop: None,
        },
        min: (16, 5),
        weight: 1,
        present,
    }
}

fn spark_spec(
    id: u8,
    label: &'static str,
    value: Option<String>,
    hop: Panel,
    present: bool,
) -> CellSpec {
    CellSpec {
        id,
        kind: CellKind::Spark,
        title: CellTitle {
            label,
            value,
            hop: Some(hop),
        },
        min: (12, 3),
        weight: 1,
        present,
    }
}

fn cluster_spec(id: u8, label: &'static str, value: Option<String>, present: bool) -> CellSpec {
    CellSpec {
        id,
        kind: CellKind::Cluster,
        title: CellTitle {
            label,
            value,
            hop: None,
        },
        min: (14, 5),
        weight: 1,
        present,
    }
}

#[allow(clippy::too_many_lines)]
fn cpu_bands(view: &AppView<'_>) -> Vec<Band> {
    let s = has_cluster(view, ClusterKind::Super);
    let p = has_cluster(view, ClusterKind::Performance);
    let e = has_cluster(view, ClusterKind::Efficiency);
    let grow = view.show_cores && (s || p || e);
    vec![
        Band {
            min_height: 5,
            grow_to: None,
            cells: vec![
                graph_spec(
                    ID_CPU,
                    "cpu",
                    Some(ready_pct(view.ready, view.snapshot.cpu.scaled)),
                    true,
                ),
                graph_spec(
                    ID_SUPER_LOAD,
                    "super",
                    Some(percent_display(cluster_load(view, ClusterKind::Super))),
                    s,
                ),
                graph_spec(
                    ID_PERF_LOAD,
                    "performance",
                    Some(percent_display(cluster_load(
                        view,
                        ClusterKind::Performance,
                    ))),
                    p,
                ),
                graph_spec(
                    ID_EFF_LOAD,
                    "efficiency",
                    Some(percent_display(cluster_load(view, ClusterKind::Efficiency))),
                    e,
                ),
            ],
        },
        Band {
            min_height: 5,
            grow_to: None,
            cells: vec![
                graph_spec(
                    ID_SUPER_ZONE,
                    "super zone",
                    zone_value(view, ClusterKind::Super),
                    live(view.snapshot.sensors.s_c, view.s_temp_history),
                ),
                graph_spec(
                    ID_PERF_ZONE,
                    "perf zone",
                    zone_value(view, ClusterKind::Performance),
                    live(view.snapshot.sensors.p_c, view.p_temp_history),
                ),
                graph_spec(
                    ID_EFF_ZONE,
                    "eff zone",
                    zone_value(view, ClusterKind::Efficiency),
                    live(view.snapshot.sensors.e_c, view.e_temp_history),
                ),
                graph_spec(
                    ID_PACKAGE,
                    "package",
                    view.snapshot
                        .cpu
                        .temp_c
                        .or(view.snapshot.sensors.best_cpu_c())
                        .map(|c| format!("{c:.0}°")),
                    live(
                        view.snapshot
                            .cpu
                            .temp_c
                            .or(view.snapshot.sensors.best_cpu_c()),
                        view.cpu_temp_history,
                    ),
                ),
            ],
        },
        Band {
            min_height: 3,
            grow_to: None,
            cells: vec![
                spark_spec(
                    ID_HOP_GPU,
                    "gpu",
                    view.snapshot.gpu.map(|g| percent_display(g.scaled)),
                    Panel::Gpu,
                    view.show_gpu && view.snapshot.gpu.is_some(),
                ),
                spark_spec(
                    ID_HOP_FAN,
                    "fan",
                    peak_fan(view).map(|rpm| format!("{rpm} rpm")),
                    Panel::Fans,
                    view.show_fans && view.flags().has_fans,
                ),
            ],
        },
        Band {
            min_height: 5,
            grow_to: grow.then_some(8),
            cells: vec![
                cluster_spec(
                    ID_SUPER_STRIP,
                    "super",
                    Some(strip_value(view, ClusterKind::Super)),
                    s,
                ),
                cluster_spec(
                    ID_PERF_STRIP,
                    "performance",
                    Some(strip_value(view, ClusterKind::Performance)),
                    p,
                ),
                cluster_spec(
                    ID_EFF_STRIP,
                    "efficiency",
                    Some(strip_value(view, ClusterKind::Efficiency)),
                    e,
                ),
            ],
        },
    ]
}

fn gpu_bands(view: &AppView<'_>) -> Vec<Band> {
    let Some(gpu) = view.snapshot.gpu else {
        return Vec::new();
    };
    let temp = gpu.temp_c.or(view.snapshot.sensors.gpu_c);
    vec![
        Band {
            min_height: 5,
            grow_to: None,
            cells: vec![graph_spec(
                ID_GPU_UTIL,
                "gpu",
                Some(ready_pct(view.ready, gpu.scaled)),
                true,
            )],
        },
        Band {
            min_height: 5,
            grow_to: None,
            cells: vec![graph_spec(
                ID_GPU_TEMP,
                "gpu temp",
                temp.map(|c| format!("{c:.0}°")),
                live(temp, view.gpu_temp_history),
            )],
        },
        Band {
            min_height: 3,
            grow_to: None,
            cells: vec![
                spark_spec(
                    ID_HOP_CPU,
                    "cpu",
                    Some(percent_display(view.snapshot.cpu.scaled)),
                    Panel::Cpu,
                    true,
                ),
                spark_spec(
                    ID_HOP_SENS,
                    "sens",
                    view.snapshot
                        .sensors
                        .best_cpu_c()
                        .map(|c| format!("{c:.0}°")),
                    Panel::Fans,
                    view.show_fans && view.flags().has_fans,
                ),
            ],
        },
    ]
}

#[allow(clippy::too_many_lines)]
fn sens_bands(view: &AppView<'_>) -> Vec<Band> {
    let mut fans = Vec::new();
    for (i, fan) in view.snapshot.fans.fans.iter().take(4).enumerate() {
        if fan.rpm == 0 && fan.max_rpm == 0 {
            continue;
        }
        fans.push(graph_spec(
            ID_FAN0.saturating_add(i as u8),
            "fan",
            Some(format!("{} rpm", fan.rpm)),
            true,
        ));
        if let Some(cell) = fans.last_mut() {
            cell.title.label = match i {
                0 => "Fan 1",
                1 => "Fan 2",
                2 => "Fan 3",
                _ => "Fan 4",
            };
        }
    }
    let extras = extra_readings(view);
    vec![
        Band {
            min_height: 5,
            grow_to: None,
            cells: vec![
                graph_spec(
                    ID_SUPER_ZONE,
                    "super zone",
                    zone_value(view, ClusterKind::Super),
                    live(view.snapshot.sensors.s_c, view.s_temp_history),
                ),
                graph_spec(
                    ID_PERF_ZONE,
                    "perf zone",
                    zone_value(view, ClusterKind::Performance),
                    live(view.snapshot.sensors.p_c, view.p_temp_history),
                ),
                graph_spec(
                    ID_EFF_ZONE,
                    "eff zone",
                    zone_value(view, ClusterKind::Efficiency),
                    live(view.snapshot.sensors.e_c, view.e_temp_history),
                ),
                graph_spec(
                    ID_PACKAGE,
                    "package",
                    view.snapshot
                        .sensors
                        .best_cpu_c()
                        .map(|c| format!("{c:.0}°")),
                    live(view.snapshot.sensors.best_cpu_c(), view.cpu_temp_history),
                ),
                graph_spec(
                    ID_GPU_TEMP,
                    "gpu temp",
                    view.snapshot
                        .sensors
                        .gpu_c
                        .or(view.snapshot.gpu.and_then(|g| g.temp_c))
                        .map(|c| format!("{c:.0}°")),
                    live(
                        view.snapshot
                            .sensors
                            .gpu_c
                            .or(view.snapshot.gpu.and_then(|g| g.temp_c)),
                        view.gpu_temp_history,
                    ),
                ),
            ],
        },
        Band {
            min_height: 5,
            grow_to: None,
            cells: fans,
        },
        Band {
            min_height: 3,
            grow_to: None,
            cells: vec![
                spark_spec(
                    ID_HOP_CPU,
                    "cpu",
                    Some(percent_display(view.snapshot.cpu.scaled)),
                    Panel::Cpu,
                    true,
                ),
                spark_spec(
                    ID_HOP_GPU,
                    "gpu",
                    view.snapshot.gpu.map(|g| percent_display(g.scaled)),
                    Panel::Gpu,
                    view.show_gpu && view.snapshot.gpu.is_some(),
                ),
                CellSpec {
                    id: ID_READINGS,
                    kind: CellKind::List,
                    title: CellTitle {
                        label: "readings",
                        value: None,
                        hop: None,
                    },
                    min: (16, 4),
                    weight: 1,
                    present: !extras.is_empty(),
                },
            ],
        },
    ]
}

fn mem_bands(view: &AppView<'_>) -> Vec<Band> {
    let m = &view.snapshot.memory;
    let app = m
        .used_bytes
        .saturating_sub(m.wired_bytes)
        .saturating_sub(m.compressed_bytes);
    let mut parts = Vec::new();
    if m.wired_bytes > 0 {
        parts.push(cluster_spec(
            33,
            "wired",
            Some(bytes_short(m.wired_bytes)),
            true,
        ));
    }
    if m.compressed_bytes > 0 {
        parts.push(cluster_spec(
            34,
            "compressed",
            Some(bytes_short(m.compressed_bytes)),
            true,
        ));
    }
    if app > 0 {
        parts.push(cluster_spec(35, "app", Some(bytes_short(app)), true));
    }
    vec![
        Band {
            min_height: 5,
            grow_to: None,
            cells: vec![graph_spec(
                ID_MEM,
                "memory",
                Some(format!(
                    "{} / {}",
                    bytes_short(m.used_bytes),
                    bytes_short(m.total_bytes)
                )),
                true,
            )],
        },
        Band {
            min_height: 5,
            grow_to: None,
            cells: parts,
        },
        Band {
            min_height: 3,
            grow_to: None,
            cells: vec![spark_spec(
                ID_HOP_PROC,
                "proc",
                None,
                Panel::Processes,
                true,
            )],
        },
    ]
}

fn net_bands(view: &AppView<'_>) -> Vec<Band> {
    vec![
        Band {
            min_height: 5,
            grow_to: None,
            cells: vec![
                graph_spec(
                    ID_NET_DOWN,
                    "down",
                    Some(bits_per_sec(view.snapshot.network.rx_bps)),
                    true,
                ),
                graph_spec(
                    ID_NET_UP,
                    "up",
                    Some(bits_per_sec(view.snapshot.network.tx_bps)),
                    true,
                ),
            ],
        },
        Band {
            min_height: 3,
            grow_to: None,
            cells: vec![spark_spec(
                ID_HOP_DISK,
                "disk",
                Some(bytes_per_sec(
                    view.snapshot
                        .disk
                        .read_bps
                        .saturating_add(view.snapshot.disk.write_bps),
                )),
                Panel::Disk,
                view.show_disk && view.flags().has_disk,
            )],
        },
    ]
}

fn disk_bands(view: &AppView<'_>) -> Vec<Band> {
    vec![
        Band {
            min_height: 5,
            grow_to: None,
            cells: vec![
                graph_spec(
                    ID_DISK_READ,
                    "read",
                    Some(bytes_per_sec(view.snapshot.disk.read_bps)),
                    true,
                ),
                graph_spec(
                    ID_DISK_WRITE,
                    "write",
                    Some(bytes_per_sec(view.snapshot.disk.write_bps)),
                    true,
                ),
            ],
        },
        Band {
            min_height: 4,
            grow_to: None,
            cells: vec![CellSpec {
                id: ID_VOLUMES,
                kind: CellKind::List,
                title: CellTitle {
                    label: "volumes",
                    value: None,
                    hop: None,
                },
                min: (16, 4),
                weight: 1,
                present: !view.snapshot.disk.volumes.is_empty(),
            }],
        },
        Band {
            min_height: 3,
            grow_to: None,
            cells: vec![spark_spec(
                ID_HOP_NET,
                "net",
                None,
                Panel::Net,
                view.show_net,
            )],
        },
    ]
}

#[allow(clippy::too_many_lines)]
fn paint_pack(frame: &mut Frame, placed: &Pack, view: &AppView<'_>, theme: &Theme, _panel: Panel) {
    for cell in &placed.cells {
        match cell.id {
            ID_GPU_UTIL | ID_HOP_GPU => paint_series(
                frame,
                cell,
                view,
                view.gpu_history,
                theme.gpu,
                GraphInk::Load(view.snapshot.thermal),
                Scale::LOAD,
                Axis::Percent,
                theme,
            ),
            ID_CPU | ID_HOP_CPU => paint_series(
                frame,
                cell,
                view,
                view.cpu_history,
                theme.cpu,
                GraphInk::Load(view.snapshot.thermal),
                Scale::LOAD,
                Axis::Percent,
                theme,
            ),
            ID_SUPER_LOAD => paint_series(
                frame,
                cell,
                view,
                view.s_load_history,
                theme.cpu,
                GraphInk::Load(view.snapshot.thermal),
                Scale::LOAD,
                Axis::Percent,
                theme,
            ),
            ID_PERF_LOAD => paint_series(
                frame,
                cell,
                view,
                view.p_load_history,
                theme.cpu,
                GraphInk::Load(view.snapshot.thermal),
                Scale::LOAD,
                Axis::Percent,
                theme,
            ),
            ID_EFF_LOAD => paint_series(
                frame,
                cell,
                view,
                view.e_load_history,
                theme.cpu,
                GraphInk::Load(view.snapshot.thermal),
                Scale::LOAD,
                Axis::Percent,
                theme,
            ),
            ID_SUPER_ZONE => paint_temp(frame, cell, view.s_temp_history, theme),
            ID_PERF_ZONE => paint_temp(frame, cell, view.p_temp_history, theme),
            ID_EFF_ZONE => paint_temp(frame, cell, view.e_temp_history, theme),
            ID_PACKAGE | ID_HOP_SENS | ID_HOP_FAN => {
                paint_temp(frame, cell, view.cpu_temp_history, theme);
            }
            ID_GPU_TEMP => paint_temp(frame, cell, view.gpu_temp_history, theme),
            ID_HOP_PROC => paint_titled_empty(frame, cell, theme),
            ID_HOP_DISK => paint_series(
                frame,
                cell,
                view,
                view.disk_history,
                theme.disk,
                GraphInk::Flat,
                Scale::Auto { floor: 1_024.0 },
                Axis::Bytes,
                theme,
            ),
            ID_SUPER_STRIP => paint_strip(frame, cell, ClusterKind::Super, view, theme),
            ID_PERF_STRIP => paint_strip(frame, cell, ClusterKind::Performance, view, theme),
            ID_EFF_STRIP => paint_strip(frame, cell, ClusterKind::Efficiency, view, theme),
            ID_MEM => paint_series(
                frame,
                cell,
                view,
                view.mem_history,
                theme.mem,
                GraphInk::Load(view.snapshot.thermal),
                Scale::Fixed(1.0),
                Axis::Percent,
                theme,
            ),
            ID_NET_DOWN | ID_HOP_NET => paint_series(
                frame,
                cell,
                view,
                view.net_rx_history,
                theme.net,
                GraphInk::Flat,
                Scale::Auto { floor: 8_000.0 },
                Axis::Bits,
                theme,
            ),
            ID_NET_UP => paint_series(
                frame,
                cell,
                view,
                view.net_tx_history,
                theme.net,
                GraphInk::Flat,
                Scale::Auto { floor: 8_000.0 },
                Axis::Bits,
                theme,
            ),
            ID_DISK_READ => paint_series(
                frame,
                cell,
                view,
                view.disk_read_history,
                theme.disk,
                GraphInk::Flat,
                Scale::Auto { floor: 1_024.0 },
                Axis::Bytes,
                theme,
            ),
            ID_DISK_WRITE => paint_series(
                frame,
                cell,
                view,
                view.disk_write_history,
                theme.disk,
                GraphInk::Flat,
                Scale::Auto { floor: 1_024.0 },
                Axis::Bytes,
                theme,
            ),
            ID_VOLUMES => paint_volumes(frame, cell.rect, view, theme),
            ID_READINGS => paint_readings(frame, cell.rect, &extra_readings(view), theme),
            id if (ID_FAN0..ID_FAN0 + 4).contains(&id) => {
                let i = usize::from(id - ID_FAN0);
                if let Some(fan) = view.snapshot.fans.fans.get(i) {
                    paint_fan(frame, cell, fan, view.fan_histories.get(i), theme);
                }
            }
            33..=35 => paint_mem_part(frame, cell, view, theme),
            _ => {}
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn paint_series(
    frame: &mut Frame,
    cell: &crate::widgets::grid::Placed,
    view: &AppView<'_>,
    history: &History,
    accent: ratatui::style::Color,
    ink: GraphInk,
    scale: Scale,
    axis: Axis,
    theme: &Theme,
) {
    let (label, value) = series_title(cell.id, view);
    let inner = cell_titled(
        frame,
        cell.rect,
        label,
        value.as_deref(),
        cell.hop.is_some(),
        theme,
    );
    render_scaled_graph(
        frame,
        inner,
        Graph {
            history,
            accent,
            theme,
            scale,
            axis,
            ink,
        },
    );
}

fn series_title(id: u8, view: &AppView<'_>) -> (&'static str, Option<String>) {
    let (label, _) = label_for(id);
    let value = match id {
        ID_CPU => Some(ready_pct(view.ready, view.snapshot.cpu.scaled)),
        ID_GPU_UTIL => view.snapshot.gpu.map(|g| ready_pct(view.ready, g.scaled)),
        ID_SUPER_LOAD => Some(percent_display(cluster_load(view, ClusterKind::Super))),
        ID_PERF_LOAD => Some(percent_display(cluster_load(
            view,
            ClusterKind::Performance,
        ))),
        ID_EFF_LOAD => Some(percent_display(cluster_load(view, ClusterKind::Efficiency))),
        ID_HOP_CPU => Some(percent_display(view.snapshot.cpu.scaled)),
        ID_HOP_GPU => view.snapshot.gpu.map(|g| percent_display(g.scaled)),
        ID_NET_DOWN => Some(bits_per_sec(view.snapshot.network.rx_bps)),
        ID_NET_UP => Some(bits_per_sec(view.snapshot.network.tx_bps)),
        ID_DISK_READ | ID_HOP_DISK => Some(bytes_per_sec(view.snapshot.disk.read_bps)),
        ID_DISK_WRITE => Some(bytes_per_sec(view.snapshot.disk.write_bps)),
        ID_MEM => Some(format!(
            "{} / {}",
            bytes_short(view.snapshot.memory.used_bytes),
            bytes_short(view.snapshot.memory.total_bytes)
        )),
        _ => None,
    };
    (label, value)
}

fn label_for(id: u8) -> (&'static str, Option<String>) {
    match id {
        ID_CPU | ID_HOP_CPU => ("cpu", None),
        ID_GPU_UTIL | ID_HOP_GPU => ("gpu", None),
        ID_SUPER_LOAD | ID_SUPER_STRIP => ("super", None),
        ID_PERF_LOAD | ID_PERF_STRIP => ("performance", None),
        ID_EFF_LOAD | ID_EFF_STRIP => ("efficiency", None),
        ID_SUPER_ZONE => ("super zone", None),
        ID_PERF_ZONE => ("perf zone", None),
        ID_EFF_ZONE => ("eff zone", None),
        ID_PACKAGE => ("package", None),
        ID_GPU_TEMP => ("gpu temp", None),
        ID_HOP_SENS => ("sens", None),
        ID_HOP_FAN => ("fan", None),
        ID_HOP_PROC => ("proc", None),
        ID_HOP_DISK => ("disk", None),
        ID_HOP_NET => ("net", None),
        ID_MEM => ("memory", None),
        ID_NET_DOWN => ("down", None),
        ID_NET_UP => ("up", None),
        ID_DISK_READ => ("read", None),
        ID_DISK_WRITE => ("write", None),
        ID_VOLUMES => ("volumes", None),
        ID_READINGS => ("readings", None),
        ID_FAN0 => ("Fan 1", None),
        x if x == ID_FAN0 + 1 => ("Fan 2", None),
        x if x == ID_FAN0 + 2 => ("Fan 3", None),
        x if x == ID_FAN0 + 3 => ("Fan 4", None),
        33 => ("wired", None),
        34 => ("compressed", None),
        35 => ("app", None),
        _ => ("", None),
    }
}

fn paint_temp(
    frame: &mut Frame,
    cell: &crate::widgets::grid::Placed,
    history: &History,
    theme: &Theme,
) {
    let (label, _) = label_for(cell.id);
    let value = history.last().map(|c| format!("{c:.0}°"));
    let inner = cell_titled(
        frame,
        cell.rect,
        label,
        value.as_deref(),
        cell.hop.is_some(),
        theme,
    );
    render_scaled_graph(
        frame,
        inner,
        Graph {
            history,
            accent: theme.temp,
            theme,
            scale: Scale::TEMP,
            axis: Axis::Celsius,
            ink: GraphInk::Flat,
        },
    );
}

fn paint_titled_empty(frame: &mut Frame, cell: &crate::widgets::grid::Placed, theme: &Theme) {
    let (label, _) = label_for(cell.id);
    let _ = cell_titled(frame, cell.rect, label, None, cell.hop.is_some(), theme);
}

fn paint_strip(
    frame: &mut Frame,
    cell: &crate::widgets::grid::Placed,
    kind: ClusterKind,
    view: &AppView<'_>,
    theme: &Theme,
) {
    let load = cluster_load(view, kind);
    let mut value = percent_display(load);
    if let Some(c) = view.snapshot.sensors.zone_temp(kind) {
        value = format!("{value}  {c:.0}°");
    }
    let inner = cell_titled(frame, cell.rect, kind.word(), Some(&value), false, theme);
    if inner.height == 0 {
        return;
    }
    let bar = Rect {
        x: inner.x,
        y: inner.y,
        width: inner.width,
        height: 1,
    };
    render_fill_bar(frame, bar, load, theme.cpu);
    let mosaic = view.show_cores && cell.rect.height >= 8;
    if !mosaic {
        return;
    }
    let grid = Rect {
        x: inner.x,
        y: inner.y.saturating_add(1),
        width: inner.width,
        height: inner.height.saturating_sub(1),
    };
    let mut cores: Vec<CoreSample> = view
        .snapshot
        .cpu
        .cores
        .iter()
        .copied()
        .filter(|core| core.kind == kind)
        .collect();
    cores.sort_by_key(|core| core.index);
    render_core_grid(frame, grid, &cores, theme);
}

fn render_core_grid(frame: &mut Frame, area: Rect, cores: &[CoreSample], theme: &Theme) {
    if area.height == 0 || cores.is_empty() {
        return;
    }
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

fn paint_fan(
    frame: &mut Frame,
    cell: &crate::widgets::grid::Placed,
    fan: &plottypus_core::FanMetric,
    history: Option<&History>,
    theme: &Theme,
) {
    let name = if fan.name.is_empty() {
        "fan"
    } else {
        fan.name.as_str()
    };
    let value = format!("{} rpm", fan.rpm);
    let inner = cell_titled(frame, cell.rect, name, Some(&value), false, theme);
    if let Some(history) = history.filter(|h| !h.is_empty()) {
        render_scaled_graph(
            frame,
            inner,
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
    if inner.height > 0 {
        render_fill_bar(
            frame,
            Rect {
                x: inner.x,
                y: inner.y.saturating_add(inner.height.saturating_sub(1)),
                width: inner.width,
                height: 1,
            },
            fan.ratio(),
            theme.fan,
        );
    }
}

fn paint_volumes(frame: &mut Frame, area: Rect, view: &AppView<'_>, theme: &Theme) {
    let inner = cell_titled(frame, area, "volumes", None, false, theme);
    let mut y = inner.y;
    for vol in &view.snapshot.disk.volumes {
        if y >= inner.y.saturating_add(inner.height) {
            break;
        }
        let line = Rect {
            x: inner.x,
            y,
            width: inner.width,
            height: 1,
        };
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(vol.name.clone(), theme.fg()),
                Span::styled(
                    format!(
                        "  {} / {}",
                        bytes_short(vol.used_bytes),
                        bytes_short(vol.total_bytes)
                    ),
                    theme.dim(),
                ),
            ])),
            line,
        );
        y = y.saturating_add(1);
        if y < inner.y.saturating_add(inner.height) {
            render_fill_bar(
                frame,
                Rect {
                    x: inner.x,
                    y,
                    width: inner.width,
                    height: 1,
                },
                vol.ratio(),
                theme.disk,
            );
            y = y.saturating_add(1);
        }
    }
}

fn paint_readings(frame: &mut Frame, area: Rect, extras: &[(String, f32)], theme: &Theme) {
    let inner = cell_titled(frame, area, "readings", None, false, theme);
    let lines: Vec<Line<'static>> = extras
        .iter()
        .take(usize::from(inner.height))
        .map(|(name, c)| {
            Line::from(Span::styled(
                format!(" {name}  {c:.0}°"),
                Style::default().fg(theme.temp_color(*c)),
            ))
        })
        .collect();
    frame.render_widget(Paragraph::new(lines), inner);
}

fn paint_mem_part(
    frame: &mut Frame,
    cell: &crate::widgets::grid::Placed,
    view: &AppView<'_>,
    theme: &Theme,
) {
    let m = &view.snapshot.memory;
    let (label, bytes) = match cell.id {
        33 => ("wired", m.wired_bytes),
        34 => ("compressed", m.compressed_bytes),
        _ => (
            "app",
            m.used_bytes
                .saturating_sub(m.wired_bytes)
                .saturating_sub(m.compressed_bytes),
        ),
    };
    let inner = cell_titled(
        frame,
        cell.rect,
        label,
        Some(&bytes_short(bytes)),
        false,
        theme,
    );
    let total = m.total_bytes;
    let ratio = if total == 0 {
        0.0
    } else {
        bytes as f32 / total as f32
    };
    if inner.height > 0 {
        render_fill_bar(
            frame,
            Rect {
                x: inner.x,
                y: inner.y.saturating_add(inner.height.saturating_sub(1)),
                width: inner.width,
                height: 1,
            },
            ratio,
            theme.mem,
        );
    }
}

fn outer_title(panel: Panel, view: &AppView<'_>, theme: &Theme) -> Line<'static> {
    let mut spans = panel_title(panel.label(), theme).spans;
    match panel {
        Panel::Cpu => {
            push_token(
                &mut spans,
                ready_pct(view.ready, view.snapshot.cpu.scaled),
                theme.title(),
            );
            if let Some(w) = view.snapshot.cpu.watts.filter(|w| *w > 0.0) {
                push_token(&mut spans, watts_display(w), theme.cpu());
            }
            if let Some(c) = view
                .snapshot
                .cpu
                .temp_c
                .or(view.snapshot.sensors.best_cpu_c())
            {
                push_token(&mut spans, format!("{c:.0}°"), theme.temp());
            }
            if !view.snapshot.thermal.is_nominal() {
                push_token(
                    &mut spans,
                    thermal_word(view.snapshot.thermal).to_owned(),
                    theme.thermal(view.snapshot.thermal),
                );
            }
            if view.ready && (view.snapshot.cpu.scaled - view.snapshot.cpu.active).abs() > 0.01 {
                push_token(
                    &mut spans,
                    format!("busy {}", percent_display(view.snapshot.cpu.active)),
                    theme.dim(),
                );
            }
        }
        Panel::Gpu => {
            if let Some(gpu) = view.snapshot.gpu {
                push_token(&mut spans, ready_pct(view.ready, gpu.scaled), theme.title());
                if let Some(w) = gpu.watts.filter(|w| *w > 0.0) {
                    push_token(&mut spans, watts_display(w), theme.gpu());
                }
                if let Some(c) = gpu.temp_c.or(view.snapshot.sensors.gpu_c) {
                    push_token(&mut spans, format!("{c:.0}°"), theme.temp());
                }
            }
        }
        Panel::Fans => {
            if let Some(c) = view.snapshot.sensors.best_cpu_c() {
                push_token(&mut spans, format!("{c:.0}°"), theme.temp());
            }
            if let Some(rpm) = peak_fan(view) {
                push_token(&mut spans, format!("{rpm} rpm"), theme.title());
            }
        }
        Panel::Mem => {
            let m = &view.snapshot.memory;
            push_token(&mut spans, bytes_short(m.used_bytes), theme.title());
            spans.push(Span::styled(" / ", theme.dim()));
            spans.push(Span::styled(bytes_short(m.total_bytes), theme.title()));
            if m.pressure != Pressure::Nominal {
                push_token(&mut spans, String::from("●"), theme.pressure(m.pressure));
            }
        }
        Panel::Net => {
            push_token(
                &mut spans,
                format!("↓ {}", bits_per_sec(view.snapshot.network.rx_bps)),
                theme.net(),
            );
            push_token(
                &mut spans,
                format!("↑ {}", bits_per_sec(view.snapshot.network.tx_bps)),
                theme.net(),
            );
        }
        Panel::Disk => {
            push_token(
                &mut spans,
                format!("↓ {}", bytes_per_sec(view.snapshot.disk.read_bps)),
                theme.disk(),
            );
            push_token(
                &mut spans,
                format!("↑ {}", bytes_per_sec(view.snapshot.disk.write_bps)),
                theme.disk(),
            );
        }
        Panel::Processes => {}
    }
    Line::from(spans)
}

fn has_cluster(view: &AppView<'_>, kind: ClusterKind) -> bool {
    let named = match kind {
        ClusterKind::Efficiency => view.snapshot.cpu.e_cluster.is_some(),
        ClusterKind::Performance => view.snapshot.cpu.p_cluster.is_some(),
        ClusterKind::Super => view.snapshot.cpu.s_cluster.is_some(),
    };
    named || view.snapshot.cpu.cores.iter().any(|c| c.kind == kind)
}

fn cluster_load(view: &AppView<'_>, kind: ClusterKind) -> f32 {
    let from = match kind {
        ClusterKind::Efficiency => view.snapshot.cpu.e_cluster.as_ref().map(|c| c.scaled),
        ClusterKind::Performance => view.snapshot.cpu.p_cluster.as_ref().map(|c| c.scaled),
        ClusterKind::Super => view.snapshot.cpu.s_cluster.as_ref().map(|c| c.scaled),
    };
    if let Some(load) = from {
        return load;
    }
    let mut sum = 0.0;
    let mut n = 0_u32;
    for core in &view.snapshot.cpu.cores {
        if core.kind == kind {
            sum += core.scaled;
            n += 1;
        }
    }
    if n == 0 { 0.0 } else { sum / n as f32 }
}

fn zone_value(view: &AppView<'_>, kind: ClusterKind) -> Option<String> {
    view.snapshot
        .sensors
        .zone_temp(kind)
        .or_else(|| view.zone_temp_history(kind).last())
        .map(|c| format!("{c:.0}°"))
}

fn strip_value(view: &AppView<'_>, kind: ClusterKind) -> String {
    percent_display(cluster_load(view, kind))
}

fn peak_fan(view: &AppView<'_>) -> Option<u16> {
    view.snapshot
        .fans
        .fans
        .iter()
        .map(|f| f.rpm)
        .max()
        .filter(|r| *r > 0)
}

fn extra_readings(view: &AppView<'_>) -> Vec<(String, f32)> {
    view.snapshot
        .sensors
        .readings
        .iter()
        .filter(|r| {
            !matches!(
                r.name.as_str(),
                "cpu" | "gpu" | "efficiency" | "performance" | "super"
            )
        })
        .map(|r| (r.name.clone(), r.celsius))
        .collect()
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
        Thermal::Nominal => "",
        Thermal::Fair => "fair",
        Thermal::Serious => "serious",
        Thermal::Critical => "critical",
    }
}

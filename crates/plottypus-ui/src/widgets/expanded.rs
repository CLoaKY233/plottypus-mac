use plottypus_core::{
    ClusterKind, CoreSample, History, Pressure, Scale, bits_per_sec, bytes_per_sec, bytes_short,
    percent_display, ready_pct, watts_display,
};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
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
    let budget = meta_budget(panel, view);
    let mins = panel_mins(panel, view);
    let plan = split_meta(inner, budget, mins);
    let placed = pack(plan.main, &bands_for(panel, view));
    paint_pack(frame, &placed, view, theme, panel);
    if let Some(meta) = plan.meta {
        paint_meta(frame, meta, plan.side, panel, view, theme);
    }
}

#[must_use]
pub fn hop_hit(area: Rect, view: &AppView<'_>, col: u16, row: u16) -> Option<Panel> {
    let panel = view.expanded?;
    if panel == Panel::Processes {
        return None;
    }
    let inner = inset(area);
    let budget = meta_budget(panel, view);
    let plan = split_meta(inner, budget, panel_mins(panel, view));
    if let Some(meta) = plan.meta
        && let Some(hit) = meta_hop_hit(meta, plan.side, panel, view, col, row)
    {
        return Some(hit);
    }
    pack(plan.main, &bands_for(panel, view)).hop_at(col, row)
}

fn inset(area: Rect) -> Rect {
    Rect {
        x: area.x.saturating_add(1),
        y: area.y.saturating_add(1),
        width: area.width.saturating_sub(2),
        height: area.height.saturating_sub(2),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HopStyle {
    Absent,
    Spark,
    LabelOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct MetaBudget {
    hops: HopStyle,
    mosaic: bool,
    identity: bool,
    extras: bool,
    volumes: bool,
}

impl MetaBudget {
    fn is_empty(self) -> bool {
        self.hops == HopStyle::Absent
            && !self.mosaic
            && !self.identity
            && !self.extras
            && !self.volumes
    }

    fn hops_h(self) -> u16 {
        match self.hops {
            HopStyle::Spark => 3,
            HopStyle::LabelOnly => 1,
            HopStyle::Absent => 0,
        }
    }
}

const META_COL_MIN_WIDTH: u16 = 100;
const META_COL_WIDTH: u16 = 26;
const META_COL_GAP: u16 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MetaSide {
    Left,
    Bottom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct MetaPlan {
    main: Rect,
    meta: Option<Rect>,
    side: MetaSide,
}

fn split_meta(inner: Rect, budget: MetaBudget, panel_mins: u16) -> MetaPlan {
    if inner.width == 0 || inner.height == 0 || budget.is_empty() {
        return MetaPlan {
            main: inner,
            meta: None,
            side: MetaSide::Bottom,
        };
    }
    let hops_h = budget.hops_h();
    let col_w = META_COL_WIDTH.saturating_add(META_COL_GAP);
    if inner.width >= META_COL_MIN_WIDTH
        && inner.width >= col_w.saturating_add(16)
        && (budget.hops == HopStyle::Spark || budget.mosaic)
        && inner.height >= hops_h.max(1)
    {
        return MetaPlan {
            main: Rect {
                x: inner.x.saturating_add(col_w),
                width: inner.width.saturating_sub(col_w),
                ..inner
            },
            meta: Some(Rect {
                width: META_COL_WIDTH,
                height: inner.height,
                ..inner
            }),
            side: MetaSide::Left,
        };
    }

    let mut height = 0_u16;
    if budget.mosaic && inner.height >= panel_mins.saturating_add(hops_h).saturating_add(5) {
        height = height.saturating_add(5);
    }
    if budget.hops == HopStyle::Spark && inner.height >= panel_mins.saturating_add(3) {
        height = height.saturating_add(3);
    } else if budget.volumes
        && budget.hops == HopStyle::Absent
        && !budget.mosaic
        && inner.height >= panel_mins.saturating_add(4)
    {
        height = height.saturating_add(4);
    } else if budget.hops == HopStyle::LabelOnly
        && !budget.mosaic
        && !budget.volumes
        && inner.height >= panel_mins.saturating_add(1)
    {
        height = height.saturating_add(1);
    }
    let max_steal = inner.height.saturating_sub(panel_mins);
    height = height.min(max_steal);
    if height == 0 {
        return MetaPlan {
            main: inner,
            meta: None,
            side: MetaSide::Bottom,
        };
    }
    let main_h = inner.height.saturating_sub(height);
    MetaPlan {
        main: Rect {
            height: main_h,
            ..inner
        },
        meta: Some(Rect {
            x: inner.x,
            y: inner.y.saturating_add(main_h),
            width: inner.width,
            height,
        }),
        side: MetaSide::Bottom,
    }
}

fn panel_mins(panel: Panel, view: &AppView<'_>) -> u16 {
    match panel {
        Panel::Cpu => {
            let mut n = 4_u16;
            if any_cpu_zone(view) {
                n = n.saturating_add(5);
            }
            n
        }
        Panel::Gpu => {
            let mut n = 5_u16;
            if gpu_temp_live(view) {
                n = n.saturating_add(5);
            }
            n
        }
        Panel::Fans => {
            let mut n = 0_u16;
            if any_sens_graph(view) {
                n = n.saturating_add(5);
            }
            if view.snapshot.fans.is_present() {
                n = n.saturating_add(5);
            }
            n
        }
        Panel::Mem | Panel::Net | Panel::Disk => 5,
        Panel::Processes => 0,
    }
}

fn any_cpu_zone(view: &AppView<'_>) -> bool {
    live(view.snapshot.sensors.s_c, view.s_temp_history)
        || live(view.snapshot.sensors.p_c, view.p_temp_history)
        || live(view.snapshot.sensors.e_c, view.e_temp_history)
}

fn any_sens_graph(view: &AppView<'_>) -> bool {
    any_cpu_zone(view)
        || live(view.snapshot.sensors.best_cpu_c(), view.cpu_temp_history)
        || gpu_temp_live(view)
}

fn gpu_temp_live(view: &AppView<'_>) -> bool {
    live(
        view.snapshot
            .gpu
            .and_then(|g| g.temp_c)
            .or(view.snapshot.sensors.gpu_c),
        view.gpu_temp_history,
    )
}

fn meta_budget(panel: Panel, view: &AppView<'_>) -> MetaBudget {
    match panel {
        Panel::Cpu => MetaBudget {
            hops: if cpu_has_spark_hops(view) {
                HopStyle::Spark
            } else {
                HopStyle::Absent
            },
            mosaic: view.show_cores && has_any_cluster(view),
            identity: false,
            extras: false,
            volumes: false,
        },
        Panel::Gpu | Panel::Fans => MetaBudget {
            hops: HopStyle::Spark,
            mosaic: false,
            identity: false,
            extras: false,
            volumes: false,
        },
        Panel::Mem => MetaBudget {
            hops: HopStyle::LabelOnly,
            mosaic: false,
            identity: false,
            extras: view.snapshot.memory.swap_used_bytes > 0
                || view.snapshot.memory.cache_bytes > 0,
            volumes: false,
        },
        Panel::Net => MetaBudget {
            hops: if view.show_disk && view.flags().has_disk {
                HopStyle::Spark
            } else {
                HopStyle::Absent
            },
            mosaic: false,
            identity: false,
            extras: false,
            volumes: false,
        },
        Panel::Disk => MetaBudget {
            hops: if view.show_net {
                HopStyle::Spark
            } else {
                HopStyle::Absent
            },
            mosaic: false,
            identity: false,
            extras: false,
            volumes: !view.snapshot.disk.volumes.is_empty(),
        },
        Panel::Processes => MetaBudget {
            hops: HopStyle::Absent,
            mosaic: false,
            identity: false,
            extras: false,
            volumes: false,
        },
    }
}

fn cpu_has_spark_hops(view: &AppView<'_>) -> bool {
    (view.show_gpu && view.snapshot.gpu.is_some())
        || (view.show_fans && view.snapshot.fans.is_present())
}

fn has_any_cluster(view: &AppView<'_>) -> bool {
    has_cluster(view, ClusterKind::Super)
        || has_cluster(view, ClusterKind::Performance)
        || has_cluster(view, ClusterKind::Efficiency)
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

fn usage_spec(id: u8, label: &'static str, value: Option<String>, present: bool) -> CellSpec {
    CellSpec {
        min: (16, 4),
        ..graph_spec(id, label, value, present)
    }
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
    let fallback = !s && !p && !e;
    vec![
        Band {
            take_leftover: true,
            ..Band::new(
                4,
                vec![
                    usage_spec(
                        ID_CPU,
                        "cpu",
                        Some(ready_pct(view.ready, view.snapshot.cpu.scaled)),
                        fallback,
                    ),
                    usage_spec(
                        ID_SUPER_LOAD,
                        "super",
                        Some(percent_display(cluster_load(view, ClusterKind::Super))),
                        s,
                    ),
                    usage_spec(
                        ID_PERF_LOAD,
                        "performance",
                        Some(percent_display(cluster_load(
                            view,
                            ClusterKind::Performance,
                        ))),
                        p,
                    ),
                    usage_spec(
                        ID_EFF_LOAD,
                        "efficiency",
                        Some(percent_display(cluster_load(view, ClusterKind::Efficiency))),
                        e,
                    ),
                ],
            )
        },
        Band {
            take_leftover: true,
            ..Band::new(
                5,
                vec![
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
                        !any_cpu_zone(view)
                            && live(
                                view.snapshot
                                    .cpu
                                    .temp_c
                                    .or(view.snapshot.sensors.best_cpu_c()),
                                view.cpu_temp_history,
                            ),
                    ),
                ],
            )
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
            take_leftover: true,
            ..Band::new(
                5,
                vec![graph_spec(
                    ID_GPU_UTIL,
                    "gpu",
                    Some(ready_pct(view.ready, gpu.scaled)),
                    true,
                )],
            )
        },
        Band {
            take_leftover: true,
            ..Band::new(
                5,
                vec![graph_spec(
                    ID_GPU_TEMP,
                    "gpu temp",
                    temp.map(|c| format!("{c:.0}°")),
                    live(temp, view.gpu_temp_history),
                )],
            )
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
    vec![
        Band {
            take_leftover: true,
            ..Band::new(
                5,
                vec![
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
            )
        },
        Band {
            take_leftover: true,
            ..Band::new(5, fans)
        },
        Band::new(
            3,
            vec![CellSpec {
                id: ID_READINGS,
                kind: CellKind::List,
                title: CellTitle {
                    label: "readings",
                    value: None,
                    hop: None,
                },
                min: (16, 3),
                weight: 1,
                present: !extra_readings(view).is_empty(),
            }],
        ),
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
            take_leftover: true,
            ..Band::new(
                5,
                vec![graph_spec(
                    ID_MEM,
                    "memory",
                    Some(format!(
                        "{} / {}",
                        bytes_short(m.used_bytes),
                        bytes_short(m.total_bytes)
                    )),
                    true,
                )],
            )
        },
        Band::new(5, parts),
    ]
}

fn net_bands(view: &AppView<'_>) -> Vec<Band> {
    vec![Band {
        take_leftover: true,
        ..Band::new(
            5,
            vec![
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
        )
    }]
}

fn disk_bands(view: &AppView<'_>) -> Vec<Band> {
    vec![
        Band {
            take_leftover: true,
            ..Band::new(
                5,
                vec![
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
            )
        },
        Band::new(
            4,
            vec![CellSpec {
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
        ),
    ]
}

fn hop_targets(panel: Panel, view: &AppView<'_>) -> Vec<(u8, Panel)> {
    match panel {
        Panel::Cpu => {
            let mut hops = Vec::new();
            if view.show_gpu && view.snapshot.gpu.is_some() {
                hops.push((ID_HOP_GPU, Panel::Gpu));
            }
            if view.show_fans && view.snapshot.fans.is_present() {
                hops.push((ID_HOP_FAN, Panel::Fans));
            }
            hops
        }
        Panel::Gpu => {
            let mut hops = vec![(ID_HOP_CPU, Panel::Cpu)];
            if view.show_fans && view.flags().has_fans {
                hops.push((ID_HOP_SENS, Panel::Fans));
            }
            hops
        }
        Panel::Fans => {
            let mut hops = vec![(ID_HOP_CPU, Panel::Cpu)];
            if view.show_gpu && view.snapshot.gpu.is_some() {
                hops.push((ID_HOP_GPU, Panel::Gpu));
            }
            hops
        }
        Panel::Net if view.show_disk && view.flags().has_disk => {
            vec![(ID_HOP_DISK, Panel::Disk)]
        }
        Panel::Disk if view.show_net => vec![(ID_HOP_NET, Panel::Net)],
        _ => Vec::new(),
    }
}

fn paint_meta(
    frame: &mut Frame,
    area: Rect,
    side: MetaSide,
    panel: Panel,
    view: &AppView<'_>,
    theme: &Theme,
) {
    match side {
        MetaSide::Left => paint_meta_left(frame, area, panel, view, theme),
        MetaSide::Bottom => paint_meta_bottom(frame, area, panel, view, theme),
    }
}

fn paint_meta_bottom(
    frame: &mut Frame,
    area: Rect,
    panel: Panel,
    view: &AppView<'_>,
    theme: &Theme,
) {
    if panel == Panel::Mem {
        paint_label_hop(frame, area, "proc", Panel::Processes, theme);
        return;
    }
    if panel == Panel::Disk && hop_targets(panel, view).is_empty() {
        paint_volumes(frame, area, view, theme);
        return;
    }
    let hops = hop_targets(panel, view);
    let hop_h = if hops.is_empty() {
        0
    } else {
        3.min(area.height)
    };
    if hop_h > 0 {
        paint_hop_row(
            frame,
            Rect {
                height: hop_h,
                ..area
            },
            &hops,
            view,
            theme,
        );
    }
    if view.show_cores && panel == Panel::Cpu && area.height > hop_h {
        paint_cores(
            frame,
            Rect {
                y: area.y.saturating_add(hop_h),
                height: area.height.saturating_sub(hop_h),
                ..area
            },
            view,
            theme,
        );
    }
}

struct LeftSlots {
    identity: u16,
    hop_rects: Vec<(u8, Panel, Rect)>,
    cores: Option<Rect>,
}

fn left_slots(area: Rect, panel: Panel, view: &AppView<'_>) -> LeftSlots {
    let hops = hop_targets(panel, view);
    let ident_n = u16::try_from(identity_lines(panel, view).len()).unwrap_or(0);
    let hop_n = u16::try_from(hops.len()).unwrap_or(0);
    let hop_min = 3u16.saturating_mul(hop_n);
    let cores_want = if view.show_cores && panel == Panel::Cpu {
        u16::try_from(view.snapshot.cpu.cores.len()).unwrap_or(0)
    } else {
        0
    };
    let cores_min = u16::from(cores_want > 0);
    let keep_after_ident = hop_min.saturating_add(cores_min);
    let ident = ident_n.min(area.height.saturating_sub(keep_after_ident));
    let rest = area.height.saturating_sub(ident);
    let cores_h = cores_want.min(rest.saturating_sub(hop_min));
    let hop_space = rest.saturating_sub(cores_h);
    let mut hop_rects = Vec::new();
    let mut y = area.y.saturating_add(ident);
    if hop_n > 0 && hop_space >= 3 {
        let base = hop_space / hop_n;
        let extra = hop_space % hop_n;
        for (i, (id, hop)) in hops.into_iter().enumerate() {
            let h = base.saturating_add(u16::from(i < extra as usize)).max(3);
            let h = h.min(area.y.saturating_add(area.height).saturating_sub(y));
            if h < 3 {
                break;
            }
            hop_rects.push((
                id,
                hop,
                Rect {
                    x: area.x,
                    y,
                    width: area.width,
                    height: h,
                },
            ));
            y = y.saturating_add(h);
        }
    }
    let cores = if cores_h > 0 {
        Some(Rect {
            x: area.x,
            y,
            width: area.width,
            height: cores_h,
        })
    } else {
        None
    };
    LeftSlots {
        identity: ident,
        hop_rects,
        cores,
    }
}

fn paint_meta_left(frame: &mut Frame, area: Rect, panel: Panel, view: &AppView<'_>, theme: &Theme) {
    let slots = left_slots(area, panel, view);
    for (i, line) in identity_lines(panel, view)
        .into_iter()
        .take(usize::from(slots.identity))
        .enumerate()
    {
        let y = area.y.saturating_add(u16::try_from(i).unwrap_or(0));
        paint_dim_line(frame, area.x, y, area.width, line, theme);
    }
    for (id, hop, rect) in slots.hop_rects {
        paint_one_hop(frame, rect, id, hop, view, theme);
    }
    if let Some(cores) = slots.cores {
        paint_cores(frame, cores, view, theme);
    }
}

fn paint_dim_line(frame: &mut Frame, x: u16, y: u16, width: u16, text: String, theme: &Theme) {
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(text, theme.dim()))),
        Rect {
            x,
            y,
            width,
            height: 1,
        },
    );
}

fn identity_lines(panel: Panel, view: &AppView<'_>) -> Vec<String> {
    let mut lines = Vec::new();
    if !view.snapshot.soc.name.is_empty() && view.snapshot.soc.name != "unknown" {
        lines.push(view.snapshot.soc.name.clone());
    }
    match panel {
        Panel::Cpu => {
            let e = view.snapshot.soc.e_cores;
            let p = view.snapshot.soc.p_cores;
            let s = view.snapshot.soc.s_cores;
            if e > 0 || p > 0 || s > 0 {
                let mut parts = Vec::new();
                if e > 0 {
                    parts.push(format!("{e}E"));
                }
                if p > 0 {
                    parts.push(format!("{p}P"));
                }
                if s > 0 {
                    parts.push(format!("{s}S"));
                }
                lines.push(parts.join(" + "));
            }
        }
        Panel::Gpu => {
            if let Some(gpu) = view.snapshot.gpu {
                if let Some(mhz) = gpu.freq_mhz.filter(|mhz| *mhz > 0) {
                    if mhz >= 1000 {
                        lines.push(format!("{:.1}GHz", f64::from(mhz) / 1000.0));
                    } else {
                        lines.push(format!("{mhz}MHz"));
                    }
                }
                if let Some(watts) = gpu.ane_watts.filter(|w| *w > 0.0) {
                    lines.push(format!("ane {}", watts_display(watts)));
                }
            }
            if view.snapshot.soc.gpu_cores > 0 {
                lines.push(format!("{}c", view.snapshot.soc.gpu_cores));
            }
        }
        _ => {}
    }
    lines
}

fn hop_row_rects(area: Rect, n: usize) -> Vec<Rect> {
    if n == 0 || area.width == 0 {
        return Vec::new();
    }
    let constraints: Vec<Constraint> = (0..n).map(|_| Constraint::Fill(1)).collect();
    Layout::horizontal(constraints).split(area).to_vec()
}

fn hop_row_hit(area: Rect, hops: &[(u8, Panel)], col: u16, row: u16) -> Option<Panel> {
    if row < area.y || row >= area.y.saturating_add(area.height) {
        return None;
    }
    hop_row_rects(area, hops.len())
        .into_iter()
        .zip(hops.iter())
        .find(|(rect, _)| rect_contains(*rect, col, row))
        .map(|(_, (_, panel))| *panel)
}

fn mem_label_hit(area: Rect, col: u16, row: u16) -> Option<Panel> {
    let line = Rect { height: 1, ..area };
    if rect_contains(line, col, row) {
        Some(Panel::Processes)
    } else {
        None
    }
}

fn paint_hop_row(
    frame: &mut Frame,
    area: Rect,
    hops: &[(u8, Panel)],
    view: &AppView<'_>,
    theme: &Theme,
) {
    for ((id, hop), col) in hops.iter().zip(hop_row_rects(area, hops.len())) {
        paint_one_hop(frame, col, *id, *hop, view, theme);
    }
}

fn paint_one_hop(
    frame: &mut Frame,
    area: Rect,
    id: u8,
    hop: Panel,
    view: &AppView<'_>,
    theme: &Theme,
) {
    let cell = crate::widgets::grid::Placed {
        id,
        kind: CellKind::Spark,
        rect: area,
        hop: Some(hop),
    };
    match id {
        ID_HOP_FAN => paint_fan_hop(frame, &cell, view, theme),
        ID_HOP_CPU => paint_series(
            frame,
            &cell,
            view,
            view.cpu_history,
            theme.cpu,
            GraphInk::Load(view.snapshot.thermal),
            Scale::LOAD,
            Axis::Percent,
            theme,
        ),
        ID_HOP_GPU => paint_series(
            frame,
            &cell,
            view,
            view.gpu_history,
            theme.gpu,
            GraphInk::Load(view.snapshot.thermal),
            Scale::LOAD,
            Axis::Percent,
            theme,
        ),
        ID_HOP_SENS => paint_temp(frame, &cell, view.cpu_temp_history, theme),
        ID_HOP_DISK => paint_series(
            frame,
            &cell,
            view,
            view.disk_history,
            theme.disk,
            GraphInk::Flat,
            Scale::Auto { floor: 1_024.0 },
            Axis::Bytes,
            theme,
        ),
        ID_HOP_NET => paint_series(
            frame,
            &cell,
            view,
            view.net_rx_history,
            theme.net,
            GraphInk::Flat,
            Scale::Auto { floor: 8_000.0 },
            Axis::Bits,
            theme,
        ),
        _ => {}
    }
}

fn paint_label_hop(frame: &mut Frame, area: Rect, label: &str, _hop: Panel, theme: &Theme) {
    if area.height == 0 {
        return;
    }
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(format!(" {label}  "), theme.dim()),
            Span::styled("→", theme.dim()),
        ])),
        Rect { height: 1, ..area },
    );
}

fn meta_hop_hit(
    area: Rect,
    side: MetaSide,
    panel: Panel,
    view: &AppView<'_>,
    col: u16,
    row: u16,
) -> Option<Panel> {
    if !rect_contains(area, col, row) {
        return None;
    }
    if panel == Panel::Mem {
        return mem_label_hit(area, col, row);
    }
    let hops = hop_targets(panel, view);
    if hops.is_empty() {
        return None;
    }
    match side {
        MetaSide::Bottom => {
            let hop_h = 3.min(area.height);
            if row >= area.y.saturating_add(hop_h) {
                return None;
            }
            hop_row_hit(
                Rect {
                    height: hop_h,
                    ..area
                },
                &hops,
                col,
                row,
            )
        }
        MetaSide::Left => left_slots(area, panel, view)
            .hop_rects
            .into_iter()
            .find_map(|(_, hop, rect)| rect_contains(rect, col, row).then_some(hop)),
    }
}

fn rect_contains(area: Rect, col: u16, row: u16) -> bool {
    col >= area.x
        && col < area.x.saturating_add(area.width)
        && row >= area.y
        && row < area.y.saturating_add(area.height)
}

fn paint_cores(frame: &mut Frame, area: Rect, view: &AppView<'_>, theme: &Theme) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let mut cores: Vec<CoreSample> = view.snapshot.cpu.cores.clone();
    cores.sort_by_key(|c| (core_kind_rank(c.kind), c.index));
    let end = area.y.saturating_add(area.height);
    let mut y = area.y;
    for core in cores {
        if y >= end {
            break;
        }
        let label = format!(
            "{}{}  {}",
            core.kind.tag(),
            core.index,
            percent_display(core.scaled)
        );
        let label_w = u16::try_from(label.chars().count())
            .unwrap_or(0)
            .min(area.width);
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                label,
                theme.stain(theme.cpu, core.scaled, view.snapshot.thermal),
            ))),
            Rect {
                x: area.x,
                y,
                width: label_w,
                height: 1,
            },
        );
        let bar_x = area.x.saturating_add(label_w.saturating_add(1));
        let bar_w = area.x.saturating_add(area.width).saturating_sub(bar_x);
        if bar_w > 0 {
            render_fill_bar(
                frame,
                Rect {
                    x: bar_x,
                    y,
                    width: bar_w,
                    height: 1,
                },
                core.scaled,
                theme.cpu,
            );
        }
        y = y.saturating_add(1);
    }
}

fn core_kind_rank(kind: ClusterKind) -> u8 {
    match kind {
        ClusterKind::Super => 0,
        ClusterKind::Performance => 1,
        ClusterKind::Efficiency => 2,
    }
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
            ID_PACKAGE | ID_HOP_SENS => paint_temp(frame, cell, view.cpu_temp_history, theme),
            ID_HOP_FAN => paint_fan_hop(frame, cell, view, theme),
            ID_GPU_TEMP => paint_temp(frame, cell, view.gpu_temp_history, theme),
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
        ID_DISK_READ => Some(bytes_per_sec(view.snapshot.disk.read_bps)),
        ID_HOP_DISK => Some(bytes_per_sec(
            view.snapshot
                .disk
                .read_bps
                .saturating_add(view.snapshot.disk.write_bps),
        )),
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
        ID_SUPER_LOAD => ("super", None),
        ID_PERF_LOAD => ("performance", None),
        ID_EFF_LOAD => ("efficiency", None),
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
            if let Some(word) = view.snapshot.thermal.word() {
                push_token(
                    &mut spans,
                    word.to_owned(),
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

fn peak_fan_index(view: &AppView<'_>) -> Option<usize> {
    if !view.snapshot.fans.is_present() {
        return None;
    }
    view.snapshot
        .fans
        .fans
        .iter()
        .enumerate()
        .min_by(|a, b| b.1.rpm.cmp(&a.1.rpm).then(a.0.cmp(&b.0)))
        .map(|(i, _)| i)
}

fn peak_fan(view: &AppView<'_>) -> Option<u16> {
    Some(view.snapshot.fans.fans.get(peak_fan_index(view)?)?.rpm)
}

fn paint_fan_hop(
    frame: &mut Frame,
    cell: &crate::widgets::grid::Placed,
    view: &AppView<'_>,
    theme: &Theme,
) {
    let Some(i) = peak_fan_index(view) else {
        return;
    };
    let Some(fan) = view.snapshot.fans.fans.get(i) else {
        return;
    };
    let value = if view.snapshot.fans.fans.len() >= 2 {
        format!("max {} rpm", fan.rpm)
    } else {
        format!("{} rpm", fan.rpm)
    };
    let inner = cell_titled(
        frame,
        cell.rect,
        "fan",
        Some(&value),
        cell.hop.is_some(),
        theme,
    );
    if let Some(history) = view.fan_histories.get(i).filter(|h| !h.is_empty()) {
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

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::{peak_fan, peak_fan_index};
    use crate::layout::Panel;
    use crate::widgets::tests_support::fixture;
    use plottypus_core::{FanMetric, FanSnapshot};
    use ratatui::layout::Rect;

    fn fan(rpm: u16, max_rpm: u16) -> FanMetric {
        FanMetric {
            name: String::from("Fan"),
            rpm,
            max_rpm,
        }
    }

    #[test]
    fn peak_fan_index_picks_highest_rpm_then_lowest_index() {
        let mut fx = fixture("");
        fx.snap.fans = FanSnapshot {
            fans: vec![fan(1200, 6000), fan(2140, 6000), fan(2140, 6000)],
        };
        let view = fx.view();
        assert_eq!(peak_fan_index(&view), Some(1));
        assert_eq!(peak_fan(&view), Some(2140));
    }

    #[test]
    fn peak_fan_idle_zero_is_present() {
        let mut fx = fixture("");
        fx.snap.fans = FanSnapshot {
            fans: vec![fan(0, 6000)],
        };
        let view = fx.view();
        assert_eq!(peak_fan_index(&view), Some(0));
        assert_eq!(peak_fan(&view), Some(0));
    }

    #[test]
    fn peak_fan_absent_when_no_hardware() {
        let fx = fixture("");
        assert!(peak_fan_index(&fx.view()).is_none());
        assert!(peak_fan(&fx.view()).is_none());
    }

    #[test]
    fn sensors_without_fans_do_not_open_a_fan_hop() {
        let mut fx = fixture("");
        fx.expanded = Some(Panel::Cpu);
        fx.snap.sensors.cpu_c = Some(42.0);
        fx.snap.fans = FanSnapshot { fans: Vec::new() };
        let hops = super::hop_targets(Panel::Cpu, &fx.view());
        assert!(!hops.iter().any(|(id, _)| *id == super::ID_HOP_FAN));
    }

    #[test]
    fn split_meta_78x13_hops_only() {
        let budget = super::MetaBudget {
            hops: super::HopStyle::Spark,
            mosaic: true,
            identity: false,
            extras: false,
            volumes: false,
        };
        let plan = super::split_meta(Rect::new(0, 0, 78, 13), budget, 9);
        assert_eq!(plan.main.height, 10);
        assert_eq!(plan.meta.map(|r| r.height), Some(3));
    }

    #[test]
    fn split_meta_78x16_still_no_mosaic() {
        let budget = super::MetaBudget {
            hops: super::HopStyle::Spark,
            mosaic: true,
            identity: false,
            extras: false,
            volumes: false,
        };
        let plan = super::split_meta(Rect::new(0, 0, 78, 16), budget, 9);
        assert_eq!(plan.meta.map(|r| r.height), Some(3));
        assert_eq!(plan.main.height, 13);
    }

    #[test]
    fn split_meta_78x17_mosaic_exact() {
        let budget = super::MetaBudget {
            hops: super::HopStyle::Spark,
            mosaic: true,
            identity: false,
            extras: false,
            volumes: false,
        };
        let plan = super::split_meta(Rect::new(0, 0, 78, 17), budget, 9);
        assert_eq!(plan.meta.map(|r| r.height), Some(8));
        assert_eq!(plan.main.height, 9);
    }

    #[test]
    fn pack_cpu_main_78x13_reference() {
        use crate::widgets::grid::pack;
        let mut fx = fixture("");
        fx.snap.cpu.cores = vec![
            plottypus_core::CoreSample {
                kind: plottypus_core::ClusterKind::Super,
                index: 0,
                scaled: 0.8,
                active: 0.8,
            },
            plottypus_core::CoreSample {
                kind: plottypus_core::ClusterKind::Performance,
                index: 0,
                scaled: 0.4,
                active: 0.4,
            },
        ];
        fx.snap.sensors.s_c = Some(71.0);
        fx.snap.sensors.p_c = Some(62.0);
        fx.s_temp.push(71.0);
        fx.p_temp.push(62.0);
        let bands = super::cpu_bands(&fx.view());
        let packed = pack(Rect::new(0, 0, 78, 13), &bands);
        let usage = packed.get(super::ID_SUPER_LOAD).expect("super");
        let zone = packed.get(super::ID_SUPER_ZONE).expect("super zone");
        assert!(
            usage.rect.height > 4,
            "usage must grow with leftover, got {}",
            usage.rect.height
        );
        assert_eq!(usage.rect.height.saturating_add(zone.rect.height), 13);
        assert_eq!(usage.rect.y, 0);
        assert_eq!(zone.rect.y, usage.rect.height);
        assert!(packed.get(super::ID_CPU).is_none());
        assert!(packed.get(super::ID_PACKAGE).is_none());
    }

    #[test]
    fn split_meta_wide_opens_a_left_column() {
        let budget = super::MetaBudget {
            hops: super::HopStyle::Spark,
            mosaic: true,
            identity: false,
            extras: false,
            volumes: false,
        };
        let plan = super::split_meta(Rect::new(0, 0, 160, 40), budget, 9);
        assert_eq!(plan.side, super::MetaSide::Left);
        let meta = plan.meta.expect("left column");
        assert_eq!(meta.x, 0);
        assert_eq!(meta.width, 26);
        assert_eq!(meta.height, 40);
        assert_eq!(plan.main.x, 27);
        assert_eq!(plan.main.width, 133);
        assert_eq!(plan.main.height, 40, "graphs keep the full height");
    }

    #[test]
    fn split_meta_narrow_stays_a_bottom_strip() {
        let budget = super::MetaBudget {
            hops: super::HopStyle::Spark,
            mosaic: true,
            identity: false,
            extras: false,
            volumes: false,
        };
        let plan = super::split_meta(Rect::new(0, 0, 78, 21), budget, 9);
        assert_eq!(plan.side, super::MetaSide::Bottom);
        assert_eq!(plan.main.width, 78);
        assert_eq!(plan.meta.map(|r| r.height), Some(8));
    }

    #[test]
    fn pack_cpu_shares_leftover_on_a_tall_pane() {
        use crate::widgets::grid::pack;
        let mut fx = fixture("");
        fx.snap.cpu.cores = vec![
            plottypus_core::CoreSample {
                kind: plottypus_core::ClusterKind::Super,
                index: 0,
                scaled: 0.8,
                active: 0.8,
            },
            plottypus_core::CoreSample {
                kind: plottypus_core::ClusterKind::Performance,
                index: 0,
                scaled: 0.4,
                active: 0.4,
            },
        ];
        fx.snap.sensors.s_c = Some(71.0);
        fx.snap.sensors.p_c = Some(62.0);
        fx.s_temp.push(71.0);
        fx.p_temp.push(62.0);
        let packed = pack(Rect::new(0, 0, 78, 30), &super::cpu_bands(&fx.view()));
        let usage = packed.get(super::ID_SUPER_LOAD).expect("super");
        let zone = packed.get(super::ID_SUPER_ZONE).expect("super zone");
        assert_eq!(usage.rect.height.saturating_add(zone.rect.height), 30);
        assert!(usage.rect.height >= 10, "got {}", usage.rect.height);
        assert!(zone.rect.height >= 10, "got {}", zone.rect.height);
    }

    #[test]
    fn pack_cpu_without_zones_fills_a_tall_pane() {
        use crate::widgets::grid::pack;
        let mut fx = fixture("");
        fx.snap.cpu.cores = vec![plottypus_core::CoreSample {
            kind: plottypus_core::ClusterKind::Performance,
            index: 0,
            scaled: 0.4,
            active: 0.4,
        }];
        fx.snap.sensors.e_c = None;
        fx.snap.sensors.p_c = None;
        fx.snap.sensors.s_c = None;
        fx.snap.sensors.cpu_c = None;
        fx.snap.cpu.temp_c = None;
        let packed = pack(Rect::new(0, 0, 130, 40), &super::cpu_bands(&fx.view()));
        let usage = packed.get(super::ID_PERF_LOAD).expect("performance");
        assert_eq!(usage.rect.height, 40, "usage must fill when heat is absent");
        assert!(packed.get(super::ID_SUPER_ZONE).is_none());
        assert!(packed.get(super::ID_PACKAGE).is_none());
    }

    #[test]
    fn pack_gpu_without_temp_fills_a_tall_pane() {
        use crate::widgets::grid::pack;
        let mut fx = fixture("");
        fx.snap.gpu = Some(plottypus_core::GpuSnapshot {
            scaled: 0.16,
            temp_c: None,
            ..plottypus_core::GpuSnapshot::default()
        });
        fx.snap.sensors.gpu_c = None;
        let packed = pack(Rect::new(0, 0, 130, 40), &super::gpu_bands(&fx.view()));
        let util = packed.get(super::ID_GPU_UTIL).expect("util");
        assert_eq!(util.rect.height, 40);
        assert!(packed.get(super::ID_GPU_TEMP).is_none());
    }

    #[test]
    fn pack_gpu_shares_height_between_util_and_temp() {
        use crate::widgets::grid::pack;
        let mut fx = fixture("");
        fx.snap.gpu = Some(plottypus_core::GpuSnapshot {
            scaled: 0.16,
            temp_c: Some(51.0),
            ..plottypus_core::GpuSnapshot::default()
        });
        fx.snap.sensors.gpu_c = Some(51.0);
        fx.gpu.push(0.16);
        fx.gpu_temp.push(51.0);
        let packed = pack(Rect::new(0, 0, 130, 30), &super::gpu_bands(&fx.view()));
        let util = packed.get(super::ID_GPU_UTIL).expect("util");
        let temp = packed.get(super::ID_GPU_TEMP).expect("temp");
        assert_eq!(util.rect.height.saturating_add(temp.rect.height), 30);
        let gap = util.rect.height.abs_diff(temp.rect.height);
        assert!(
            gap <= 1,
            "util {} vs temp {}",
            util.rect.height,
            temp.rect.height
        );
    }
}

mod cpu;
mod disk;
mod expanded;
mod fans;
mod footer;
mod gpu;
mod grid;
mod help;
mod mem;
mod net;
mod processes;

use plottypus_core::{History, ProcSort, Snapshot, Surface};
use ratatui::Frame;
use ratatui::layout::Rect;

use crate::layout::{Degrade, LayoutFlags, Panel, plan};
use crate::theme::Theme;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Focus {
    Cpu,
    Gpu,
    Mem,
    Net,
    Disk,
    Fans,
    Search,
    #[default]
    Processes,
}

impl Focus {
    #[must_use]
    pub const fn panel(self) -> Panel {
        match self {
            Self::Cpu => Panel::Cpu,
            Self::Gpu => Panel::Gpu,
            Self::Mem => Panel::Mem,
            Self::Net => Panel::Net,
            Self::Disk => Panel::Disk,
            Self::Fans => Panel::Fans,
            Self::Search | Self::Processes => Panel::Processes,
        }
    }

    #[must_use]
    pub const fn from_panel(panel: Panel) -> Self {
        match panel {
            Panel::Cpu => Self::Cpu,
            Panel::Gpu => Self::Gpu,
            Panel::Mem => Self::Mem,
            Panel::Net => Self::Net,
            Panel::Disk => Self::Disk,
            Panel::Fans => Self::Fans,
            Panel::Processes => Self::Processes,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct ProcView {
    pub selected: usize,
    pub selected_pid: Option<u32>,
    pub filter: String,
}

#[derive(Debug, Clone)]
pub struct AppView<'a> {
    pub snapshot: &'a Snapshot,
    pub cpu_history: &'a History,
    pub gpu_history: &'a History,
    pub mem_history: &'a History,
    pub net_rx_history: &'a History,
    pub net_tx_history: &'a History,
    pub disk_history: &'a History,
    pub disk_read_history: &'a History,
    pub disk_write_history: &'a History,
    pub cpu_temp_history: &'a History,
    pub gpu_temp_history: &'a History,
    pub e_temp_history: &'a History,
    pub p_temp_history: &'a History,
    pub s_temp_history: &'a History,
    pub e_load_history: &'a History,
    pub p_load_history: &'a History,
    pub s_load_history: &'a History,
    pub fan_histories: &'a [History],
    pub surface: Surface,
    pub degrade: Degrade,
    pub focus: Focus,
    pub proc: &'a ProcView,
    pub help: bool,
    pub settings: bool,
    pub confirm_kill: bool,
    pub confirm_pid: Option<u32>,
    pub confirm_signal: &'static str,
    pub searching: bool,
    pub ready: bool,
    pub frozen: bool,
    pub show_gpu: bool,
    pub show_net: bool,
    pub show_disk: bool,
    pub show_fans: bool,
    pub show_cores: bool,
    pub show_threads: bool,
    pub show_tree: bool,
    pub sort: ProcSort,
    pub detail_pid: Option<u32>,
    pub expanded: Option<Panel>,
    pub proc_ratio: u16,
    pub interval_ms: u64,
    pub status: Option<&'a str>,
}

impl AppView<'_> {
    #[must_use]
    pub fn flags(&self) -> LayoutFlags {
        LayoutFlags {
            show_gpu: self.show_gpu,
            show_net: self.show_net,
            show_disk: self.show_disk,
            show_fans: self.show_fans,
            has_gpu: self.snapshot.gpu.is_some(),
            has_fans: self.snapshot.fans.is_present() || self.snapshot.sensors.is_present(),
            has_disk: !self.snapshot.disk.volumes.is_empty(),
            expanded: self.expanded,
            proc_ratio: self.proc_ratio,
        }
    }

    #[must_use]
    pub fn is_focused(&self, panel: Panel) -> bool {
        self.focus.panel() == panel
    }

    #[must_use]
    pub fn is_expanded(&self, panel: Panel) -> bool {
        self.expanded == Some(panel)
    }

    #[must_use]
    pub fn zone_temp_history(&self, kind: plottypus_core::ClusterKind) -> &History {
        match kind {
            plottypus_core::ClusterKind::Efficiency => self.e_temp_history,
            plottypus_core::ClusterKind::Performance => self.p_temp_history,
            plottypus_core::ClusterKind::Super => self.s_temp_history,
        }
    }

    #[must_use]
    pub fn cluster_load_history(&self, kind: plottypus_core::ClusterKind) -> &History {
        match kind {
            plottypus_core::ClusterKind::Efficiency => self.e_load_history,
            plottypus_core::ClusterKind::Performance => self.p_load_history,
            plottypus_core::ClusterKind::Super => self.s_load_history,
        }
    }
}

pub use expanded::hop_hit;
pub use footer::footer_hit;
pub use processes::filtered as filtered_processes;
pub use processes::{DetailAction, detail_actions, detail_rect};

pub fn render_app(frame: &mut Frame, view: &AppView<'_>) {
    let theme = Theme::default();
    let area = frame.area();
    let layout = plan(area, view.surface, view.flags());
    let view = AppView {
        surface: layout.surface,
        degrade: layout.degrade,
        ..view.clone()
    };
    let expanded = layout.expanded;

    if let Some(area) = layout.cpu {
        render_panel(
            frame,
            area,
            &view,
            &theme,
            Panel::Cpu,
            expanded,
            cpu::render,
        );
    }
    if let Some(area) = layout.gpu {
        render_panel(
            frame,
            area,
            &view,
            &theme,
            Panel::Gpu,
            expanded,
            gpu::render,
        );
    }
    if let Some(area) = layout.mem {
        render_panel(
            frame,
            area,
            &view,
            &theme,
            Panel::Mem,
            expanded,
            mem::render,
        );
    }
    if let Some(area) = layout.net {
        render_panel(
            frame,
            area,
            &view,
            &theme,
            Panel::Net,
            expanded,
            net::render,
        );
    }
    if let Some(area) = layout.disk {
        render_panel(
            frame,
            area,
            &view,
            &theme,
            Panel::Disk,
            expanded,
            disk::render,
        );
    }
    if let Some(area) = layout.fans {
        render_panel(
            frame,
            area,
            &view,
            &theme,
            Panel::Fans,
            expanded,
            fans::render,
        );
    }
    if let Some(area) = layout.processes {
        render_panel(
            frame,
            area,
            &view,
            &theme,
            Panel::Processes,
            expanded,
            processes::render,
        );
    }
    footer::render(frame, layout.footer, &view, &theme);

    if view.settings {
        help::render_settings(frame, area, &view, &theme);
    } else if view.help {
        help::render(frame, area, &theme);
    }
}

fn render_panel(
    frame: &mut Frame,
    area: Rect,
    view: &AppView<'_>,
    theme: &Theme,
    panel: Panel,
    expanded: Option<Panel>,
    compact: fn(&mut Frame, Rect, &AppView<'_>, &Theme),
) {
    if expanded == Some(panel) {
        expanded::render(frame, area, view, theme, panel);
    } else {
        compact(frame, area, view, theme);
    }
}

#[must_use]
pub fn inner_process_area(area: Rect, surface: Surface, flags: LayoutFlags) -> Option<Rect> {
    plan(area, surface, flags).processes
}

#[cfg(test)]
pub(crate) mod tests_support {
    use plottypus_core::{History, Process, Snapshot, Surface};

    use super::{AppView, Focus, ProcView};
    use crate::layout::Degrade;

    pub(crate) struct Fixture {
        pub snap: Snapshot,
        pub cpu: History,
        pub gpu: History,
        pub mem: History,
        pub net_rx: History,
        pub net_tx: History,
        pub disk: History,
        pub disk_read: History,
        pub disk_write: History,
        pub cpu_temp: History,
        pub gpu_temp: History,
        pub e_temp: History,
        pub p_temp: History,
        pub s_temp: History,
        pub e_load: History,
        pub p_load: History,
        pub s_load: History,
        pub fans: Vec<History>,
        pub proc: ProcView,
        pub surface: Surface,
        pub degrade: Degrade,
        pub focus: Focus,
        pub help: bool,
        pub settings: bool,
        pub confirm_kill: bool,
        pub confirm_pid: Option<u32>,
        pub confirm_signal: &'static str,
        pub detail_pid: Option<u32>,
        pub searching: bool,
        pub ready: bool,
        pub frozen: bool,
        pub show_tree: bool,
        pub expanded: Option<crate::layout::Panel>,
    }

    pub(crate) fn process(pid: u32, name: &str, cpu: f32) -> Process {
        Process {
            pid,
            ppid: 1,
            name: name.to_owned(),
            cpu,
            mem_bytes: 0,
            threads: 1,
            gpu: 0.0,
            user: String::from("cloaky"),
            command: None,
            status: "sleeping",
            start_unix: 0,
            cpu_spark: Vec::new(),
        }
    }

    pub(crate) fn fixture(filter: &str) -> Fixture {
        let mut snap = Snapshot::empty();
        snap.soc.name = String::from("M4 Pro");
        snap.soc.e_cores = 4;
        snap.soc.p_cores = 8;
        snap.soc.gpu_cores = 16;
        snap.soc.memory_bytes = 36 * 1024 * 1024 * 1024;
        snap.processes = vec![process(904, "Xcode", 48.1)];
        Fixture {
            snap,
            cpu: History::default(),
            gpu: History::default(),
            mem: History::default(),
            net_rx: History::default(),
            net_tx: History::default(),
            disk: History::default(),
            disk_read: History::default(),
            disk_write: History::default(),
            cpu_temp: History::default(),
            gpu_temp: History::default(),
            e_temp: History::default(),
            p_temp: History::default(),
            s_temp: History::default(),
            e_load: History::default(),
            p_load: History::default(),
            s_load: History::default(),
            fans: Vec::new(),
            proc: ProcView {
                filter: filter.to_owned(),
                ..ProcView::default()
            },
            surface: Surface::Work,
            degrade: Degrade::Full,
            focus: Focus::Processes,
            help: false,
            settings: false,
            confirm_kill: false,
            confirm_pid: None,
            confirm_signal: "TERM",
            detail_pid: None,
            searching: false,
            ready: true,
            frozen: false,
            show_tree: false,
            expanded: None,
        }
    }

    impl Fixture {
        pub(crate) fn view(&self) -> AppView<'_> {
            AppView {
                snapshot: &self.snap,
                degrade: self.degrade,
                cpu_history: &self.cpu,
                gpu_history: &self.gpu,
                mem_history: &self.mem,
                net_rx_history: &self.net_rx,
                net_tx_history: &self.net_tx,
                disk_history: &self.disk,
                disk_read_history: &self.disk_read,
                disk_write_history: &self.disk_write,
                cpu_temp_history: &self.cpu_temp,
                gpu_temp_history: &self.gpu_temp,
                e_temp_history: &self.e_temp,
                p_temp_history: &self.p_temp,
                s_temp_history: &self.s_temp,
                e_load_history: &self.e_load,
                p_load_history: &self.p_load,
                s_load_history: &self.s_load,
                fan_histories: &self.fans,
                surface: self.surface,
                focus: self.focus,
                proc: &self.proc,
                help: self.help,
                settings: self.settings,
                confirm_kill: self.confirm_kill,
                confirm_pid: self.confirm_pid,
                confirm_signal: self.confirm_signal,
                searching: self.searching,
                ready: self.ready,
                frozen: self.frozen,
                show_gpu: true,
                show_net: true,
                show_disk: true,
                show_fans: true,
                show_cores: true,
                show_threads: false,
                show_tree: self.show_tree,
                sort: plottypus_core::ProcSort::Cpu,
                detail_pid: self.detail_pid,
                expanded: self.expanded,
                proc_ratio: 55,
                interval_ms: 1000,
                status: None,
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod render_tests {
    use plottypus_core::{
        ClusterKind, CoreSample, DiskSnapshot, DiskVolume, FanMetric, FanSnapshot, GpuSnapshot,
        History, Surface,
    };
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    use super::render_app;
    use super::tests_support::fixture;
    use crate::layout::Panel;

    fn paint(view: &super::AppView<'_>, width: u16, height: u16) -> String {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| render_app(frame, view)).unwrap();
        let buf = terminal.backend().buffer();
        let mut out = String::new();
        for y in 0..buf.area.height {
            for x in 0..buf.area.width {
                out.push_str(buf[(x, y)].symbol());
            }
            out.push('\n');
        }
        out
    }

    #[test]
    fn work_paints_separate_boxes() {
        let mut fx = fixture("");
        fx.snap.cpu.scaled = 0.184;
        fx.snap.cpu.active = 0.184;
        fx.snap.cpu.watts = Some(8.24);
        fx.snap.cpu.temp_c = Some(42.0);
        fx.snap.memory.used_bytes = 18 * 1024 * 1024 * 1024;
        fx.snap.memory.total_bytes = 36 * 1024 * 1024 * 1024;
        fx.snap.gpu = Some(GpuSnapshot {
            scaled: 0.12,
            watts: Some(1.1),
            freq_mhz: Some(461),
            temp_c: Some(51.0),
            ..GpuSnapshot::default()
        });
        fx.snap.disk = DiskSnapshot {
            volumes: vec![DiskVolume {
                name: String::from("Macintosh HD"),
                mount: String::from("/"),
                used_bytes: 400 * 1024 * 1024 * 1024,
                total_bytes: 926 * 1024 * 1024 * 1024,
            }],
            read_bps: 0,
            write_bps: 0,
        };
        fx.snap.fans = FanSnapshot {
            fans: vec![FanMetric {
                name: String::from("Fan"),
                rpm: 1850,
                max_rpm: 6000,
            }],
        };
        fx.ready = true;
        let text = paint(&fx.view(), 120, 36);
        assert!(text.contains("cpu"), "{text}");
        assert!(text.contains("18%"), "{text}");
        assert!(text.contains("gpu"), "{text}");
        assert!(text.contains("51°"), "{text}");
        assert!(text.contains('↗'), "{text}");
        assert!(text.contains("mem"), "{text}");
        assert!(text.contains("net"), "{text}");
        assert!(text.contains("disk"), "{text}");
        assert!(text.contains("sens"), "{text}");
        assert!(text.contains("Xcode"), "{text}");
        assert!(text.contains("help"), "{text}");
        assert!(!text.contains("nominal"), "{text}");
    }

    #[test]
    fn flags_has_fans_when_sensors_present() {
        let mut fx = fixture("");
        assert!(!fx.view().flags().has_fans);
        fx.snap.sensors.cpu_c = Some(42.0);
        assert!(fx.view().flags().has_fans);
        assert!(fx.view().flags().visible(Panel::Fans));
    }

    #[test]
    fn sensors_box_shows_without_fans() {
        let mut fx = fixture("");
        fx.snap.sensors.cpu_c = Some(42.0);
        let text = paint(&fx.view(), 120, 36);
        assert!(text.contains("sens"), "{text}");
        assert!(text.contains("42°"), "{text}");
    }

    #[test]
    fn glance_is_compact_cluster() {
        let mut fx = fixture("");
        fx.surface = Surface::Glance;
        fx.snap.cpu.scaled = 0.18;
        fx.snap.cpu.active = 0.18;
        fx.snap.cpu.watts = Some(8.2);
        fx.snap.gpu = Some(GpuSnapshot {
            scaled: 0.12,
            watts: Some(1.1),
            freq_mhz: Some(461),
            ..GpuSnapshot::default()
        });
        let text = paint(&fx.view(), 80, 20);
        assert!(text.contains("cpu"), "{text}");
        assert!(text.contains("gpu"), "{text}");
        assert!(text.contains("mem"), "{text}");
        assert!(!text.contains("nominal"), "{text}");
        assert!(!text.contains("Xcode"), "{text}");
    }

    #[test]
    fn work_request_rendered_inside_the_panic_window_is_glance() {
        // Regression for the 40..59 column clamp panic: asking for Work here
        // must fall back and still paint a complete frame.
        let mut fx = fixture("");
        fx.ready = true;
        fx.snap.cpu.active = 0.3;
        for (width, height) in [(45_u16, 20_u16), (59, 16), (41, 24)] {
            let text = paint(&fx.view(), width, height);
            assert!(text.contains("cpu"), "{width}x{height}: {text}");
            assert!(text.contains("mem"), "{width}x{height}: {text}");
        }
    }

    #[test]
    fn tight_work_drops_fan_graph_but_keeps_headline() {
        use crate::layout::Degrade;
        let mut fx = fixture("");
        fx.ready = true;
        fx.snap.cpu.scaled = 0.3;
        fx.snap.sensors.cpu_c = Some(42.0);
        fx.snap.fans = FanSnapshot {
            fans: vec![FanMetric {
                name: String::from("Fan 1"),
                rpm: 1200,
                max_rpm: 6000,
            }],
        };
        let mut view = fx.view();
        view.degrade = Degrade::Tight;
        let text = paint(&view, 120, 30);
        assert!(text.contains("1200 rpm"), "{text}");
        assert!(
            !text.contains("related"),
            "compact never shows related: {text}"
        );
    }

    #[test]
    fn minimal_work_strips_specs_and_graphs() {
        use crate::layout::Degrade;
        let mut fx = fixture("");
        fx.ready = true;
        fx.snap.cpu.scaled = 0.3;
        fx.snap.memory.wired_bytes = 8 * 1024 * 1024 * 1024;
        fx.snap.cpu.temp_c = Some(42.0);
        let mut view = fx.view();
        view.degrade = Degrade::Minimal;
        let text = paint(&view, 100, 26);
        assert!(text.contains("mem"), "{text}");
        assert!(
            !text.contains("wired"),
            "specs must collapse at Minimal: {text}"
        );
        assert!(
            !text.contains("M4 Pro"),
            "cpu spec subline must collapse at Minimal: {text}"
        );
    }

    #[test]
    fn expand_cpu_is_a_grid_of_cells() {
        let mut fx = fixture("");
        fx.expanded = Some(Panel::Cpu);
        fx.snap.cpu.scaled = 0.5;
        fx.snap.cpu.active = 0.7;
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
        fx.e_temp.push(36.0);
        fx.p_temp.push(51.0);
        fx.e_load.push(0.2);
        fx.p_load.push(0.8);
        let text = paint(&fx.view(), 80, 24);
        for want in [
            "cpu",
            "efficiency",
            "performance",
            "eff zone",
            "perf zone",
            "E0",
            "P0",
            "36°",
            "51°",
        ] {
            assert!(text.contains(want), "missing {want}: {text}");
        }
        assert!(text.contains("busy 70%"), "{text}");
        assert!(!text.contains("power"), "empty power cell: {text}");
        assert!(!text.contains("clock"), "empty clock cell: {text}");
        assert!(
            !text.contains("Macintosh"),
            "other panels must hide: {text}"
        );
        assert!(!text.contains("Xcode"), "procs stay in their pane: {text}");
    }

    #[test]
    fn expand_gpu_is_stats_over_two_graphs() {
        let mut fx = fixture("");
        fx.expanded = Some(Panel::Gpu);
        fx.ready = true;
        fx.snap.gpu = Some(GpuSnapshot {
            scaled: 0.12,
            watts: Some(1.1),
            freq_mhz: Some(461),
            temp_c: Some(51.0),
            ..GpuSnapshot::default()
        });
        let text = paint(&fx.view(), 80, 24);
        for want in ["gpu", "gpu temp", "12%", "51°"] {
            assert!(text.contains(want), "missing {want}: {text}");
        }
        assert!(text.contains("1.1W") || text.contains("1.1 W"), "{text}");
        assert!(!text.contains("no readings on this machine"), "{text}");
    }

    #[test]
    fn expand_cpu_hides_empty_power_and_clock() {
        let mut fx = fixture("");
        fx.expanded = Some(Panel::Cpu);
        fx.snap.cpu.scaled = 0.4;
        fx.snap.cpu.active = 0.4;
        fx.snap.cpu.watts = None;
        fx.snap.cpu.freq_mhz = None;
        fx.snap.cpu.cores = vec![CoreSample {
            kind: ClusterKind::Performance,
            index: 0,
            scaled: 0.4,
            active: 0.4,
        }];
        let text = paint(&fx.view(), 80, 24);
        assert!(text.contains("cpu"), "{text}");
        assert!(text.contains("performance"), "{text}");
        assert!(!text.contains("power"), "{text}");
        assert!(!text.contains("clock"), "{text}");
        assert!(text.contains("P0"), "{text}");
    }

    #[test]
    fn expand_gpu_hides_empty_optional_cells() {
        let mut fx = fixture("");
        fx.expanded = Some(Panel::Gpu);
        fx.ready = true;
        fx.snap.gpu = Some(GpuSnapshot {
            scaled: 0.22,
            ..GpuSnapshot::default()
        });
        fx.snap.soc.gpu_cores = 0;
        let text = paint(&fx.view(), 80, 24);
        assert!(text.contains("gpu"), "{text}");
        assert!(text.contains("22%"), "{text}");
        assert!(!text.contains("power"), "{text}");
        assert!(!text.contains("clock"), "{text}");
        assert!(!text.contains("gpu temp"), "{text}");
    }

    #[test]
    fn expand_disk_graphs_io_not_capacity() {
        let mut fx = fixture("");
        fx.expanded = Some(Panel::Disk);
        fx.snap.disk = DiskSnapshot {
            volumes: vec![DiskVolume {
                name: String::from("Macintosh HD"),
                mount: String::from("/"),
                used_bytes: 400 * 1024 * 1024 * 1024,
                total_bytes: 926 * 1024 * 1024 * 1024,
            }],
            read_bps: 2 * 1024 * 1024,
            write_bps: 512 * 1024,
        };
        let text = paint(&fx.view(), 80, 24);
        assert!(text.contains("volumes"), "{text}");
        assert!(text.contains("Macintosh HD"), "{text}");
        assert!(text.contains("read"), "{text}");
        assert!(text.contains("write"), "{text}");
        assert!(text.contains("2.0M/s") || text.contains("2M/s"), "{text}");
        assert!(!text.contains("primary volume"), "{text}");
    }

    #[test]
    fn expand_sensors_graphs_each_fan() {
        let mut fx = fixture("");
        fx.expanded = Some(Panel::Fans);
        fx.snap.fans = FanSnapshot {
            fans: vec![
                FanMetric {
                    name: String::from("Fan 1"),
                    rpm: 1200,
                    max_rpm: 6000,
                },
                FanMetric {
                    name: String::from("Fan 2"),
                    rpm: 1850,
                    max_rpm: 6000,
                },
            ],
        };
        fx.snap.sensors.cpu_c = Some(42.0);
        fx.snap.sensors.p_c = Some(51.0);
        fx.snap.sensors.s_c = Some(62.0);
        fx.p_temp.push(51.0);
        fx.s_temp.push(62.0);
        fx.fans = vec![{
            let mut h = History::default();
            for rpm in [800.0, 1000.0, 1200.0] {
                h.push(rpm);
            }
            h
        }];
        fx.cpu_temp.push(40.0);
        fx.cpu_temp.push(42.0);
        let text = paint(&fx.view(), 100, 30);
        assert!(text.contains("Fan 1"), "{text}");
        assert!(text.contains("Fan 2"), "{text}");
        assert!(text.contains("1200 rpm"), "{text}");
        assert!(text.contains("1850 rpm"), "{text}");
        assert!(text.contains("perf zone"), "{text}");
        assert!(text.contains("super zone"), "{text}");
        assert!(text.contains("51°"), "{text}");
        assert!(text.contains("62°"), "{text}");
        assert!(!text.contains("E0 36°"), "{text}");
    }

    #[test]
    fn compact_sens_sparks_at_80x24_minimal() {
        let mut fx = fixture("");
        fx.snap.sensors.e_c = Some(48.0);
        fx.snap.sensors.p_c = Some(62.0);
        fx.snap.sensors.s_c = Some(71.0);
        fx.cpu_temp.push(40.0);
        fx.cpu_temp.push(71.0);
        let view = fx.view();
        let layout = crate::layout::plan(
            ratatui::layout::Rect::new(0, 0, 80, 24),
            Surface::Work,
            view.flags(),
        );
        assert_eq!(layout.degrade, crate::layout::Degrade::Minimal);
        assert!(layout.fans.is_some());
        let text = paint(&view, 80, 24);
        assert!(text.contains("48°"), "{text}");
        assert!(text.contains("62°"), "{text}");
        assert!(text.contains('e'), "{text}");
        assert!(text.contains('p'), "{text}");
        let has_spark = text.chars().any(|c| ('\u{2801}'..='\u{28FF}').contains(&c));
        assert!(has_spark, "SENS at 80x24 Minimal must keep a spark: {text}");
    }

    #[test]
    fn glance_sens_is_numbers_not_spark() {
        let mut fx = fixture("");
        fx.surface = Surface::Glance;
        fx.snap.sensors.e_c = Some(48.0);
        fx.snap.sensors.p_c = Some(62.0);
        fx.cpu_temp.push(48.0);
        fx.cpu_temp.push(62.0);
        let text = paint(&fx.view(), 80, 20);
        assert!(text.contains("48°") || text.contains("e 48"), "{text}");
    }

    #[test]
    fn expand_sens_shows_cpu_gpu_hops() {
        let mut fx = fixture("");
        fx.expanded = Some(Panel::Fans);
        fx.snap.gpu = Some(GpuSnapshot {
            scaled: 0.31,
            ..GpuSnapshot::default()
        });
        fx.snap.cpu.scaled = 0.44;
        fx.snap.sensors.p_c = Some(51.0);
        fx.p_temp.push(51.0);
        let text = paint(&fx.view(), 100, 30);
        assert!(text.contains("cpu"), "{text}");
        assert!(text.contains("gpu"), "{text}");
        assert!(text.contains('→'), "{text}");
        assert!(text.contains("44%") || text.contains("31%"), "{text}");
    }

    #[test]
    fn hop_hit_on_related_cell() {
        let mut fx = fixture("");
        fx.expanded = Some(Panel::Cpu);
        fx.snap.gpu = Some(GpuSnapshot {
            scaled: 0.2,
            ..GpuSnapshot::default()
        });
        fx.snap.fans = FanSnapshot {
            fans: vec![FanMetric {
                name: String::from("Fan 1"),
                rpm: 1000,
                max_rpm: 6000,
            }],
        };
        let view = fx.view();
        let area = ratatui::layout::Rect::new(0, 0, 80, 23);
        let hit = (0..area.height).find_map(|y| {
            (0..area.width).find_map(|x| {
                crate::widgets::hop_hit(area, &view, x, y).filter(|p| *p == Panel::Gpu)
            })
        });
        assert_eq!(hit, Some(Panel::Gpu), "a gpu hop cell must exist");
    }

    #[test]
    fn expand_cpu_honors_show_cores_off() {
        let mut fx = fixture("");
        fx.expanded = Some(Panel::Cpu);
        fx.snap.cpu.cores = vec![CoreSample {
            kind: ClusterKind::Efficiency,
            index: 0,
            scaled: 0.3,
            active: 0.3,
        }];
        let mut view = fx.view();
        view.show_cores = false;
        let text = paint(&view, 80, 24);
        assert!(text.contains("efficiency"), "{text}");
        assert!(!text.contains("E0"), "{text}");
    }

    #[test]
    fn detail_popup_shows_identity_and_actions() {
        let mut fx = fixture("");
        fx.proc.selected_pid = Some(904);
        fx.snap.processes = vec![crate::widgets::tests_support::process(904, "claude", 12.5)];
        fx.snap.processes[0].command = Some(String::from("/Users/u/.local/bin/claude"));
        fx.snap.processes[0].start_unix = 1_700_000_000;
        fx.expanded = None;
        let mut view = fx.view();
        view.detail_pid = Some(904);
        let text = paint(&view, 100, 30);
        for want in [
            "process",
            "@cloaky",
            "/Users/u/.local/bin/claude",
            "sleeping",
            "term",
            "kill",
            "interrupt",
            "esc close",
        ] {
            assert!(text.contains(want), "missing {want}: {text}");
        }
    }

    #[test]
    fn help_overlay_lists_keys() {
        let mut fx = fixture("");
        fx.help = true;
        let text = paint(&fx.view(), 80, 24);
        assert!(text.contains("help"), "{text}");
        assert!(text.contains("search"), "{text}");
        assert!(text.contains("kill"), "{text}");
        assert!(text.contains("settings"), "{text}");
        assert!(text.contains("expand"), "{text}");
        assert!(text.contains("focus"), "{text}");
        assert!(text.contains('↗'), "{text}");
    }
}

#[cfg(test)]
mod debug_degrade {
    use crate::layout::{Degrade, LayoutFlags, plan};
    use plottypus_core::Surface;
    use ratatui::layout::Rect;

    #[test]
    fn print_degrades() {
        let fs = LayoutFlags {
            show_gpu: true,
            show_net: true,
            show_disk: true,
            show_fans: true,
            has_gpu: true,
            has_fans: true,
            has_disk: true,
            expanded: None,
            proc_ratio: 55,
        };
        for (w, h) in [(100u16, 26u16), (160, 17), (120, 30)] {
            let p = plan(Rect::new(0, 0, w, h), Surface::Work, fs);
            println!("{w}x{h} -> {:?} surface={:?}", p.degrade, p.surface);
        }
        assert_eq!(
            plan(Rect::new(0, 0, 100, 26), Surface::Work, fs).degrade,
            Degrade::Minimal
        );
    }
}

#[cfg(test)]
mod visual_dump {
    use crate::layout::Panel;
    use crate::widgets::render_app;
    use crate::widgets::tests_support::fixture;
    use plottypus_core::*;
    use ratatui::{Terminal, backend::TestBackend};

    fn paint(view: &crate::widgets::AppView<'_>, w: u16, h: u16) -> String {
        let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
        t.draw(|f| render_app(f, view)).unwrap();
        let b = t.backend().buffer();
        let mut out = String::new();
        for y in 0..b.area.height {
            out.push('|');
            for x in 0..b.area.width {
                out.push_str(b[(x, y)].symbol());
            }
            out.push('\n');
        }
        out
    }

    #[test]
    #[ignore = "visual tool: run with --ignored --nocapture"]
    fn dump_detail_popup() {
        use crate::widgets::tests_support::process;
        let mut fx = fixture("");
        fx.ready = true;
        fx.snap.processes = vec![process(904, "claude", 12.5)];
        fx.snap.processes[0].user = String::from("cloaky");
        fx.snap.processes[0].command = Some(String::from(
            "/Users/cloaky/.local/share/claude/versions/2.1.241/bin/claude",
        ));
        fx.snap.processes[0].start_unix = 1_755_000_000;
        fx.snap.processes[0].mem_bytes = 544 * 1024 * 1024;
        let mut view = fx.view();
        view.detail_pid = Some(904);
        println!("{}", paint(&view, 100, 30));
    }

    #[test]
    #[ignore = "visual tool: run with --ignored --nocapture"]
    fn dump_all_expanded() {
        for (panel, name) in [
            (Panel::Cpu, "CPU"),
            (Panel::Gpu, "GPU"),
            (Panel::Mem, "MEM"),
            (Panel::Net, "NET"),
            (Panel::Disk, "DISK"),
            (Panel::Fans, "SENS"),
        ] {
            let mut fx = fixture("");
            fx.ready = true;
            fx.expanded = Some(panel);
            fx.snap.cpu.scaled = 0.42;
            fx.snap.cpu.active = 0.61;
            fx.snap.cpu.watts = Some(9.3);
            fx.snap.cpu.cores = (0..10)
                .map(|i| CoreSample {
                    kind: if i < 4 {
                        ClusterKind::Efficiency
                    } else {
                        ClusterKind::Performance
                    },
                    index: i % 6,
                    scaled: f32::from(i) * 0.07 + 0.15,
                    active: f32::from(i) * 0.06 + 0.2,
                })
                .collect();
            fx.snap.sensors.e_c = Some(38.0);
            fx.snap.sensors.p_c = Some(58.0);
            fx.snap.gpu = Some(GpuSnapshot {
                scaled: 0.31,
                watts: Some(2.2),
                ane_watts: Some(0.4),
                freq_mhz: Some(1278),
                temp_c: Some(49.0),
                ..Default::default()
            });
            fx.snap.memory.used_bytes = 22 * 1024 * 1024 * 1024;
            fx.snap.memory.total_bytes = 36 * 1024 * 1024 * 1024;
            fx.snap.memory.wired_bytes = 7 * 1024 * 1024 * 1024;
            fx.snap.memory.compressed_bytes = 2 * 1024 * 1024 * 1024;
            fx.snap.memory.cache_bytes = 3 * 1024 * 1024 * 1024;
            fx.snap.memory.swap_used_bytes = 512 * 1024 * 1024;
            fx.snap.memory.swap_total_bytes = 4 * 1024 * 1024 * 1024;
            fx.snap.network.iface = "en0".into();
            fx.snap.network.rx_bps = 12_400_000;
            fx.snap.network.tx_bps = 890_000;
            fx.snap.disk.volumes.push(DiskVolume {
                name: "Macintosh HD".into(),
                mount: "/".into(),
                used_bytes: 400 * 1024 * 1024 * 1024,
                total_bytes: 926 * 1024 * 1024 * 1024,
            });
            fx.snap.fans.fans.push(FanMetric {
                name: "Fan 1".into(),
                rpm: 1720,
                max_rpm: 6000,
            });
            fx.snap.fans.fans.push(FanMetric {
                name: "Fan 2".into(),
                rpm: 2140,
                max_rpm: 5800,
            });
            let text = paint(&fx.view(), 100, 30);
            println!("\n===== {name} =====\n{text}");
        }
    }
}

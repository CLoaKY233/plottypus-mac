mod cpu;
mod disk;
mod fans;
mod footer;
mod gpu;
mod help;
mod mem;
mod net;
mod processes;

use plottypus_core::{History, ProcSort, Snapshot, Surface};
use ratatui::Frame;
use ratatui::layout::Rect;

use crate::layout::{LayoutFlags, Panel, plan};
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
    pub cpu_temp_history: &'a History,
    pub gpu_temp_history: &'a History,
    pub e_temp_history: &'a History,
    pub p_temp_history: &'a History,
    pub s_temp_history: &'a History,
    pub surface: Surface,
    pub focus: Focus,
    pub proc: &'a ProcView,
    pub help: bool,
    pub settings: bool,
    pub confirm_kill: bool,
    pub searching: bool,
    pub ready: bool,
    pub frozen: bool,
    pub show_gpu: bool,
    pub show_net: bool,
    pub show_disk: bool,
    pub show_fans: bool,
    pub show_cores: bool,
    pub show_threads: bool,
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
}

pub use processes::filtered as filtered_processes;

pub fn render_app(frame: &mut Frame, view: &AppView<'_>) {
    let theme = Theme::default();
    let area = frame.area();
    let layout = plan(area, view.surface, view.flags());
    let view = AppView {
        surface: layout.surface,
        ..view.clone()
    };

    if let Some(area) = layout.cpu {
        cpu::render(frame, area, &view, &theme);
    }
    if let Some(area) = layout.gpu {
        gpu::render(frame, area, &view, &theme);
    }
    if let Some(area) = layout.mem {
        mem::render(frame, area, &view, &theme);
    }
    if let Some(area) = layout.net {
        net::render(frame, area, &view, &theme);
    }
    if let Some(area) = layout.disk {
        disk::render(frame, area, &view, &theme);
    }
    if let Some(area) = layout.fans {
        fans::render(frame, area, &view, &theme);
    }
    if let Some(area) = layout.processes {
        processes::render(frame, area, &view, &theme);
    }
    footer::render(frame, layout.footer, &view, &theme);

    if view.settings {
        help::render_settings(frame, area, &view, &theme);
    } else if view.help {
        help::render(frame, area, &theme);
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

    pub(crate) struct Fixture {
        pub snap: Snapshot,
        pub cpu: History,
        pub gpu: History,
        pub mem: History,
        pub net_rx: History,
        pub net_tx: History,
        pub disk: History,
        pub cpu_temp: History,
        pub gpu_temp: History,
        pub e_temp: History,
        pub p_temp: History,
        pub s_temp: History,
        pub proc: ProcView,
        pub surface: Surface,
        pub focus: Focus,
        pub help: bool,
        pub settings: bool,
        pub confirm_kill: bool,
        pub searching: bool,
        pub ready: bool,
        pub frozen: bool,
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
            cpu_temp: History::default(),
            gpu_temp: History::default(),
            e_temp: History::default(),
            p_temp: History::default(),
            s_temp: History::default(),
            proc: ProcView {
                filter: filter.to_owned(),
                ..ProcView::default()
            },
            surface: Surface::Work,
            focus: Focus::Processes,
            help: false,
            settings: false,
            confirm_kill: false,
            searching: false,
            ready: true,
            frozen: false,
            expanded: None,
        }
    }

    impl Fixture {
        pub(crate) fn view(&self) -> AppView<'_> {
            AppView {
                snapshot: &self.snap,
                cpu_history: &self.cpu,
                gpu_history: &self.gpu,
                mem_history: &self.mem,
                net_rx_history: &self.net_rx,
                net_tx_history: &self.net_tx,
                disk_history: &self.disk,
                cpu_temp_history: &self.cpu_temp,
                gpu_temp_history: &self.gpu_temp,
                e_temp_history: &self.e_temp,
                p_temp_history: &self.p_temp,
                s_temp_history: &self.s_temp,
                surface: self.surface,
                focus: self.focus,
                proc: &self.proc,
                help: self.help,
                settings: self.settings,
                confirm_kill: self.confirm_kill,
                searching: self.searching,
                ready: self.ready,
                frozen: self.frozen,
                show_gpu: true,
                show_net: true,
                show_disk: true,
                show_fans: true,
                show_cores: true,
                show_threads: false,
                sort: plottypus_core::ProcSort::Cpu,
                detail_pid: None,
                expanded: self.expanded,
                proc_ratio: 34,
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
        Surface,
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
    fn expand_cpu_hides_other_boxes() {
        let mut fx = fixture("");
        fx.expanded = Some(Panel::Cpu);
        fx.snap.cpu.active = 0.5;
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
        let text = paint(&fx.view(), 80, 24);
        assert!(text.contains("cpu"), "{text}");
        assert!(
            text.contains('×') || text.contains('x') || text.contains('X'),
            "{text}"
        );
        assert!(text.contains("busiest"), "{text}");
        assert!(text.contains("efficiency"), "{text}");
        assert!(text.contains("performance"), "{text}");
        assert!(text.contains("E0"), "{text}");
        assert!(text.contains("P0"), "{text}");
        assert!(text.contains("36°"), "{text}");
        assert!(text.contains("51°"), "{text}");
        assert!(text.contains("Xcode"), "{text}");
        assert!(!text.contains("Macintosh"), "{text}");
    }

    #[test]
    fn expand_gpu_shows_graphs_and_related() {
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
        fx.snap.processes[0].gpu = 77.0;
        let text = paint(&fx.view(), 80, 24);
        assert!(text.contains("gpu"), "{text}");
        assert!(text.contains("12%"), "{text}");
        assert!(text.contains("51°"), "{text}");
        assert!(text.contains("Xcode"), "{text}");
        assert!(text.contains("no per-process"), "{text}");
        assert!(!text.contains("77"), "{text}");
        assert!(!text.contains("Macintosh"), "{text}");
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

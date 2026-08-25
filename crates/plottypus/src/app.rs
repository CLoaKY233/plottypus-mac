use std::path::PathBuf;
use std::time::{Duration, Instant};

use plottypus_core::{
    Config, History, PROC_RATIO_MAX, PROC_RATIO_MIN, Result, Snapshot, Surface, auto_surface,
};
use plottypus_metrics::{Signal, send_signal};
use plottypus_ui::{
    AppView, DetailAction, Focus, Hit, LayoutFlags, Panel, ProcView, detail_actions, detail_rect,
    filtered_processes, hit_test, render_app,
};
use ratatui::Frame;
use ratatui::layout::Rect;

use crate::event::{self, Event};
use crate::tui::{self, AppTerminal};
use crate::worker::{self, Cmd, Handle};

pub fn run() -> Result<()> {
    tui::install_panic_hook();
    let mut terminal = tui::install()?;
    let (result, warning) = run_inner(&mut terminal);
    result.and(tui::restore())?;
    if let Some(warning) = warning {
        eprintln!("{warning}");
    }
    Ok(())
}

fn prefs_path() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))?;
    Some(base.join("plottypus").join("config.toml"))
}

fn run_inner(terminal: &mut AppTerminal) -> (Result<()>, Option<String>) {
    let mut app = match App::new() {
        Ok(app) => app,
        Err(err) => return (Err(err), None),
    };
    loop {
        tui::sync_begin();
        let draw_result = terminal.draw(|frame| app.draw(frame));
        tui::sync_end();
        if let Err(err) = draw_result {
            return (Err(plottypus_core::Error::terminal(err.to_string())), None);
        }
        let timeout = app.poll_timeout();
        let modes = event::Modes {
            searching: app.searching,
            settings: app.settings,
            expanded: app.expanded.is_some(),
            detail: app.detail_pid.is_some(),
        };
        match event::poll(timeout, modes) {
            Ok(Some(Event::Quit)) => break,
            Ok(Some(ev)) => {
                if !matches!(ev, Event::Tick) {
                    app.on_tick();
                }
                app.handle(ev);
            }
            Ok(None) => {}
            Err(err) => return (Err(err), None),
        }
    }
    (Ok(()), app.save_prefs())
}

struct App {
    config: Config,
    worker: Handle,
    snapshot: Snapshot,
    cpu_history: History,
    gpu_history: History,
    mem_history: History,
    net_rx_history: History,
    net_tx_history: History,
    disk_history: History,
    cpu_temp_history: History,
    gpu_temp_history: History,
    e_temp_history: History,
    p_temp_history: History,
    s_temp_history: History,
    surface: Surface,
    focus: Focus,
    expanded: Option<Panel>,
    proc: ProcView,
    help: bool,
    settings: bool,
    confirm_kill: bool,
    confirm_pid: Option<u32>,
    detail_pid: Option<u32>,
    searching: bool,
    frozen: bool,
    ready: bool,
    last_tick: Instant,
    status: Option<(String, Instant)>,
    dirty_prefs: bool,
    pending_signal: Signal,
    last_area: ratatui::layout::Rect,
    proc_ratio: u16,
    dragging_split: bool,
}

impl App {
    fn new() -> Result<Self> {
        let config = prefs_path().map_or_else(Config::default, |path| Config::load(&path));
        let proc_ratio = config.proc_ratio;
        let worker = worker::spawn(config.interval)?;
        let mut app = Self {
            config,
            worker,
            snapshot: Snapshot::empty(),
            cpu_history: History::default(),
            gpu_history: History::default(),
            mem_history: History::default(),
            net_rx_history: History::default(),
            net_tx_history: History::default(),
            disk_history: History::default(),
            cpu_temp_history: History::default(),
            gpu_temp_history: History::default(),
            e_temp_history: History::default(),
            p_temp_history: History::default(),
            s_temp_history: History::default(),
            surface: Surface::Work,
            focus: Focus::Processes,
            expanded: None,
            proc: ProcView::default(),
            help: false,
            settings: false,
            confirm_kill: false,
            confirm_pid: None,
            detail_pid: None,
            searching: false,
            frozen: false,
            ready: false,
            last_tick: Instant::now(),
            status: None,
            dirty_prefs: false,
            pending_signal: Signal::Term,
            last_area: ratatui::layout::Rect::new(0, 0, 80, 24),
            proc_ratio,
            dragging_split: false,
        };
        match app.worker.snaps.recv_timeout(Duration::from_secs(2)) {
            Ok(Ok(first)) => app.apply_snapshot(first),
            Ok(Err(err)) => return Err(err),
            Err(_) => {}
        }
        Ok(app)
    }

    fn touch(&mut self) {
        self.dirty_prefs = true;
    }

    fn save_prefs(&self) -> Option<String> {
        if !self.dirty_prefs {
            return None;
        }
        let path = prefs_path()?;
        match self.config.save(&path) {
            Ok(()) => None,
            Err(err) => Some(format!("could not save settings: {err}")),
        }
    }

    fn poll_timeout(&self) -> Duration {
        if self.frozen {
            return self.config.interval;
        }
        self.config
            .interval
            .saturating_sub(self.last_tick.elapsed())
    }

    fn tick(&mut self) {
        let mut latest = None;
        while let Ok(result) = self.worker.snaps.try_recv() {
            latest = Some(result);
        }
        match latest {
            Some(Ok(snapshot)) => self.apply_snapshot(snapshot),
            Some(Err(err)) => self.set_status(err.to_string()),
            None => {}
        }
    }

    fn apply_snapshot(&mut self, mut snapshot: Snapshot) {
        if snapshot.cpu.temp_c.is_none() {
            snapshot.cpu.temp_c = snapshot.sensors.cpu_c;
        }
        if let Some(gpu) = snapshot.gpu.as_mut()
            && gpu.temp_c.is_none()
        {
            gpu.temp_c = snapshot.sensors.gpu_c;
        }
        let keep_pid = self.proc.selected_pid;
        self.snapshot = snapshot;
        let snap = &self.snapshot;
        self.cpu_history.push(snap.cpu.scaled);
        let gpu = snap.gpu.map_or(0.0, |g| g.scaled);
        self.gpu_history.push(gpu);
        let mem_total = snap.memory.total_bytes;
        let mem = if mem_total == 0 {
            0.0
        } else {
            snap.memory.used_bytes as f32 / mem_total as f32
        };
        self.mem_history.push(mem);
        self.net_rx_history.push(snap.network.rx_bps as f32);
        self.net_tx_history.push(snap.network.tx_bps as f32);
        self.disk_history.push(snap.disk.used_ratio());
        self.cpu_temp_history
            .push(snap.cpu.temp_c.or(snap.sensors.best_cpu_c()).unwrap_or(0.0));
        self.gpu_temp_history.push(
            snap.gpu
                .and_then(|g| g.temp_c)
                .or(snap.sensors.gpu_c)
                .unwrap_or(0.0),
        );
        push_temp(&mut self.e_temp_history, snap.sensors.e_c);
        push_temp(&mut self.p_temp_history, snap.sensors.p_c);
        push_temp(&mut self.s_temp_history, snap.sensors.s_c);
        self.ready = true;
        self.last_tick = Instant::now();
        self.proc.selected_pid = keep_pid;
        self.sync_selection();
        if let Some(pid) = self.detail_pid
            && !self.snapshot.processes.iter().any(|p| p.pid == pid)
        {
            self.detail_pid = None;
        }
    }

    fn handle(&mut self, event: Event) {
        match event {
            Event::Tick => self.on_tick(),
            Event::Resize | Event::Quit => {}
            Event::Help => {
                self.help = !self.help;
                self.settings = false;
            }
            Event::FilterCancel => self.cancel_overlay(),
            _ if self.help => {}
            _ if self.confirm_kill => self.handle_confirm(event),
            _ if self.settings => self.handle_settings(event),
            _ => self.handle_normal(event),
        }
    }

    fn cancel_overlay(&mut self) {
        if self.help {
            self.help = false;
        } else if self.settings {
            self.settings = false;
        } else if self.confirm_kill {
            self.cancel_kill();
        } else if self.detail_pid.is_some() {
            self.detail_pid = None;
        } else if self.expanded.is_some() {
            self.expanded = None;
        } else if self.searching || !self.proc.filter.is_empty() {
            self.searching = false;
            self.proc.filter.clear();
            self.focus = Focus::Processes;
            self.sync_selection();
        }
    }

    fn handle_normal(&mut self, event: Event) {
        match event {
            Event::Settings => self.settings = true,
            Event::Glance => {
                self.expanded = None;
                self.lock_surface(Surface::Glance);
            }
            Event::Work => self.lock_surface(Surface::Work),
            Event::Expand => {
                if self.focus.panel() == Panel::Processes && self.expanded != Some(Panel::Processes)
                {
                    self.open_detail();
                } else {
                    self.expand_focus();
                }
            }
            Event::NextPanel => self.cycle_focus(1),
            Event::PrevPanel => self.cycle_focus(-1),
            Event::Search => {
                self.searching = !self.searching;
                self.focus = if self.searching {
                    Focus::Search
                } else {
                    Focus::Processes
                };
            }
            Event::FilterChar(c) => {
                self.searching = true;
                self.focus = Focus::Search;
                self.proc.filter.push(c);
                self.sync_selection();
            }
            Event::FilterBackspace => {
                self.proc.filter.pop();
                self.sync_selection();
            }
            Event::Move(delta) => self.handle_move(delta),
            Event::Kill => {
                if self.can_kill() {
                    self.pending_signal = Signal::Term;
                    self.arm_kill();
                }
            }
            Event::DetailTerm | Event::DetailKill | Event::DetailInterrupt => {
                if self.detail_pid.is_some() {
                    self.pending_signal = match event {
                        Event::DetailTerm => Signal::Term,
                        Event::DetailKill => Signal::Kill,
                        _ => Signal::Int,
                    };
                    self.arm_kill();
                }
            }
            Event::CycleInterval => {
                self.config.interval = self.config.cycle_interval();
                let _ = self.worker.cmds.send(Cmd::Interval(self.config.interval));
                self.touch();
            }
            Event::Freeze => {
                self.frozen = !self.frozen;
                let _ = self.worker.cmds.send(Cmd::Paused(self.frozen));
            }
            Event::ToggleGpu => {
                self.config.show_gpu = !self.config.show_gpu;
                self.touch();
            }
            Event::ToggleNet => {
                self.config.show_net = !self.config.show_net;
                self.touch();
            }
            Event::ToggleCores => {
                self.config.show_cores = !self.config.show_cores;
                self.touch();
            }
            Event::ToggleDisk => {
                self.config.show_disk = !self.config.show_disk;
                self.touch();
            }
            Event::ToggleFans => {
                self.config.show_fans = !self.config.show_fans;
                self.touch();
            }
            Event::ToggleThreads => {
                self.config.show_threads = !self.config.show_threads;
                self.touch();
            }
            Event::ToggleTree => {
                self.config.show_tree = !self.config.show_tree;
                self.touch();
            }
            Event::CycleSort => {
                self.config.proc_sort = self.config.proc_sort.next();
                self.touch();
            }
            Event::Click { col, row } => self.on_click(col, row),
            Event::Drag { col, .. } => self.drag_split(col),
            Event::MouseUp => self.dragging_split = false,
            _ => {}
        }
    }

    fn point_in(rect: Rect, col: u16, row: u16) -> bool {
        col >= rect.x
            && col < rect.x.saturating_add(rect.width)
            && row >= rect.y
            && row < rect.y.saturating_add(rect.height)
    }

    fn drag_split(&mut self, col: u16) {
        if !self.dragging_split || self.last_area.width < 40 {
            return;
        }
        let proc_w = self.last_area.width.saturating_sub(col.saturating_add(1));
        let ratio =
            u16::try_from(u32::from(proc_w) * 100 / u32::from(self.last_area.width)).unwrap_or(34);
        self.proc_ratio = ratio.clamp(PROC_RATIO_MIN, PROC_RATIO_MAX);
        self.touch();
    }

    fn handle_move(&mut self, delta: i32) {
        if self.expanded.is_some() && self.expanded != Some(Panel::Processes) {
            return;
        }
        if self.focus.panel() == Panel::Processes || self.expanded == Some(Panel::Processes) {
            self.searching = false;
            self.focus = Focus::Processes;
            self.move_sel(delta);
        } else {
            self.cycle_focus(delta.signum());
        }
    }

    fn handle_settings(&mut self, event: Event) {
        match event {
            Event::Settings => self.settings = false,
            Event::CycleInterval => {
                self.config.interval = self.config.cycle_interval();
                let _ = self.worker.cmds.send(Cmd::Interval(self.config.interval));
                self.touch();
            }
            Event::ToggleGpu => {
                self.config.show_gpu = !self.config.show_gpu;
                self.touch();
            }
            Event::ToggleNet => {
                self.config.show_net = !self.config.show_net;
                self.touch();
            }
            Event::ToggleCores => {
                self.config.show_cores = !self.config.show_cores;
                self.touch();
            }
            Event::ToggleDisk => {
                self.config.show_disk = !self.config.show_disk;
                self.touch();
            }
            Event::ToggleFans => {
                self.config.show_fans = !self.config.show_fans;
                self.touch();
            }
            Event::ToggleThreads => {
                self.config.show_threads = !self.config.show_threads;
                self.touch();
            }
            Event::ToggleTree => {
                self.config.show_tree = !self.config.show_tree;
                self.touch();
            }
            Event::CycleSort => {
                self.config.proc_sort = self.config.proc_sort.next();
                self.touch();
            }
            _ => {}
        }
    }

    fn open_detail(&mut self) {
        if let Some(pid) = self.selected_pid() {
            self.detail_pid = Some(pid);
            self.proc.selected_pid = Some(pid);
        }
    }

    fn on_click(&mut self, col: u16, row: u16) {
        let flags = self.layout_flags();
        if self.detail_pid.is_some()
            && let Some(proc_area) =
                plottypus_ui::inner_process_area(self.last_area, self.effective_surface(), flags)
        {
            for (rect, action) in detail_actions(proc_area) {
                if Self::point_in(rect, col, row) {
                    match action {
                        DetailAction::Term => self.handle(Event::DetailTerm),
                        DetailAction::Kill => self.handle(Event::DetailKill),
                        DetailAction::Interrupt => self.handle(Event::DetailInterrupt),
                        DetailAction::Close => self.handle(Event::FilterCancel),
                    }
                    return;
                }
            }
            let rect = detail_rect(proc_area);
            if Self::point_in(rect, col, row) {
                return;
            }
            self.detail_pid = None;
        }
        match hit_test(self.last_area, self.effective_surface(), flags, col, row) {
            Some(Hit::Search) => {
                self.searching = true;
                self.focus = Focus::Search;
            }
            Some(Hit::ProcRow(idx)) => {
                self.searching = false;
                self.focus = Focus::Processes;
                let rows = self.visible();
                if let Some(proc) = rows.get(idx) {
                    self.proc.selected_pid = Some(proc.pid);
                    self.proc.selected = idx;
                    self.detail_pid = Some(proc.pid);
                }
            }
            Some(Hit::Panel(panel)) => {
                self.focus = Focus::from_panel(panel);
            }
            Some(Hit::Expand(panel)) => {
                self.focus = Focus::from_panel(panel);
                self.expand_panel(panel);
            }
            Some(Hit::ExpandClose) => self.expanded = None,
            Some(Hit::Split) => self.dragging_split = true,
            Some(Hit::Help) => self.help = !self.help,
            Some(Hit::Settings) => self.settings = !self.settings,
            Some(Hit::Kill) => {
                if self.can_kill() {
                    self.arm_kill();
                }
            }
            Some(Hit::Quit) | None => {}
        }
    }

    fn layout_flags(&self) -> LayoutFlags {
        LayoutFlags {
            show_gpu: self.config.show_gpu,
            show_net: self.config.show_net,
            show_disk: self.config.show_disk,
            show_fans: self.config.show_fans,
            has_gpu: self.snapshot.gpu.is_some(),
            has_fans: self.snapshot.fans.is_present() || self.snapshot.sensors.is_present(),
            has_disk: !self.snapshot.disk.volumes.is_empty(),
            expanded: self.expanded,
            proc_ratio: self.proc_ratio,
        }
    }

    fn expand_focus(&mut self) {
        if self.expanded.is_some() {
            return;
        }
        self.expand_panel(self.focus.panel());
    }

    fn expand_panel(&mut self, panel: Panel) {
        self.expanded = Some(panel);
        if panel != Panel::Processes {
            self.searching = false;
        }
    }

    fn cycle_focus(&mut self, dir: i32) {
        let panels = self.layout_flags().visible_panels();
        if panels.is_empty() {
            return;
        }
        let current = self.focus.panel();
        let next = match panels.iter().position(|&p| p == current) {
            Some(i) if dir < 0 => i.checked_sub(1).unwrap_or(panels.len() - 1),
            Some(i) => (i + 1) % panels.len(),
            None if dir < 0 => panels.len() - 1,
            None => 0,
        };
        if let Some(&panel) = panels.get(next) {
            self.focus = Focus::from_panel(panel);
        }
    }

    fn can_kill(&self) -> bool {
        self.expanded.is_none() || self.expanded == Some(Panel::Processes)
    }

    fn arm_kill(&mut self) {
        self.confirm_pid = if self.detail_pid.is_some() {
            self.detail_pid
        } else {
            self.selected_pid()
        };
        self.confirm_kill = self.confirm_pid.is_some();
        if !self.confirm_kill {
            self.set_status(String::from("no process selected"));
        }
    }

    fn set_status(&mut self, message: String) {
        self.status = Some((message, Instant::now()));
    }

    fn live_status(&self) -> Option<&str> {
        let (message, at) = self.status.as_ref()?;
        if at.elapsed() < Duration::from_secs(4) {
            Some(message)
        } else {
            None
        }
    }

    fn cancel_kill(&mut self) {
        self.confirm_pid = None;
        self.confirm_kill = false;
    }

    fn handle_confirm(&mut self, event: Event) {
        match event {
            Event::ConfirmYes => {
                if let Some(pid) = self.confirm_pid.take() {
                    let signal = self.pending_signal;
                    match send_signal(pid, signal) {
                        Ok(()) => self.set_status(format!("sent {} to {pid}", signal.label())),
                        Err(err) => self.set_status(err.to_string()),
                    }
                }
                self.confirm_kill = false;
            }
            Event::ConfirmNo | Event::Kill => self.cancel_kill(),
            _ => {}
        }
    }

    fn on_tick(&mut self) {
        if !self.frozen && self.last_tick.elapsed() >= self.config.interval {
            self.tick();
        }
    }

    fn lock_surface(&mut self, surface: Surface) {
        if self.config.surface != Some(surface) {
            self.touch();
        }
        self.config.surface = Some(surface);
        self.surface = surface;
    }

    fn move_sel(&mut self, delta: i32) {
        let rows = self.visible();
        if rows.is_empty() {
            self.proc.selected = 0;
            self.proc.selected_pid = None;
            return;
        }
        let cur = rows
            .iter()
            .position(|p| Some(p.pid) == self.proc.selected_pid)
            .unwrap_or(self.proc.selected.min(rows.len() - 1));
        let next =
            (i32::try_from(cur).unwrap_or(0) + delta).clamp(0, (rows.len() - 1) as i32) as usize;
        self.proc.selected = next;
        self.proc.selected_pid = rows.get(next).map(|p| p.pid);
        if self.detail_pid.is_some() {
            self.detail_pid = self.proc.selected_pid;
        }
    }

    fn sync_selection(&mut self) {
        let rows = self.visible();
        if rows.is_empty() {
            self.proc.selected = 0;
            return;
        }
        if let Some(pid) = self.proc.selected_pid
            && let Some(i) = rows.iter().position(|p| p.pid == pid)
        {
            self.proc.selected = i;
            return;
        }
        self.proc.selected = self.proc.selected.min(rows.len() - 1);
        self.proc.selected_pid = rows.get(self.proc.selected).map(|p| p.pid);
    }

    fn visible(&self) -> Vec<plottypus_core::Process> {
        filtered_processes(&self.view())
    }

    fn selected_pid(&self) -> Option<u32> {
        self.proc
            .selected_pid
            .or_else(|| self.visible().get(self.proc.selected).map(|p| p.pid))
    }

    fn view(&self) -> AppView<'_> {
        AppView {
            snapshot: &self.snapshot,
            cpu_history: &self.cpu_history,
            gpu_history: &self.gpu_history,
            mem_history: &self.mem_history,
            net_rx_history: &self.net_rx_history,
            net_tx_history: &self.net_tx_history,
            disk_history: &self.disk_history,
            cpu_temp_history: &self.cpu_temp_history,
            gpu_temp_history: &self.gpu_temp_history,
            e_temp_history: &self.e_temp_history,
            p_temp_history: &self.p_temp_history,
            s_temp_history: &self.s_temp_history,
            surface: self.effective_surface(),
            degrade: plottypus_ui::Degrade::Full,
            focus: self.focus,
            proc: &self.proc,
            help: self.help,
            settings: self.settings,
            confirm_kill: self.confirm_kill,
            confirm_pid: self.confirm_pid,
            confirm_signal: self.pending_signal.label(),
            searching: self.searching,
            ready: self.ready,
            frozen: self.frozen,
            show_gpu: self.config.show_gpu,
            show_net: self.config.show_net,
            show_disk: self.config.show_disk,
            show_fans: self.config.show_fans,
            show_cores: self.config.show_cores,
            show_threads: self.config.show_threads,
            show_tree: self.config.show_tree,
            sort: self.config.proc_sort,
            detail_pid: self.detail_pid,
            expanded: self.expanded,
            proc_ratio: self.proc_ratio,
            interval_ms: u64::try_from(self.config.interval.as_millis()).unwrap_or(1000),
            status: self.live_status(),
        }
    }

    fn effective_surface(&self) -> Surface {
        self.config.surface.unwrap_or(self.surface)
    }

    fn draw(&mut self, frame: &mut Frame) {
        self.last_area = frame.area();
        if self.config.surface.is_none() {
            self.surface = auto_surface(self.last_area.width, self.last_area.height);
        }
        render_app(frame, &self.view());
    }
}

fn push_temp(history: &mut History, temp: Option<f32>) {
    if let Some(value) = temp {
        history.push(value);
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use plottypus_ui::{LayoutPlan, plan};

    #[test]
    fn new_app_loads() {
        let app = App::new().unwrap();
        assert!(app.ready, "first sample is applied during construction");
        assert!(app.expanded.is_none());
        assert!(!app.cpu_history.is_empty());
        assert!(!app.net_rx_history.is_empty());
    }

    #[test]
    fn has_fans_when_sensors_present() {
        let mut app = App::new().unwrap();
        app.snapshot.fans = plottypus_core::FanSnapshot::default();
        app.snapshot.sensors.cpu_c = Some(42.0);
        assert!(app.layout_flags().has_fans);
        assert!(app.view().flags().has_fans);
    }

    #[test]
    fn tick_stores_raw_history_values() {
        let mut app = App::new().unwrap();
        let mut snap = app.snapshot.clone();
        snap.cpu.active = 0.42;
        snap.cpu.scaled = 0.42;
        snap.network.rx_bps = 1_250;
        snap.network.tx_bps = 800;
        snap.memory.used_bytes = 8;
        snap.memory.total_bytes = 16;
        app.apply_snapshot(snap);
        assert_eq!(app.cpu_history.last(), Some(0.42));
        assert_eq!(app.net_rx_history.last(), Some(1_250.0));
        assert_eq!(app.net_tx_history.last(), Some(800.0));
        let mem = if app.snapshot.memory.total_bytes == 0 {
            0.0
        } else {
            app.snapshot.memory.used_bytes as f32 / app.snapshot.memory.total_bytes as f32
        };
        assert_eq!(app.mem_history.last(), Some(mem));
    }

    #[test]
    fn selection_follows_pid_after_resort() {
        let mut app = App::new().unwrap();
        app.snapshot.processes = vec![
            plottypus_core::Process {
                pid: 1,
                ppid: 1,
                name: String::from("low"),
                cpu: 1.0,
                mem_bytes: 1,
                threads: 1,
                gpu: 0.0,
                user: String::from("cloaky"),
                command: None,
                status: "sleeping",
                start_unix: 0,
                cpu_spark: Vec::new(),
            },
            plottypus_core::Process {
                pid: 2,
                ppid: 1,
                name: String::from("high"),
                cpu: 90.0,
                mem_bytes: 1,
                threads: 1,
                gpu: 0.0,
                user: String::from("cloaky"),
                command: None,
                status: "sleeping",
                start_unix: 0,
                cpu_spark: Vec::new(),
            },
        ];
        app.proc.selected_pid = Some(1);
        app.sync_selection();
        app.snapshot.processes[0].cpu = 99.0;
        app.snapshot.processes[1].cpu = 1.0;
        app.sync_selection();
        assert_eq!(app.proc.selected_pid, Some(1));
    }

    #[test]
    fn click_box_focuses_without_expand() {
        let mut app = App::new().unwrap();
        app.last_area = ratatui::layout::Rect::new(0, 0, 120, 30);
        let planned = plan(app.last_area, Surface::Work, app.layout_flags());
        let cpu = planned.cpu.unwrap_or_default();
        app.handle(Event::Click {
            col: cpu.x + 2,
            row: cpu.y + 1,
        });
        assert_eq!(app.focus, Focus::Cpu);
        assert!(app.expanded.is_none());
        let corner = LayoutPlan::corner_hit(cpu).unwrap_or_default();
        app.handle(Event::Click {
            col: corner.x,
            row: corner.y,
        });
        assert_eq!(app.expanded, Some(Panel::Cpu));
    }

    #[test]
    fn click_selected_process_opens_detail() {
        let mut app = App::new().unwrap();
        app.snapshot.processes = vec![plottypus_core::Process {
            pid: 7,
            ppid: 1,
            name: String::from("Xcode"),
            cpu: 10.0,
            mem_bytes: 1,
            threads: 1,
            gpu: 0.0,
            user: String::from("cloaky"),
            command: None,
            status: "sleeping",
            start_unix: 0,
            cpu_spark: Vec::new(),
        }];
        app.focus = Focus::Processes;
        app.last_area = ratatui::layout::Rect::new(0, 0, 120, 30);
        let planned = plan(app.last_area, Surface::Work, app.layout_flags());
        let proc = planned.processes.unwrap_or_default();
        app.handle(Event::Click {
            col: proc.x + 2,
            row: proc.y + 3,
        });
        assert_eq!(app.proc.selected_pid, Some(7));
        assert_eq!(app.detail_pid, Some(7));
        app.handle(Event::FilterCancel);
        assert!(app.detail_pid.is_none());
    }

    #[test]
    fn search_question_is_not_a_letter() {
        let mut app = App::new().unwrap();
        app.handle(Event::Search);
        assert!(app.searching);
        app.handle(Event::Help);
        assert!(app.help);
    }

    #[test]
    fn expand_collapse_and_next_panel() {
        let mut app = App::new().unwrap();
        app.focus = Focus::Cpu;
        assert!(app.expanded.is_none());

        app.handle(Event::Expand);
        assert_eq!(app.expanded, Some(Panel::Cpu));

        app.handle(Event::FilterCancel);
        assert!(app.expanded.is_none());

        app.handle(Event::Expand);
        assert_eq!(app.expanded, Some(Panel::Cpu));
        app.handle(Event::FilterCancel);
        assert!(app.expanded.is_none());

        let panels = app.view().flags().visible_panels();
        let idx = panels.iter().position(|&p| p == app.focus.panel()).unwrap();
        let expected = Focus::from_panel(panels[(idx + 1) % panels.len()]);
        app.handle(Event::NextPanel);
        assert_eq!(app.focus, expected);
        assert!(app.expanded.is_none());

        let focused = app.focus.panel();
        app.handle(Event::Expand);
        assert_eq!(app.expanded, Some(focused));
        app.focus = Focus::Processes;
        app.handle(Event::Move(1));
        app.handle(Event::Move(-1));
        assert_eq!(
            app.expanded,
            Some(focused),
            "wheel must not switch the expanded panel"
        );
    }

    #[test]
    fn kill_from_detail_targets_the_detailed_pid() {
        let mut app = App::new().unwrap();
        let mk = |pid: u32, name: &str, cpu: f32| plottypus_core::Process {
            pid,
            ppid: 1,
            name: name.to_owned(),
            cpu,
            mem_bytes: 1,
            threads: 1,
            gpu: 0.0,
            user: String::from("u"),
            command: None,
            status: "sleeping",
            start_unix: 0,
            cpu_spark: Vec::new(),
        };
        app.snapshot.processes = vec![mk(11, "target", 1.0), mk(22, "other", 9.0)];
        app.detail_pid = Some(11);
        app.handle(Event::DetailKill);
        assert!(app.confirm_kill);
        assert_eq!(
            app.confirm_pid,
            Some(11),
            "kill must target the detailed pid"
        );
        app.handle(Event::ConfirmNo);
        assert!(!app.confirm_kill);
    }

    #[test]
    fn open_detail_pins_selection() {
        let mut app = App::new().unwrap();
        let mk = |pid: u32, cpu: f32| plottypus_core::Process {
            pid,
            ppid: 1,
            name: format!("p{pid}"),
            cpu,
            mem_bytes: 1,
            threads: 1,
            gpu: 0.0,
            user: String::from("u"),
            command: None,
            status: "sleeping",
            start_unix: 0,
            cpu_spark: Vec::new(),
        };
        app.snapshot.processes = vec![mk(11, 1.0), mk(22, 9.0)];
        app.proc.selected_pid = Some(11);
        app.handle(Event::Expand);
        assert_eq!(app.detail_pid, Some(11));
        assert_eq!(app.proc.selected_pid, Some(11));
        app.handle(Event::Move(1));
        assert_eq!(app.detail_pid, app.proc.selected_pid);
    }
}

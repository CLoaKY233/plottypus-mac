use std::time::{Duration, Instant};

use plottypus_core::{Config, History, Result, Snapshot, Surface, auto_surface};
use plottypus_metrics::{Sampler, Signal, send_signal};
use plottypus_ui::{
    AppView, Focus, Hit, LayoutFlags, Panel, ProcView, filtered_processes, hit_test, render_app,
};
use ratatui::Frame;

use crate::event::{self, Event};
use crate::tui::{self, AppTerminal};

pub fn run() -> Result<()> {
    tui::install_panic_hook();
    let mut terminal = tui::install()?;
    let result = run_inner(&mut terminal);
    result.and(tui::restore())
}

fn run_inner(terminal: &mut AppTerminal) -> Result<()> {
    let mut app = App::new()?;
    loop {
        terminal.draw(|frame| app.draw(frame))?;
        let timeout = app.poll_timeout();
        match event::poll(timeout, app.searching, app.settings, app.expanded.is_some())? {
            Some(Event::Quit) => break,
            Some(ev) => {
                if !matches!(ev, Event::Tick) {
                    app.on_tick()?;
                }
                app.handle(ev)?;
            }
            None => {}
        }
    }
    Ok(())
}

struct App {
    config: Config,
    sampler: Sampler,
    snapshot: Snapshot,
    cpu_history: History,
    gpu_history: History,
    mem_history: History,
    net_rx_history: History,
    net_tx_history: History,
    disk_history: History,
    surface: Surface,
    focus: Focus,
    expanded: Option<Panel>,
    proc: ProcView,
    help: bool,
    settings: bool,
    confirm_kill: bool,
    confirm_pid: Option<u32>,
    searching: bool,
    frozen: bool,
    ready: bool,
    last_tick: Instant,
    status: Option<String>,
    last_area: ratatui::layout::Rect,
    proc_ratio: u16,
    dragging_split: bool,
}

impl App {
    fn new() -> Result<Self> {
        let mut sampler = Sampler::new()?;
        let snapshot = sampler.tick()?;
        Ok(Self {
            config: Config::default(),
            sampler,
            snapshot,
            cpu_history: History::default(),
            gpu_history: History::default(),
            mem_history: History::default(),
            net_rx_history: History::default(),
            net_tx_history: History::default(),
            disk_history: History::default(),
            surface: Surface::Work,
            focus: Focus::Processes,
            expanded: None,
            proc: ProcView::default(),
            help: false,
            settings: false,
            confirm_kill: false,
            confirm_pid: None,
            searching: false,
            frozen: false,
            ready: false,
            last_tick: Instant::now(),
            status: None,
            last_area: ratatui::layout::Rect::new(0, 0, 80, 24),
            proc_ratio: 34,
            dragging_split: false,
        })
    }

    fn poll_timeout(&self) -> Duration {
        if self.frozen {
            return self.config.interval;
        }
        self.config
            .interval
            .saturating_sub(self.last_tick.elapsed())
    }

    fn tick(&mut self) -> Result<()> {
        let keep_pid = self.proc.selected_pid;
        self.snapshot = self.sampler.tick()?;
        if self.snapshot.cpu.temp_c.is_none() {
            self.snapshot.cpu.temp_c = self.snapshot.sensors.cpu_c;
        }
        if let Some(gpu) = self.snapshot.gpu.as_mut()
            && gpu.temp_c.is_none()
        {
            gpu.temp_c = self.snapshot.sensors.gpu_c;
        }
        self.cpu_history.push(self.snapshot.cpu.active);
        let gpu = self.snapshot.gpu.map_or(0.0, |g| g.scaled);
        self.gpu_history.push(gpu);
        let mem_total = self.snapshot.memory.total_bytes;
        let mem = if mem_total == 0 {
            0.0
        } else {
            self.snapshot.memory.used_bytes as f32 / mem_total as f32
        };
        self.mem_history.push(mem);
        self.net_rx_history
            .push(self.snapshot.network.rx_bps as f32);
        self.net_tx_history
            .push(self.snapshot.network.tx_bps as f32);
        self.disk_history.push(self.snapshot.disk.used_ratio());
        self.ready = true;
        self.last_tick = Instant::now();
        self.proc.selected_pid = keep_pid;
        self.sync_selection();
        Ok(())
    }

    fn handle(&mut self, event: Event) -> Result<()> {
        match event {
            Event::Tick => return self.on_tick(),
            Event::Resize | Event::Quit => return Ok(()),
            Event::Help => {
                self.help = !self.help;
                self.settings = false;
                return Ok(());
            }
            Event::FilterCancel => {
                self.cancel_overlay();
                return Ok(());
            }
            _ => {}
        }
        if self.help {
            return Ok(());
        }
        if self.confirm_kill {
            self.handle_confirm(event);
            return Ok(());
        }
        if self.settings {
            self.handle_settings(event);
            return Ok(());
        }
        self.handle_normal(event);
        Ok(())
    }

    fn cancel_overlay(&mut self) {
        if self.help {
            self.help = false;
        } else if self.settings {
            self.settings = false;
        } else if self.confirm_kill {
            self.cancel_kill();
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
            Event::Expand => self.expand_focus(),
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
                    self.arm_kill();
                }
            }
            Event::CycleInterval => self.config.interval = self.config.cycle_interval(),
            Event::Freeze => self.frozen = !self.frozen,
            Event::ToggleGpu => self.config.show_gpu = !self.config.show_gpu,
            Event::ToggleNet => self.config.show_net = !self.config.show_net,
            Event::ToggleCores => self.config.show_cores = !self.config.show_cores,
            Event::ToggleDisk => self.config.show_disk = !self.config.show_disk,
            Event::ToggleFans => self.config.show_fans = !self.config.show_fans,
            Event::Click { col, row } => self.on_click(col, row),
            Event::Drag { col, .. } => self.drag_split(col),
            Event::MouseUp => self.dragging_split = false,
            _ => {}
        }
    }

    fn drag_split(&mut self, col: u16) {
        if !self.dragging_split || self.last_area.width < 40 {
            return;
        }
        let proc_w = self.last_area.width.saturating_sub(col.saturating_add(1));
        let ratio = u16::try_from(u32::from(proc_w) * 100 / u32::from(self.last_area.width))
            .unwrap_or(34);
        self.proc_ratio = ratio.clamp(22, 48);
    }

    fn handle_move(&mut self, delta: i32) {
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
            Event::CycleInterval => self.config.interval = self.config.cycle_interval(),
            Event::ToggleGpu => self.config.show_gpu = !self.config.show_gpu,
            Event::ToggleNet => self.config.show_net = !self.config.show_net,
            Event::ToggleCores => self.config.show_cores = !self.config.show_cores,
            Event::ToggleDisk => self.config.show_disk = !self.config.show_disk,
            Event::ToggleFans => self.config.show_fans = !self.config.show_fans,
            _ => {}
        }
    }

    fn on_click(&mut self, col: u16, row: u16) {
        let flags = self.layout_flags();
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
            if self.expanded.is_some() {
                self.expanded = Some(panel);
            }
        }
    }

    fn can_kill(&self) -> bool {
        self.expanded.is_none() || self.expanded == Some(Panel::Processes)
    }

    fn arm_kill(&mut self) {
        self.confirm_pid = self.selected_pid();
        self.confirm_kill = self.confirm_pid.is_some();
        if !self.confirm_kill {
            self.status = Some(String::from("no process selected"));
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
                    match send_signal(pid, Signal::Term) {
                        Ok(()) => self.status = Some(format!("sent TERM to {pid}")),
                        Err(err) => self.status = Some(err.to_string()),
                    }
                }
                self.confirm_kill = false;
            }
            Event::ConfirmNo | Event::Kill => self.cancel_kill(),
            _ => {}
        }
    }

    fn on_tick(&mut self) -> Result<()> {
        if self.frozen {
            return Ok(());
        }
        if self.last_tick.elapsed() >= self.config.interval {
            self.tick()?;
        }
        Ok(())
    }

    fn lock_surface(&mut self, surface: Surface) {
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
            surface: self.effective_surface(),
            focus: self.focus,
            proc: &self.proc,
            help: self.help,
            settings: self.settings,
            confirm_kill: self.confirm_kill,
            searching: self.searching,
            ready: self.ready,
            frozen: self.frozen,
            show_gpu: self.config.show_gpu,
            show_net: self.config.show_net,
            show_disk: self.config.show_disk,
            show_fans: self.config.show_fans,
            show_cores: self.config.show_cores,
            expanded: self.expanded,
            proc_ratio: self.proc_ratio,
            interval_ms: u64::try_from(self.config.interval.as_millis()).unwrap_or(1000),
            status: self.status.as_deref(),
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

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use plottypus_ui::{LayoutPlan, plan};

    #[test]
    fn new_app_loads() {
        let app = App::new().unwrap();
        assert!(!app.ready);
        assert!(app.expanded.is_none());
        assert!(app.cpu_history.is_empty());
        assert!(app.net_rx_history.is_empty());
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
        app.tick().unwrap();
        assert_eq!(app.cpu_history.last(), Some(app.snapshot.cpu.active));
        assert_eq!(
            app.net_rx_history.last(),
            Some(app.snapshot.network.rx_bps as f32)
        );
        assert_eq!(
            app.net_tx_history.last(),
            Some(app.snapshot.network.tx_bps as f32)
        );
        assert_eq!(
            app.disk_history.last(),
            Some(app.snapshot.disk.used_ratio())
        );
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
            },
            plottypus_core::Process {
                pid: 2,
                ppid: 1,
                name: String::from("high"),
                cpu: 90.0,
                mem_bytes: 1,
                threads: 1,
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
        })
        .unwrap();
        assert_eq!(app.focus, Focus::Cpu);
        assert!(app.expanded.is_none());
        let corner = LayoutPlan::corner_hit(cpu).unwrap_or_default();
        app.handle(Event::Click {
            col: corner.x,
            row: corner.y,
        })
        .unwrap();
        assert_eq!(app.expanded, Some(Panel::Cpu));
    }

    #[test]
    fn search_question_is_not_a_letter() {
        let mut app = App::new().unwrap();
        app.handle(Event::Search).unwrap();
        assert!(app.searching);
        app.handle(Event::Help).unwrap();
        assert!(app.help);
    }

    #[test]
    fn expand_collapse_and_next_panel() {
        let mut app = App::new().unwrap();
        app.focus = Focus::Cpu;
        assert!(app.expanded.is_none());

        app.handle(Event::Expand).unwrap();
        assert_eq!(app.expanded, Some(Panel::Cpu));

        app.handle(Event::FilterCancel).unwrap();
        assert!(app.expanded.is_none());

        app.handle(Event::Expand).unwrap();
        assert_eq!(app.expanded, Some(Panel::Cpu));
        app.handle(Event::FilterCancel).unwrap();
        assert!(app.expanded.is_none());

        let panels = app.view().flags().visible_panels();
        let idx = panels.iter().position(|&p| p == app.focus.panel()).unwrap();
        let expected = Focus::from_panel(panels[(idx + 1) % panels.len()]);
        app.handle(Event::NextPanel).unwrap();
        assert_eq!(app.focus, expected);
        assert!(app.expanded.is_none());
    }
}

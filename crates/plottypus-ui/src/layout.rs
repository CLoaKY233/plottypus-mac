use plottypus_core::{PROC_RATIO_MAX, PROC_RATIO_MIN, Surface};
use ratatui::layout::{Constraint, Layout, Rect};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Panel {
    Cpu,
    Gpu,
    Mem,
    Net,
    Disk,
    Fans,
    Processes,
}

impl Panel {
    pub const ALL: [Self; 7] = [
        Self::Cpu,
        Self::Gpu,
        Self::Mem,
        Self::Net,
        Self::Disk,
        Self::Fans,
        Self::Processes,
    ];

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Cpu => "cpu",
            Self::Gpu => "gpu",
            Self::Mem => "mem",
            Self::Net => "net",
            Self::Disk => "disk",
            Self::Fans => "fans",
            Self::Processes => "proc",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LayoutFlags {
    pub show_gpu: bool,
    pub show_net: bool,
    pub show_disk: bool,
    pub show_fans: bool,
    pub has_gpu: bool,
    pub has_fans: bool,
    pub has_disk: bool,
    pub expanded: Option<Panel>,
    pub proc_ratio: u16,
}

impl Default for LayoutFlags {
    fn default() -> Self {
        Self {
            show_gpu: true,
            show_net: true,
            show_disk: true,
            show_fans: true,
            has_gpu: true,
            has_fans: true,
            has_disk: true,
            expanded: None,
            proc_ratio: plottypus_core::PROC_RATIO_DEFAULT,
        }
    }
}

impl LayoutFlags {
    #[must_use]
    pub fn visible(self, panel: Panel) -> bool {
        match panel {
            Panel::Cpu | Panel::Mem | Panel::Processes => true,
            Panel::Gpu => self.show_gpu && self.has_gpu,
            Panel::Net => self.show_net,
            Panel::Disk => self.show_disk && self.has_disk,
            Panel::Fans => self.show_fans,
        }
    }

    #[must_use]
    pub fn visible_panels(self) -> Vec<Panel> {
        Panel::ALL
            .into_iter()
            .filter(|p| self.visible(*p))
            .collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Region {
    Cpu,
    Gpu,
    Mem,
    Net,
    Disk,
    Fans,
    Processes,
    Footer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Degrade {
    #[default]
    Full,
    Tight,
    Minimal,
}

impl Degrade {
    fn for_left_rail(width: u16, height: u16) -> Self {
        if width < 50 || height < 17 {
            Degrade::Minimal
        } else if width < 64 || height < 22 {
            Degrade::Tight
        } else {
            Degrade::Full
        }
    }

    fn for_glance(body: Rect) -> Self {
        if body.height < 12 || body.width < 50 {
            Degrade::Minimal
        } else if body.height < 18 {
            Degrade::Tight
        } else {
            Degrade::Full
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayoutPlan {
    pub surface: Surface,
    pub degrade: Degrade,
    pub cpu: Option<Rect>,
    pub gpu: Option<Rect>,
    pub mem: Option<Rect>,
    pub net: Option<Rect>,
    pub disk: Option<Rect>,
    pub fans: Option<Rect>,
    pub processes: Option<Rect>,
    pub split: Option<Rect>,
    pub footer: Rect,
    pub expanded: Option<Panel>,
}

impl LayoutPlan {
    #[must_use]
    pub fn region(&self, region: Region) -> Option<Rect> {
        match region {
            Region::Cpu => self.cpu,
            Region::Gpu => self.gpu,
            Region::Mem => self.mem,
            Region::Net => self.net,
            Region::Disk => self.disk,
            Region::Fans => self.fans,
            Region::Processes => self.processes,
            Region::Footer => Some(self.footer),
        }
    }

    #[must_use]
    pub fn panel(&self, panel: Panel) -> Option<Rect> {
        match panel {
            Panel::Cpu => self.cpu,
            Panel::Gpu => self.gpu,
            Panel::Mem => self.mem,
            Panel::Net => self.net,
            Panel::Disk => self.disk,
            Panel::Fans => self.fans,
            Panel::Processes => self.processes,
        }
    }

    #[must_use]
    pub fn corner_hit(area: Rect) -> Option<Rect> {
        if area.width < 5 || area.height == 0 {
            return None;
        }
        Some(Rect {
            x: area.x.saturating_add(area.width.saturating_sub(4)),
            y: area.y,
            width: 3,
            height: 1,
        })
    }

    #[must_use]
    pub fn close_hit(&self) -> Option<Rect> {
        let panel = self.expanded?;
        Self::corner_hit(self.panel(panel)?)
    }

    #[must_use]
    pub fn expand_hit(&self, panel: Panel) -> Option<Rect> {
        if self.expanded.is_some() {
            return None;
        }
        Self::corner_hit(self.panel(panel)?)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hit {
    Panel(Panel),
    Expand(Panel),
    ExpandClose,
    Search,
    ProcRow(usize),
    Split,
    Help,
    Settings,
    Kill,
    Quit,
}

/// Keep in sync with `plottypus_core::WORK_MIN_COLS` (24 proc + 36 metrics).
const WORK_MIN_WIDTH: u16 = 60;
const WORK_MIN_HEIGHT: u16 = 16;

#[must_use]
pub fn plan(area: Rect, surface: Surface, flags: LayoutFlags) -> LayoutPlan {
    let surface = match surface {
        Surface::Glance => Surface::Glance,
        Surface::Work if area.height < WORK_MIN_HEIGHT || area.width < WORK_MIN_WIDTH => {
            Surface::Glance
        }
        Surface::Work => Surface::Work,
    };

    let chunks = Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).split(area);
    let body = chunks[0];
    let footer = chunks[1];

    if let Some(panel) = flags.expanded {
        return expanded_plan(surface, body, footer, panel);
    }

    match surface {
        Surface::Glance => glance_plan(body, footer, flags),
        Surface::Work => work_plan(body, footer, flags),
    }
}

fn empty_plan(surface: Surface, degrade: Degrade, footer: Rect) -> LayoutPlan {
    LayoutPlan {
        surface,
        degrade,
        cpu: None,
        gpu: None,
        mem: None,
        net: None,
        disk: None,
        fans: None,
        processes: None,
        split: None,
        footer,
        expanded: None,
    }
}

fn expanded_plan(surface: Surface, body: Rect, footer: Rect, panel: Panel) -> LayoutPlan {
    let mut planned = empty_plan(surface, Degrade::Full, footer);
    planned.expanded = Some(panel);
    match panel {
        Panel::Cpu => planned.cpu = Some(body),
        Panel::Gpu => planned.gpu = Some(body),
        Panel::Mem => planned.mem = Some(body),
        Panel::Net => planned.net = Some(body),
        Panel::Disk => planned.disk = Some(body),
        Panel::Fans => planned.fans = Some(body),
        Panel::Processes => planned.processes = Some(body),
    }
    planned
}

fn glance_plan(body: Rect, footer: Rect, flags: LayoutFlags) -> LayoutPlan {
    let degrade = Degrade::for_glance(body);
    let show_gpu = flags.visible(Panel::Gpu);
    let show_mem = flags.visible(Panel::Mem);
    let mid = u16::from(show_gpu) + u16::from(show_mem);
    let io = u16::from(flags.visible(Panel::Net)) + u16::from(flags.visible(Panel::Disk));
    let fans = u16::from(flags.visible(Panel::Fans));

    let cpu_h = if body.height >= 10 { 5 } else { 4 };
    let mid_h = if mid > 0 && body.height >= cpu_h + 4 {
        3
    } else {
        0
    };
    let io_h = if io > 0 && body.height >= cpu_h + mid_h + 3 {
        3
    } else {
        0
    };
    let fan_h = if fans > 0 && body.height >= cpu_h + mid_h + io_h + 3 {
        3
    } else {
        0
    };

    let rows = Layout::vertical([
        Constraint::Length(cpu_h),
        Constraint::Length(mid_h),
        Constraint::Length(io_h),
        Constraint::Length(fan_h),
        Constraint::Fill(1),
    ])
    .split(body);

    let mut planned = empty_plan(Surface::Glance, degrade, footer);
    planned.cpu = Some(rows[0]);
    if mid_h > 0 {
        assign_row(
            &mut planned,
            rows[1],
            [Panel::Gpu, Panel::Mem]
                .into_iter()
                .filter(|p| flags.visible(*p))
                .collect(),
        );
    }
    if io_h > 0 {
        assign_row(&mut planned, rows[2], io_panels(flags));
    }
    if fan_h > 0 {
        planned.fans = Some(rows[3]);
    }
    planned
}

fn work_plan(body: Rect, footer: Rect, flags: LayoutFlags) -> LayoutPlan {
    let left_w = body
        .width
        .saturating_sub(body.width * flags.proc_ratio.clamp(PROC_RATIO_MIN, PROC_RATIO_MAX) / 100);
    let degrade = Degrade::for_left_rail(left_w, body.height);
    let ratio = flags.proc_ratio.clamp(PROC_RATIO_MIN, PROC_RATIO_MAX);
    let proc_w = (u32::from(body.width) * u32::from(ratio) / 100) as u16;
    // keeps the clamp range from inverting below the gate
    let proc_w = proc_w.clamp(24, body.width.saturating_sub(36).max(24));
    let cols = Layout::horizontal([
        Constraint::Fill(1),
        Constraint::Length(1),
        Constraint::Length(proc_w),
    ])
    .split(body);
    let left = cols[0];
    let split = cols[1];
    let right = cols[2];

    let hero_on = !hero_panels(flags).is_empty();
    let mid_on = !mid_panels(flags).is_empty();
    let io_on = !io_panels(flags).is_empty();
    let mut weights = Vec::new();
    if hero_on {
        weights.push(Constraint::Fill(5));
    }
    if mid_on {
        weights.push(Constraint::Fill(3));
    }
    if io_on {
        weights.push(Constraint::Fill(3));
    }
    if weights.is_empty() {
        weights.push(Constraint::Fill(1));
    }
    let rows = Layout::vertical(weights).split(left);

    let mut planned = empty_plan(Surface::Work, degrade, footer);
    let mut idx = 0;
    if hero_on {
        assign_row(&mut planned, rows[idx], hero_panels(flags));
        idx += 1;
    }
    if mid_on {
        assign_row(&mut planned, rows[idx], mid_panels(flags));
        idx += 1;
    }
    if io_on {
        assign_row(&mut planned, rows[idx], io_panels(flags));
    }
    planned.split = Some(split);
    planned.processes = Some(right);
    planned
}

fn hero_panels(flags: LayoutFlags) -> Vec<Panel> {
    [Panel::Cpu, Panel::Gpu]
        .into_iter()
        .filter(|p| flags.visible(*p))
        .collect()
}

fn mid_panels(flags: LayoutFlags) -> Vec<Panel> {
    [Panel::Mem, Panel::Fans]
        .into_iter()
        .filter(|p| flags.visible(*p))
        .collect()
}

fn io_panels(flags: LayoutFlags) -> Vec<Panel> {
    [Panel::Net, Panel::Disk]
        .into_iter()
        .filter(|p| flags.visible(*p))
        .collect()
}

fn assign_row(planned: &mut LayoutPlan, area: Rect, panels: Vec<Panel>) {
    if panels.is_empty() || area.width == 0 || area.height == 0 {
        return;
    }
    let constraints: Vec<Constraint> = panels.iter().map(|_| Constraint::Fill(1)).collect();
    let cols = Layout::horizontal(constraints).split(area);
    for (panel, rect) in panels.into_iter().zip(cols.iter().copied()) {
        set_panel(planned, panel, rect);
    }
}

fn set_panel(planned: &mut LayoutPlan, panel: Panel, rect: Rect) {
    match panel {
        Panel::Cpu => planned.cpu = Some(rect),
        Panel::Gpu => planned.gpu = Some(rect),
        Panel::Mem => planned.mem = Some(rect),
        Panel::Net => planned.net = Some(rect),
        Panel::Disk => planned.disk = Some(rect),
        Panel::Fans => planned.fans = Some(rect),
        Panel::Processes => planned.processes = Some(rect),
    }
}

#[must_use]
pub fn hit_test(
    area: Rect,
    surface: Surface,
    flags: LayoutFlags,
    col: u16,
    row: u16,
) -> Option<Hit> {
    let planned = plan(area, surface, flags);
    if row == planned.footer.y {
        let x = col.saturating_sub(planned.footer.x);
        return Some(if x < 12 {
            Hit::Help
        } else if x < 24 {
            Hit::Search
        } else if x < 36 {
            Hit::Kill
        } else if x < 50 {
            Hit::Settings
        } else {
            Hit::Quit
        });
    }
    if let Some(close) = planned.close_hit()
        && contains(close, col, row)
    {
        return Some(Hit::ExpandClose);
    }
    if planned.expanded.is_none() {
        for panel in Panel::ALL {
            if let Some(corner) = planned.expand_hit(panel)
                && contains(corner, col, row)
            {
                return Some(Hit::Expand(panel));
            }
        }
    }
    if let Some(split) = planned.split
        && contains(split, col, row)
    {
        return Some(Hit::Split);
    }
    if let Some(proc) = planned.processes
        && contains(proc, col, row)
    {
        if flags.expanded.is_none() && row == proc.y {
            return Some(Hit::Panel(Panel::Processes));
        }
        if row == proc.y.saturating_add(1) {
            return Some(Hit::Search);
        }
        if row > proc.y.saturating_add(2) && row < proc.y.saturating_add(proc.height) {
            let idx = usize::from(row.saturating_sub(proc.y.saturating_add(3)));
            return Some(Hit::ProcRow(idx));
        }
        return Some(Hit::Panel(Panel::Processes));
    }
    for panel in Panel::ALL {
        if panel == Panel::Processes {
            continue;
        }
        if let Some(rect) = planned.panel(panel)
            && contains(rect, col, row)
        {
            return Some(Hit::Panel(panel));
        }
    }
    None
}

fn contains(rect: Rect, col: u16, row: u16) -> bool {
    col >= rect.x
        && col < rect.x.saturating_add(rect.width)
        && row >= rect.y
        && row < rect.y.saturating_add(rect.height)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flags() -> LayoutFlags {
        LayoutFlags {
            show_gpu: true,
            show_net: true,
            show_disk: true,
            show_fans: true,
            has_gpu: true,
            has_fans: true,
            has_disk: true,
            expanded: None,
            proc_ratio: 55,
        }
    }

    #[test]
    fn degrade_ladder_boundaries() {
        // left rail = width * (1 - 55%)
        let cases = [
            (120, 30, Degrade::Tight),   // left 54
            (145, 30, Degrade::Full),    // left 65
            (100, 30, Degrade::Minimal), // left 45
            (160, 16, Degrade::Minimal), // body is 15 tall here
            (160, 18, Degrade::Tight),   // body 17: width ok, height short
            (170, 23, Degrade::Full),    // body 22 clears both bands
        ];
        for (width, height, want) in cases {
            let planned = plan(Rect::new(0, 0, width, height), Surface::Work, flags());
            assert_eq!(planned.degrade, want, "{width}x{height}");
        }
        let mut narrow = flags();
        narrow.proc_ratio = 35;
        let planned = plan(Rect::new(0, 0, 120, 30), Surface::Work, narrow);
        assert_eq!(planned.degrade, Degrade::Full);
    }

    #[test]
    fn expanded_views_never_degrade() {
        let mut fs = flags();
        fs.expanded = Some(Panel::Cpu);
        let planned = plan(Rect::new(0, 0, 70, 20), Surface::Work, fs);
        assert_eq!(planned.degrade, Degrade::Full);
    }

    #[test]
    fn work_separates_metric_boxes() {
        let planned = plan(Rect::new(0, 0, 140, 40), Surface::Work, flags());
        assert_eq!(planned.surface, Surface::Work);
        let cpu = planned.cpu.unwrap_or_default();
        let proc = planned.processes.unwrap_or_default();
        let gpu = planned.gpu.unwrap_or_default();
        let mem = planned.mem.unwrap_or_default();
        let fans = planned.fans.unwrap_or_default();
        let net = planned.net.unwrap_or_default();
        let disk = planned.disk.unwrap_or_default();
        assert!(cpu.height >= 8, "cpu {cpu:?}");
        assert_eq!(cpu.y, gpu.y, "cpu and gpu are equal hero tiles");
        assert_eq!(cpu.height, gpu.height);
        assert!(cpu.x < gpu.x);
        assert!(cpu.width + proc.width < 140);
        assert!(
            proc.height >= 20,
            "proc should be full left-column height {proc:?}"
        );
        assert!(proc.x > cpu.x + cpu.width, "proc on the right");
        assert!(planned.split.is_some());
        assert!(mem.height >= 5 && fans.height >= 5);
        assert_eq!(mem.y, fans.y);
        assert!(mem.x < fans.x);
        assert_eq!(net.y, disk.y);
        assert!(net.x < disk.x);
        assert!(cpu.y + cpu.height <= mem.y);
        assert!(mem.y + mem.height <= net.y);
        assert!(cpu.height > mem.height, "heroes taller than mem/sens");
        assert_eq!(proc.y, cpu.y);
        assert_eq!(planned.footer.height, 1);
        assert!(planned.expanded.is_none());
    }

    #[test]
    fn midsize_work_keeps_boxes() {
        let planned = plan(Rect::new(0, 0, 80, 24), Surface::Work, flags());
        assert_eq!(planned.surface, Surface::Work);
        assert!(planned.cpu.is_some());
        assert!(planned.gpu.is_some());
        assert!(planned.mem.is_some());
        assert!(planned.fans.is_some());
        assert!(planned.net.is_some());
        assert!(planned.disk.is_some());
        assert!(planned.processes.is_some());
        let cpu = planned.cpu.unwrap_or_default();
        let proc = planned.processes.unwrap_or_default();
        let gpu = planned.gpu.unwrap_or_default();
        assert!(cpu.height >= 6);
        assert!(proc.height >= 5);
        assert_eq!(cpu.height, gpu.height);
        assert!(cpu.height + 2 < 24, "cpu must not eat the screen");
    }

    #[test]
    fn hidden_flags_drop_boxes() {
        let mut flags = flags();
        flags.show_gpu = false;
        flags.show_fans = false;
        flags.show_disk = false;
        let planned = plan(Rect::new(0, 0, 120, 30), Surface::Work, flags);
        assert!(planned.gpu.is_none());
        assert!(planned.fans.is_none());
        assert!(planned.disk.is_none());
        assert!(planned.mem.is_some());
        assert!(planned.net.is_some());
    }

    #[test]
    fn expand_fills_body() {
        let mut flags = flags();
        flags.expanded = Some(Panel::Cpu);
        let planned = plan(Rect::new(0, 0, 80, 24), Surface::Work, flags);
        let cpu = planned.cpu.unwrap_or_default();
        assert_eq!(cpu, Rect::new(0, 0, 80, 23));
        assert!(planned.gpu.is_none());
        assert!(planned.processes.is_none());
        assert_eq!(planned.expanded, Some(Panel::Cpu));
        assert!(planned.close_hit().is_some());
    }

    #[test]
    fn glance_has_no_process_pane() {
        let planned = plan(Rect::new(0, 0, 80, 20), Surface::Glance, flags());
        assert_eq!(planned.surface, Surface::Glance);
        assert!(planned.processes.is_none());
        assert!(planned.cpu.is_some());
        assert_eq!(planned.footer.height, 1);
        assert_eq!(planned.region(Region::Processes), None);
        assert_eq!(planned.region(Region::Cpu), planned.cpu);
    }

    #[test]
    fn tiny_work_falls_back_to_glance() {
        let planned = plan(Rect::new(0, 0, 40, 8), Surface::Work, flags());
        assert_eq!(planned.surface, Surface::Glance);
        assert!(planned.processes.is_none());
    }

    #[test]
    fn work_between_gates_falls_back_instead_of_panicking() {
        // Regression: widths 40..59 used to reach work_plan and invert the
        // process-column clamp (min 24 > max width-36).
        for width in 40..60_u16 {
            for height in [16_u16, 20, 24, 31] {
                let planned = plan(Rect::new(0, 0, width, height), Surface::Work, flags());
                assert_eq!(
                    planned.surface,
                    Surface::Glance,
                    "{width}x{height} must be Glance"
                );
            }
        }
    }

    #[test]
    fn plan_and_hit_test_sweep_every_size_without_panic() {
        for width in (1..=120_u16).step_by(3) {
            for height in (1..=44_u16).step_by(3) {
                let area = Rect::new(0, 0, width, height);
                let mut fs = flags();
                let _ = plan(area, Surface::Work, fs);
                let _ = plan(area, Surface::Glance, fs);
                let _ = hit_test(area, Surface::Work, fs, width / 2, height / 2);
                for panel in Panel::ALL {
                    fs.expanded = Some(panel);
                    let planned = plan(area, Surface::Work, fs);
                    if let Some(rect) = planned.panel(panel) {
                        let _ = hit_test(area, Surface::Work, fs, rect.x, rect.y);
                    }
                }
            }
        }
    }

    #[test]
    fn click_box_hits_panel() {
        let area = Rect::new(0, 0, 120, 30);
        let planned = plan(area, Surface::Work, flags());
        let cpu = planned.cpu.unwrap_or_default();
        assert_eq!(
            hit_test(area, Surface::Work, flags(), cpu.x + 2, cpu.y + 1),
            Some(Hit::Panel(Panel::Cpu))
        );
        let corner = LayoutPlan::corner_hit(cpu).unwrap_or_default();
        assert_eq!(
            hit_test(area, Surface::Work, flags(), corner.x, corner.y),
            Some(Hit::Expand(Panel::Cpu))
        );
        let mem = planned.mem.unwrap_or_default();
        assert_eq!(
            hit_test(area, Surface::Work, flags(), mem.x + 2, mem.y + 1),
            Some(Hit::Panel(Panel::Mem))
        );
    }

    #[test]
    fn click_x_closes_expand() {
        let mut flags = flags();
        flags.expanded = Some(Panel::Mem);
        let area = Rect::new(0, 0, 80, 24);
        let planned = plan(area, Surface::Work, flags);
        let close = planned.close_hit().unwrap_or_default();
        assert_eq!(
            hit_test(area, Surface::Work, flags, close.x, close.y),
            Some(Hit::ExpandClose)
        );
    }

    #[test]
    fn visible_panels_respect_hardware() {
        let mut flags = flags();
        flags.has_fans = false;
        flags.has_gpu = false;
        flags.show_fans = false;
        let vis = flags.visible_panels();
        assert!(!vis.contains(&Panel::Fans));
        assert!(!vis.contains(&Panel::Gpu));
        assert!(vis.contains(&Panel::Cpu));
        assert!(vis.contains(&Panel::Processes));
    }
}

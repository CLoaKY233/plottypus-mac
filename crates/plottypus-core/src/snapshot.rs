use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClusterKind {
    Efficiency,
    Performance,
    Super,
}

impl ClusterKind {
    pub const ALL: [Self; 3] = [Self::Efficiency, Self::Performance, Self::Super];

    #[must_use]
    pub const fn tag(self) -> &'static str {
        match self {
            Self::Efficiency => "E",
            Self::Performance => "P",
            Self::Super => "S",
        }
    }

    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Self::Efficiency => "efficiency",
            Self::Performance => "performance",
            Self::Super => "super",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cluster {
    pub kind: ClusterKind,
    pub scaled: f32,
    pub active: f32,
    pub freq_mhz: u32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CoreSample {
    pub kind: ClusterKind,
    pub index: u16,
    pub scaled: f32,
    pub active: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CpuSnapshot {
    pub scaled: f32,
    pub active: f32,
    pub watts: Option<f32>,
    pub freq_mhz: Option<u32>,
    pub temp_c: Option<f32>,
    pub e_cluster: Option<Cluster>,
    pub p_cluster: Option<Cluster>,
    pub s_cluster: Option<Cluster>,
    pub cores: Vec<CoreSample>,
}

impl Default for CpuSnapshot {
    fn default() -> Self {
        Self {
            scaled: 0.0,
            active: 0.0,
            watts: None,
            freq_mhz: None,
            temp_c: None,
            e_cluster: None,
            p_cluster: None,
            s_cluster: None,
            cores: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Pressure {
    #[default]
    Nominal,
    Warn,
    Critical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Thermal {
    #[default]
    Nominal,
    Fair,
    Serious,
    Critical,
}

impl Thermal {
    #[must_use]
    pub const fn is_nominal(self) -> bool {
        matches!(self, Self::Nominal)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MemorySnapshot {
    pub used_bytes: u64,
    pub total_bytes: u64,
    pub wired_bytes: u64,
    pub compressed_bytes: u64,
    pub cache_bytes: u64,
    pub swap_used_bytes: u64,
    pub swap_total_bytes: u64,
    pub pressure: Pressure,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct NetworkSnapshot {
    pub iface: String,
    pub rx_bps: u64,
    pub tx_bps: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiskVolume {
    pub name: String,
    pub mount: String,
    pub used_bytes: u64,
    pub total_bytes: u64,
}

impl DiskVolume {
    #[must_use]
    pub fn ratio(&self) -> f32 {
        if self.total_bytes == 0 {
            0.0
        } else {
            self.used_bytes as f32 / self.total_bytes as f32
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct DiskSnapshot {
    pub volumes: Vec<DiskVolume>,
    pub read_bps: u64,
    pub write_bps: u64,
}

impl DiskSnapshot {
    #[must_use]
    pub fn primary(&self) -> Option<&DiskVolume> {
        self.volumes
            .iter()
            .find(|v| v.mount == "/")
            .or_else(|| self.volumes.first())
    }

    #[must_use]
    pub fn used_ratio(&self) -> f32 {
        self.primary().map_or(0.0, DiskVolume::ratio)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FanMetric {
    pub name: String,
    pub rpm: u16,
    pub max_rpm: u16,
}

impl FanMetric {
    #[must_use]
    pub fn ratio(&self) -> f32 {
        if self.max_rpm == 0 {
            0.0
        } else {
            f32::from(self.rpm) / f32::from(self.max_rpm)
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TempReading {
    pub name: String,
    pub celsius: f32,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct SensorsSnapshot {
    pub cpu_c: Option<f32>,
    pub gpu_c: Option<f32>,
    pub hotspot_c: Option<f32>,
    pub e_c: Option<f32>,
    pub p_c: Option<f32>,
    pub s_c: Option<f32>,
    pub readings: Vec<TempReading>,
}

impl SensorsSnapshot {
    #[must_use]
    pub fn is_present(&self) -> bool {
        self.cpu_c.is_some()
            || self.gpu_c.is_some()
            || self.e_c.is_some()
            || self.p_c.is_some()
            || self.s_c.is_some()
            || !self.readings.is_empty()
    }

    #[must_use]
    pub fn zone_temp(&self, kind: ClusterKind) -> Option<f32> {
        match kind {
            ClusterKind::Efficiency => self.e_c,
            ClusterKind::Performance => self.p_c,
            ClusterKind::Super => self.s_c,
        }
    }

    #[must_use]
    pub fn best_cpu_c(&self) -> Option<f32> {
        self.cpu_c
            .or_else(|| mean_opts(&[self.e_c, self.p_c, self.s_c]))
            .or(self.hotspot_c)
    }

    pub fn set_zone_temp(&mut self, kind: ClusterKind, temp: f32) {
        match kind {
            ClusterKind::Efficiency => self.e_c = Some(temp),
            ClusterKind::Performance => self.p_c = Some(temp),
            ClusterKind::Super => self.s_c = Some(temp),
        }
    }
}

fn mean_opts(values: &[Option<f32>]) -> Option<f32> {
    let mut sum = 0.0_f32;
    let mut n = 0_u32;
    for value in values.iter().flatten() {
        sum += *value;
        n += 1;
    }
    if n == 0 { None } else { Some(sum / n as f32) }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct FanSnapshot {
    pub fans: Vec<FanMetric>,
}

impl FanSnapshot {
    #[must_use]
    pub fn is_present(&self) -> bool {
        self.fans.iter().any(|f| f.rpm > 0 || f.max_rpm > 0)
    }

    #[must_use]
    pub fn peak_ratio(&self) -> f32 {
        self.fans
            .iter()
            .map(FanMetric::ratio)
            .fold(0.0_f32, f32::max)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct GpuSnapshot {
    pub scaled: f32,
    pub active: f32,
    pub watts: Option<f32>,
    pub freq_mhz: Option<u32>,
    pub ane_watts: Option<f32>,
    pub temp_c: Option<f32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SocInfo {
    pub name: String,
    pub e_cores: u8,
    pub p_cores: u8,
    pub s_cores: u8,
    pub gpu_cores: u8,
    pub memory_bytes: u64,
    /// Mach index → cluster, from `IORegistry` `cluster-type`. Empty if unknown.
    pub core_kinds: Vec<ClusterKind>,
}

impl Default for SocInfo {
    fn default() -> Self {
        Self {
            name: String::from("unknown"),
            e_cores: 0,
            p_cores: 0,
            s_cores: 0,
            gpu_cores: 0,
            memory_bytes: 0,
            core_kinds: Vec::new(),
        }
    }
}

impl SocInfo {
    /// Cluster and local index for Mach core `mach_index`.
    /// Prefers `core_kinds`. Falls back to E then P then S when counts cover `total`.
    #[must_use]
    pub fn role_for(&self, mach_index: usize, total: usize) -> Option<(ClusterKind, u16)> {
        role_for_core(
            &self.core_kinds,
            self.e_cores,
            self.p_cores,
            self.s_cores,
            mach_index,
            total,
        )
    }
}

/// Apple Silicon Mach order when `IORegistry` is missing: E, then P, then S.
#[must_use]
pub fn linear_core_role(
    e_cores: u8,
    p_cores: u8,
    s_cores: u8,
    mach_index: usize,
    total: usize,
) -> Option<(ClusterKind, u16)> {
    let e = usize::from(e_cores);
    let p = usize::from(p_cores);
    let s = usize::from(s_cores);
    if e + p + s != total || mach_index >= total {
        return None;
    }
    if mach_index < e {
        Some((ClusterKind::Efficiency, mach_index as u16))
    } else if mach_index < e + p {
        Some((ClusterKind::Performance, (mach_index - e) as u16))
    } else {
        Some((ClusterKind::Super, (mach_index - e - p) as u16))
    }
}

#[must_use]
pub fn role_for_core(
    core_kinds: &[ClusterKind],
    e_cores: u8,
    p_cores: u8,
    s_cores: u8,
    mach_index: usize,
    total: usize,
) -> Option<(ClusterKind, u16)> {
    if total == 0 || mach_index >= total {
        return None;
    }
    if core_kinds.len() == total {
        let kind = *core_kinds.get(mach_index)?;
        let index = core_kinds
            .iter()
            .take(mach_index)
            .filter(|k| **k == kind)
            .count() as u16;
        return Some((kind, index));
    }
    linear_core_role(e_cores, p_cores, s_cores, mach_index, total)
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Process {
    pub pid: u32,
    pub ppid: u32,
    pub name: String,
    pub cpu: f32,
    pub mem_bytes: u64,
    pub threads: u32,
    /// Share of GPU if known; 0 means unmeasured (macOS has no cheap per-pid GPU %).
    pub gpu: f32,
    pub user: String,
    pub command: Option<String>,
    pub status: &'static str,
    pub start_unix: i64,
    /// Recent per-pid CPU % samples (0..=100), oldest first.
    pub cpu_spark: Vec<f32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Snapshot {
    pub collected_at: Instant,
    pub soc: SocInfo,
    pub cpu: CpuSnapshot,
    pub memory: MemorySnapshot,
    pub gpu: Option<GpuSnapshot>,
    pub network: NetworkSnapshot,
    pub disk: DiskSnapshot,
    pub fans: FanSnapshot,
    pub sensors: SensorsSnapshot,
    pub processes: Vec<Process>,
    pub thermal: Thermal,
    pub status: Option<String>,
}

impl Snapshot {
    #[must_use]
    pub fn empty() -> Self {
        Self {
            collected_at: Instant::now(),
            soc: SocInfo::default(),
            cpu: CpuSnapshot::default(),
            memory: MemorySnapshot::default(),
            gpu: None,
            network: NetworkSnapshot::default(),
            disk: DiskSnapshot::default(),
            fans: FanSnapshot::default(),
            sensors: SensorsSnapshot::default(),
            processes: Vec::new(),
            thermal: Thermal::Nominal,
            status: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_snapshot_is_idle() {
        let snap = Snapshot::empty();
        assert!(snap.thermal.is_nominal());
        assert!(snap.processes.is_empty());
        assert!(snap.gpu.is_none());
        assert!(snap.disk.volumes.is_empty());
        assert!(snap.fans.fans.is_empty());
        assert!((snap.cpu.scaled - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn disk_primary_prefers_root() {
        let snap = DiskSnapshot {
            volumes: vec![
                DiskVolume {
                    name: String::from("Data"),
                    mount: String::from("/System/Volumes/Data"),
                    used_bytes: 10,
                    total_bytes: 100,
                },
                DiskVolume {
                    name: String::from("Macintosh HD"),
                    mount: String::from("/"),
                    used_bytes: 40,
                    total_bytes: 100,
                },
            ],
            read_bps: 0,
            write_bps: 0,
        };
        assert_eq!(snap.primary().map(|v| v.mount.as_str()), Some("/"));
        assert!((snap.used_ratio() - 0.4).abs() < f32::EPSILON);
    }

    #[test]
    fn fanless_is_absent() {
        assert!(!FanSnapshot::default().is_present());
        let fans = FanSnapshot {
            fans: vec![FanMetric {
                name: String::from("Fan 1"),
                rpm: 1800,
                max_rpm: 6000,
            }],
        };
        assert!(fans.is_present());
        assert!((fans.peak_ratio() - 0.3).abs() < f32::EPSILON);
    }

    #[test]
    fn linear_roles_are_e_then_p_then_s() {
        assert_eq!(
            linear_core_role(4, 8, 0, 0, 12),
            Some((ClusterKind::Efficiency, 0))
        );
        assert_eq!(
            linear_core_role(4, 8, 0, 3, 12),
            Some((ClusterKind::Efficiency, 3))
        );
        assert_eq!(
            linear_core_role(4, 8, 0, 4, 12),
            Some((ClusterKind::Performance, 0))
        );
        assert_eq!(
            linear_core_role(0, 12, 6, 11, 18),
            Some((ClusterKind::Performance, 11))
        );
        assert_eq!(
            linear_core_role(0, 12, 6, 12, 18),
            Some((ClusterKind::Super, 0))
        );
        assert_eq!(linear_core_role(4, 8, 0, 0, 10), None);
    }

    #[test]
    fn registry_kinds_win_over_linear() {
        let kinds = vec![
            ClusterKind::Performance,
            ClusterKind::Performance,
            ClusterKind::Super,
        ];
        assert_eq!(
            role_for_core(&kinds, 4, 8, 0, 2, 3),
            Some((ClusterKind::Super, 0))
        );
        assert_eq!(
            role_for_core(&kinds, 4, 8, 0, 1, 3),
            Some((ClusterKind::Performance, 1))
        );
    }

    #[test]
    fn zone_temp_and_best_cpu() {
        let mut sensors = SensorsSnapshot {
            e_c: Some(36.0),
            p_c: Some(48.0),
            ..SensorsSnapshot::default()
        };
        assert_eq!(sensors.zone_temp(ClusterKind::Efficiency), Some(36.0));
        assert_eq!(sensors.zone_temp(ClusterKind::Super), None);
        let best = sensors.best_cpu_c().unwrap_or(0.0);
        assert!((best - 42.0).abs() < f32::EPSILON);
        sensors.cpu_c = Some(40.0);
        assert_eq!(sensors.best_cpu_c(), Some(40.0));
    }
}

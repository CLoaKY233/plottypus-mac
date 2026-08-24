use plottypus_core::{
    Cluster, ClusterKind, CoreSample, CpuSnapshot, Result, SensorsSnapshot, Snapshot, SocInfo,
};

use crate::cpu::CpuCollector;
use crate::disk::DiskCollector;
use crate::fan::FanCollector;
use crate::net::NetCollector;
use crate::process::ProcessCollector;
use crate::{gpu, memory, soc, thermal};

fn fill_missing_temps(snap: &mut Snapshot) {
    if snap.cpu.temp_c.is_none() {
        snap.cpu.temp_c = snap.sensors.best_cpu_c();
    }
    if let Some(gpu) = snap.gpu.as_mut()
        && gpu.temp_c.is_none()
    {
        gpu.temp_c = snap.sensors.gpu_c.or_else(|| gpu_reading(&snap.sensors));
    }
}

fn gpu_reading(sensors: &SensorsSnapshot) -> Option<f32> {
    sensors
        .readings
        .iter()
        .find(|r| r.name.to_ascii_lowercase().contains("gpu"))
        .map(|r| r.celsius)
}

pub(crate) fn assign_core_roles(cpu: &mut CpuSnapshot, soc: &SocInfo) {
    let n = cpu.cores.len();
    for (i, core) in cpu.cores.iter_mut().enumerate() {
        if let Some((kind, index)) = soc.role_for(i, n) {
            core.kind = kind;
            core.index = index;
        }
    }
    cpu.e_cluster = summarize_cluster(&cpu.cores, ClusterKind::Efficiency);
    cpu.p_cluster = summarize_cluster(&cpu.cores, ClusterKind::Performance);
    cpu.s_cluster = summarize_cluster(&cpu.cores, ClusterKind::Super);
}

fn summarize_cluster(cores: &[CoreSample], kind: ClusterKind) -> Option<Cluster> {
    let mut n = 0_u32;
    let mut sum_scaled = 0.0_f32;
    let mut sum_active = 0.0_f32;
    for core in cores {
        if core.kind == kind {
            n += 1;
            sum_scaled += core.scaled;
            sum_active += core.active;
        }
    }
    if n == 0 {
        return None;
    }
    Some(Cluster {
        kind,
        scaled: sum_scaled / n as f32,
        active: sum_active / n as f32,
        freq_mhz: 0,
    })
}

pub struct Sampler {
    soc: SocInfo,
    cpu: CpuCollector,
    processes: ProcessCollector,
    net: NetCollector,
    disk: DiskCollector,
    fans: FanCollector,
}

impl Sampler {
    pub fn new() -> Result<Self> {
        Ok(Self {
            soc: soc::info(),
            cpu: CpuCollector::new(),
            processes: ProcessCollector::new(),
            net: NetCollector::new(),
            disk: DiskCollector::new(),
            fans: FanCollector::new(),
        })
    }

    pub fn tick(&mut self) -> Result<Snapshot> {
        let mut snap = Snapshot::empty();
        snap.soc = self.soc.clone();
        snap.cpu = self.cpu.sample()?;
        assign_core_roles(&mut snap.cpu, &snap.soc);
        snap.memory = memory::sample()?;
        snap.gpu = gpu::sample();
        snap.network = self.net.sample();
        snap.disk = self.disk.sample();
        snap.fans = self.fans.sample();
        snap.sensors = self.fans.sample_sensors();
        fill_missing_temps(&mut snap);
        snap.processes = self.processes.sample()?;
        snap.thermal = thermal::sample();
        Ok(snap)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn sampler_ticks() {
        let mut sampler = Sampler::new().expect("sampler");
        let snap = sampler.tick().expect("tick");
        assert!(!snap.soc.name.is_empty());
    }

    #[test]
    fn assign_roles_uses_registry_kinds() {
        let mut cpu = plottypus_core::CpuSnapshot {
            cores: (0..4)
                .map(|i| plottypus_core::CoreSample {
                    kind: plottypus_core::ClusterKind::Performance,
                    index: i,
                    scaled: 0.1 * (f32::from(i) + 1.0),
                    active: 0.1 * (f32::from(i) + 1.0),
                })
                .collect(),
            ..plottypus_core::CpuSnapshot::default()
        };
        let soc = plottypus_core::SocInfo {
            e_cores: 2,
            p_cores: 2,
            core_kinds: vec![
                plottypus_core::ClusterKind::Efficiency,
                plottypus_core::ClusterKind::Efficiency,
                plottypus_core::ClusterKind::Performance,
                plottypus_core::ClusterKind::Performance,
            ],
            ..plottypus_core::SocInfo::default()
        };
        assign_core_roles(&mut cpu, &soc);
        assert_eq!(cpu.cores[0].kind, plottypus_core::ClusterKind::Efficiency);
        assert_eq!(cpu.cores[0].index, 0);
        assert_eq!(cpu.cores[1].index, 1);
        assert_eq!(cpu.cores[2].kind, plottypus_core::ClusterKind::Performance);
        assert_eq!(cpu.cores[2].index, 0);
        assert!(cpu.e_cluster.is_some());
        assert!(cpu.p_cluster.is_some());
        assert!(cpu.s_cluster.is_none());
    }

    #[test]
    fn fills_gpu_temp_from_sensors() {
        let mut snap = Snapshot::empty();
        snap.gpu = Some(plottypus_core::GpuSnapshot::default());
        snap.sensors.gpu_c = Some(47.0);
        fill_missing_temps(&mut snap);
        assert_eq!(snap.gpu.and_then(|g| g.temp_c), Some(47.0));
    }

    #[test]
    fn keeps_existing_gpu_temp() {
        let mut snap = Snapshot::empty();
        snap.gpu = Some(plottypus_core::GpuSnapshot {
            temp_c: Some(51.0),
            ..plottypus_core::GpuSnapshot::default()
        });
        snap.sensors.gpu_c = Some(47.0);
        fill_missing_temps(&mut snap);
        assert_eq!(snap.gpu.and_then(|g| g.temp_c), Some(51.0));
    }

    #[test]
    fn gpu_temp_falls_back_to_a_gpu_named_reading() {
        let mut snap = Snapshot::empty();
        snap.gpu = Some(plottypus_core::GpuSnapshot::default());
        snap.sensors.readings = vec![
            plottypus_core::TempReading {
                name: String::from("nand"),
                celsius: 38.0,
            },
            plottypus_core::TempReading {
                name: String::from("GPU MTR Temp Sensor0"),
                celsius: 44.5,
            },
        ];
        fill_missing_temps(&mut snap);
        assert_eq!(snap.gpu.and_then(|g| g.temp_c), Some(44.5));
    }

    #[test]
    fn cpu_package_is_never_passed_off_as_gpu_temp() {
        let mut snap = Snapshot::empty();
        snap.gpu = Some(plottypus_core::GpuSnapshot::default());
        snap.sensors.cpu_c = Some(61.0);
        snap.sensors.hotspot_c = Some(70.0);
        fill_missing_temps(&mut snap);
        // No zone or reading says GPU: honest absence beats a fake number.
        assert_eq!(snap.gpu.and_then(|g| g.temp_c), None);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn live_tick_fills_snapshot() {
        let mut sampler = Sampler::new().expect("sampler");
        let first = sampler.tick().expect("tick1");
        assert_ne!(first.soc.name, "unknown");
        assert!(first.memory.total_bytes > 0);
        assert!(!first.processes.is_empty());
        std::thread::sleep(std::time::Duration::from_millis(40));
        let second = sampler.tick().expect("tick2");
        assert!(!second.cpu.cores.is_empty());
        assert!((0.0..=1.0).contains(&second.cpu.active));
    }
}

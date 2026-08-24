use plottypus_core::{Result, Snapshot, SocInfo};

use crate::cpu::CpuCollector;
use crate::disk::DiskCollector;
use crate::fan::FanCollector;
use crate::net::NetCollector;
use crate::process::ProcessCollector;
use crate::{gpu, memory, soc, thermal};

fn fill_missing_temps(snap: &mut Snapshot) {
    if snap.cpu.temp_c.is_none() {
        snap.cpu.temp_c = snap.sensors.cpu_c.or(snap.sensors.hotspot_c);
    }
    if let Some(gpu) = snap.gpu.as_mut()
        && gpu.temp_c.is_none()
    {
        gpu.temp_c = snap.sensors.gpu_c;
    }
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

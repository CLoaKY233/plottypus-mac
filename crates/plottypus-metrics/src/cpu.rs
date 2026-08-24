use plottypus_core::{CpuSnapshot, Result};

pub(crate) struct CpuCollector {
    prev: Option<Vec<[u32; 4]>>,
}

impl CpuCollector {
    pub(crate) fn new() -> Self {
        Self { prev: None }
    }

    pub(crate) fn sample(&mut self) -> Result<CpuSnapshot> {
        #[cfg(target_os = "macos")]
        {
            macos::sample(self)
        }
        #[cfg(not(target_os = "macos"))]
        {
            Ok(CpuSnapshot::default())
        }
    }
}

const USER: usize = 0;
const SYSTEM: usize = 1;
const IDLE: usize = 2;
const NICE: usize = 3;

fn tick_delta(prev: &[u32; 4], now: &[u32; 4]) -> [u64; 4] {
    [
        u64::from(now[USER].wrapping_sub(prev[USER])),
        u64::from(now[SYSTEM].wrapping_sub(prev[SYSTEM])),
        u64::from(now[IDLE].wrapping_sub(prev[IDLE])),
        u64::from(now[NICE].wrapping_sub(prev[NICE])),
    ]
}

fn ticks_total(delta: &[u64; 4]) -> u64 {
    delta[USER] + delta[SYSTEM] + delta[IDLE] + delta[NICE]
}

fn ticks_busy(delta: &[u64; 4]) -> u64 {
    delta[USER] + delta[SYSTEM] + delta[NICE]
}

/// Active (non-idle) ratio from two `cpu_ticks` snapshots.
pub(crate) fn core_active_ratio(prev: &[u32; 4], now: &[u32; 4]) -> f32 {
    let delta = tick_delta(prev, now);
    let total = ticks_total(&delta);
    if total == 0 {
        0.0
    } else {
        ticks_busy(&delta) as f32 / total as f32
    }
}

/// Overall active ratio across matching per-core tick arrays.
pub(crate) fn overall_active_ratio(prev: &[[u32; 4]], now: &[[u32; 4]]) -> f32 {
    if prev.len() != now.len() || prev.is_empty() {
        return 0.0;
    }
    let mut busy = 0_u64;
    let mut total = 0_u64;
    for (p, n) in prev.iter().zip(now.iter()) {
        let delta = tick_delta(p, n);
        busy += ticks_busy(&delta);
        total += ticks_total(&delta);
    }
    if total == 0 {
        0.0
    } else {
        busy as f32 / total as f32
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use super::{CpuCollector, core_active_ratio, overall_active_ratio};
    use plottypus_core::{ClusterKind, CoreSample, CpuSnapshot, Error, Result};
    use std::mem::size_of;
    use std::ptr;

    pub(super) fn sample(collector: &mut CpuCollector) -> Result<CpuSnapshot> {
        let now = load_ticks()?;
        let prev = collector.prev.replace(now.clone());
        let Some(prev) = prev.filter(|p| p.len() == now.len()) else {
            return Ok(snapshot_from_ratios(0.0, &now, &now));
        };
        let overall = overall_active_ratio(&prev, &now);
        Ok(snapshot_from_ratios(overall, &prev, &now))
    }

    fn snapshot_from_ratios(overall: f32, prev: &[[u32; 4]], now: &[[u32; 4]]) -> CpuSnapshot {
        let cores = now
            .iter()
            .enumerate()
            .map(|(i, ticks)| {
                let active = prev.get(i).map_or(0.0, |p| core_active_ratio(p, ticks));
                CoreSample {
                    kind: ClusterKind::Performance,
                    index: i as u16,
                    scaled: active,
                    active,
                }
            })
            .collect();
        CpuSnapshot {
            scaled: overall,
            active: overall,
            cores,
            ..CpuSnapshot::default()
        }
    }

    fn load_ticks() -> Result<Vec<[u32; 4]>> {
        let mut cpu_count: libc::natural_t = 0;
        let mut info: libc::processor_info_array_t = ptr::null_mut();
        let mut info_count: libc::mach_msg_type_number_t = 0;
        #[allow(deprecated)]
        let kr = unsafe {
            // SAFETY: documented Mach host query; out pointers are valid locals.
            libc::host_processor_info(
                libc::mach_host_self(),
                libc::PROCESSOR_CPU_LOAD_INFO,
                &raw mut cpu_count,
                &raw mut info,
                &raw mut info_count,
            )
        };
        if kr != libc::KERN_SUCCESS {
            return Err(Error::system(format!("host_processor_info: {kr}")));
        }
        let ticks = copy_ticks(info, cpu_count, info_count);
        deallocate_info(info, info_count);
        Ok(ticks)
    }

    fn copy_ticks(
        info: libc::processor_info_array_t,
        cpu_count: libc::natural_t,
        info_count: libc::mach_msg_type_number_t,
    ) -> Vec<[u32; 4]> {
        if info.is_null() || cpu_count == 0 {
            return Vec::new();
        }
        let states = libc::CPU_STATE_MAX as usize;
        let available = info_count as usize;
        let ncpu = (cpu_count as usize).min(available / states);
        let mut out = Vec::with_capacity(ncpu);
        for i in 0..ncpu {
            let base = i * states;
            // SAFETY: kernel filled `info_count` integers; `base + 3` is in range.
            let ticks = unsafe {
                [
                    *info.add(base) as u32,
                    *info.add(base + 1) as u32,
                    *info.add(base + 2) as u32,
                    *info.add(base + 3) as u32,
                ]
            };
            out.push(ticks);
        }
        out
    }

    fn deallocate_info(
        info: libc::processor_info_array_t,
        info_count: libc::mach_msg_type_number_t,
    ) {
        if info.is_null() {
            return;
        }
        let bytes = (info_count as usize).saturating_mul(size_of::<libc::integer_t>());
        #[allow(deprecated)]
        unsafe {
            // SAFETY: buffer came from host_processor_info; size is the returned count.
            libc::vm_deallocate(
                libc::mach_task_self(),
                info as libc::vm_address_t,
                bytes as libc::vm_size_t,
            );
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn ratio_from_two_tick_arrays() {
        let prev = vec![[10, 5, 80, 5], [0, 0, 100, 0]];
        let now = vec![[20, 15, 90, 5], [10, 0, 110, 0]];
        // core0: d=(10,10,10,0) busy=20 total=30 → 2/3
        // core1: d=(10,0,10,0) busy=10 total=20 → 0.5
        // overall busy=30 total=50 → 0.6
        assert!((core_active_ratio(&prev[0], &now[0]) - 20.0 / 30.0).abs() < f32::EPSILON);
        assert!((core_active_ratio(&prev[1], &now[1]) - 0.5).abs() < f32::EPSILON);
        assert!((overall_active_ratio(&prev, &now) - 0.6).abs() < f32::EPSILON);
    }

    #[test]
    fn zero_delta_is_idle() {
        let ticks = [4, 1, 90, 1];
        assert!((core_active_ratio(&ticks, &ticks) - 0.0).abs() < f32::EPSILON);
        assert!((overall_active_ratio(&[ticks], &[ticks]) - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn length_mismatch_is_zero() {
        assert!(
            (overall_active_ratio(&[[1, 0, 0, 0]], &[[1, 0, 0, 0], [1, 0, 0, 0]]) - 0.0).abs()
                < f32::EPSILON
        );
    }

    #[test]
    fn wrapping_ticks() {
        let prev = [u32::MAX - 4, 0, 10, 0];
        let now = [5, 0, 20, 0];
        let ratio = core_active_ratio(&prev, &now);
        assert!(ratio > 0.4 && ratio < 0.6, "{ratio}");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn live_second_sample_has_cores() {
        let mut cpu = CpuCollector::new();
        let first = cpu.sample().expect("cpu1");
        assert!(first.active >= 0.0);
        std::thread::sleep(std::time::Duration::from_millis(40));
        let second = cpu.sample().expect("cpu2");
        assert!(!second.cores.is_empty());
        assert!((0.0..=1.0).contains(&second.active));
        assert!((0.0..=1.0).contains(&second.scaled));
    }
}

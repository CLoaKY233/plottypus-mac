use plottypus_core::SocInfo;

pub(crate) fn info() -> SocInfo {
    #[cfg(target_os = "macos")]
    {
        macos::info()
    }
    #[cfg(not(target_os = "macos"))]
    {
        SocInfo::default()
    }
}

/// Map `hw.nperflevels` + per-level logical CPU counts onto E/P/S.
pub(crate) fn cluster_counts(nlevels: u32, level_cpus: [u8; 3]) -> (u8, u8, u8) {
    match nlevels {
        3 => (level_cpus[2], level_cpus[1], level_cpus[0]),
        2 => (level_cpus[1], level_cpus[0], 0),
        1 => (0, level_cpus[0], 0),
        _ => (0, 0, 0),
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use super::cluster_counts;
    use crate::sys;
    use plottypus_core::SocInfo;

    pub(super) fn info() -> SocInfo {
        let name = sys::sysctl_string(c"machdep.cpu.brand_string")
            .or_else(|| sys::sysctl_string(c"hw.model"))
            .unwrap_or_else(fallback_name);
        let memory_bytes = sys::sysctl_u64(c"hw.memsize").unwrap_or(0);
        let nlevels = sys::sysctl_u64(c"hw.nperflevels")
            .and_then(|n| u32::try_from(n).ok())
            .unwrap_or(0);
        let level_cpus = [
            logical_level(c"hw.perflevel0.logicalcpu"),
            logical_level(c"hw.perflevel1.logicalcpu"),
            logical_level(c"hw.perflevel2.logicalcpu"),
        ];
        let (e_cores, mut p_cores, s_cores) = cluster_counts(nlevels, level_cpus);
        if e_cores == 0 && p_cores == 0 && s_cores == 0 {
            let phys = sys::sysctl_u64(c"hw.physicalcpu")
                .or_else(|| sys::sysctl_u64(c"hw.logicalcpu"))
                .unwrap_or(0);
            p_cores = u8::try_from(phys.min(u64::from(u8::MAX))).unwrap_or(u8::MAX);
        }
        if e_cores == 0 && p_cores == 0 && s_cores == 0 {
            p_cores = u8::try_from(sys::logical_cpus().min(u32::from(u8::MAX))).unwrap_or(1);
        }
        SocInfo {
            name,
            e_cores,
            p_cores,
            s_cores,
            gpu_cores: 0,
            memory_bytes,
        }
    }

    fn logical_level(name: &std::ffi::CStr) -> u8 {
        sys::sysctl_u64(name)
            .and_then(|n| u8::try_from(n).ok())
            .unwrap_or(0)
    }

    fn fallback_name() -> String {
        if sys::sysctl_u64(c"hw.optional.arm64") == Some(1) {
            String::from("Apple Silicon")
        } else {
            String::from("Mac")
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn cluster_counts_two_and_three_level() {
        assert_eq!(cluster_counts(2, [6, 4, 0]), (4, 6, 0));
        assert_eq!(cluster_counts(3, [2, 6, 4]), (4, 6, 2));
        assert_eq!(cluster_counts(1, [8, 0, 0]), (0, 8, 0));
        assert_eq!(cluster_counts(0, [0, 0, 0]), (0, 0, 0));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn live_soc_is_named() {
        let soc = info();
        assert!(!soc.name.is_empty());
        assert_ne!(soc.name, "unknown");
        assert!(soc.memory_bytes > 0);
        assert!(
            u16::from(soc.e_cores) + u16::from(soc.p_cores) + u16::from(soc.s_cores) > 0,
            "no cores"
        );
    }
}

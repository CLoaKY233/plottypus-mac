use plottypus_core::{MemorySnapshot, Pressure, Result};

pub(crate) fn sample() -> Result<MemorySnapshot> {
    #[cfg(target_os = "macos")]
    {
        macos::sample()
    }
    #[cfg(not(target_os = "macos"))]
    {
        Ok(MemorySnapshot::default())
    }
}

#[derive(Clone, Copy)]
pub(crate) struct VmPages {
    pub active: u64,
    pub inactive: u64,
    pub wire: u64,
    pub speculative: u64,
    pub compressor: u64,
    pub purgeable: u64,
    pub external: u64,
}

/// Activity Monitor–style used pages × page size.
pub(crate) fn used_bytes(pages: VmPages, page_size: u64) -> u64 {
    let count = pages
        .active
        .saturating_add(pages.inactive)
        .saturating_add(pages.wire)
        .saturating_add(pages.speculative)
        .saturating_add(pages.compressor)
        .saturating_sub(pages.purgeable)
        .saturating_sub(pages.external);
    count.saturating_mul(page_size)
}

/// `kern.memorystatus_vm_pressure_level`: 1=Nominal, 2=Warn, 4=Critical.
pub(crate) fn pressure_from_level(level: Option<i32>) -> Pressure {
    match level {
        Some(2) => Pressure::Warn,
        Some(4) => Pressure::Critical,
        _ => Pressure::Nominal,
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use super::{pressure_from_level, used_bytes};
    use crate::sys;
    use plottypus_core::{Error, MemorySnapshot, Result};
    use std::mem::{MaybeUninit, size_of};

    pub(super) fn sample() -> Result<MemorySnapshot> {
        let page = sys::page_size();
        let total_bytes = sys::sysctl_u64(c"hw.memsize").unwrap_or(0);
        let vm = vm_stats()?;
        let pages = |n: libc::natural_t| u64::from(n);
        let used_bytes = used_bytes(
            super::VmPages {
                active: pages(vm.active_count),
                inactive: pages(vm.inactive_count),
                wire: pages(vm.wire_count),
                speculative: pages(vm.speculative_count),
                compressor: pages(vm.compressor_page_count),
                purgeable: pages(vm.purgeable_count),
                external: pages(vm.external_page_count),
            },
            page,
        );
        let (swap_used_bytes, swap_total_bytes) = swap_usage();
        Ok(MemorySnapshot {
            used_bytes,
            total_bytes,
            wired_bytes: pages(vm.wire_count).saturating_mul(page),
            compressed_bytes: pages(vm.compressor_page_count).saturating_mul(page),
            cache_bytes: pages(vm.external_page_count).saturating_mul(page),
            swap_used_bytes,
            swap_total_bytes,
            pressure: pressure_from_level(sys::sysctl_i32(c"kern.memorystatus_vm_pressure_level")),
        })
    }

    fn vm_stats() -> Result<libc::vm_statistics64> {
        let mut vm = MaybeUninit::<libc::vm_statistics64>::zeroed();
        let mut count = libc::HOST_VM_INFO64_COUNT;
        #[allow(deprecated)]
        let kr = unsafe {
            // SAFETY: `vm` is a valid HOST_VM_INFO64 buffer; count is the libc slot count.
            libc::host_statistics64(
                libc::mach_host_self(),
                libc::HOST_VM_INFO64,
                vm.as_mut_ptr().cast(),
                &raw mut count,
            )
        };
        if kr != libc::KERN_SUCCESS {
            return Err(Error::system(format!("host_statistics64: {kr}")));
        }
        Ok(unsafe {
            // SAFETY: kernel wrote a vm_statistics64 on KERN_SUCCESS.
            vm.assume_init()
        })
    }

    fn swap_usage() -> (u64, u64) {
        let mut mib = [libc::CTL_VM, libc::VM_SWAPUSAGE];
        let mut xsw = MaybeUninit::<libc::xsw_usage>::zeroed();
        let mut len = size_of::<libc::xsw_usage>();
        let rc = unsafe {
            // SAFETY: MIB is VM_SWAPUSAGE; `xsw` is the documented xsw_usage out buffer.
            libc::sysctl(
                mib.as_mut_ptr(),
                mib.len() as libc::c_uint,
                xsw.as_mut_ptr().cast(),
                &raw mut len,
                std::ptr::null_mut(),
                0,
            )
        };
        if rc != 0 {
            return (0, 0);
        }
        let xsw = unsafe {
            // SAFETY: sysctl succeeded and wrote xsw_usage.
            xsw.assume_init()
        };
        (xsw.xsu_used, xsw.xsu_total)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn used_bytes_matches_research_formula() {
        let pages = VmPages {
            active: 10,
            inactive: 20,
            wire: 5,
            speculative: 2,
            compressor: 8,
            purgeable: 3,
            external: 4,
        };
        assert_eq!(used_bytes(pages, 4096), 38 * 4096);
    }

    #[test]
    fn used_bytes_saturates_when_subtracting() {
        let pages = VmPages {
            active: 1,
            inactive: 1,
            wire: 1,
            speculative: 0,
            compressor: 0,
            purgeable: 10,
            external: 10,
        };
        assert_eq!(used_bytes(pages, 4096), 0);
    }

    #[test]
    fn pressure_mapping() {
        assert_eq!(pressure_from_level(None), Pressure::Nominal);
        assert_eq!(pressure_from_level(Some(1)), Pressure::Nominal);
        assert_eq!(pressure_from_level(Some(2)), Pressure::Warn);
        assert_eq!(pressure_from_level(Some(4)), Pressure::Critical);
        assert_eq!(pressure_from_level(Some(99)), Pressure::Nominal);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn live_memory_totals() {
        let mem = sample().expect("memory");
        assert!(mem.total_bytes > 0, "hw.memsize");
        assert!(mem.used_bytes > 0, "used pages");
        assert!(mem.used_bytes <= mem.total_bytes.saturating_mul(2));
    }
}

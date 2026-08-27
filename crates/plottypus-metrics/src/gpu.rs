use plottypus_core::GpuSnapshot;

pub(crate) struct GpuCollector {
    #[cfg(target_os = "macos")]
    ports: Vec<u32>,
}

impl GpuCollector {
    pub(crate) fn new() -> Self {
        Self {
            #[cfg(target_os = "macos")]
            ports: Vec::new(),
        }
    }

    pub(crate) fn sample(&mut self) -> Option<GpuSnapshot> {
        #[cfg(target_os = "macos")]
        {
            macos::sample(&mut self.ports)
        }
        #[cfg(not(target_os = "macos"))]
        {
            None
        }
    }
}

#[cfg(target_os = "macos")]
impl Drop for GpuCollector {
    fn drop(&mut self) {
        macos::release_ports(&mut self.ports);
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use plottypus_core::GpuSnapshot;
    use std::ptr;

    use crate::iokit::{self, CfDictRef};

    #[link(name = "IOKit", kind = "framework")]
    unsafe extern "C" {
        fn IORegistryEntryCreateCFProperties(
            entry: u32,
            properties: *mut CfDictRef,
            allocator: *const std::ffi::c_void,
            options: u32,
        ) -> libc::kern_return_t;
    }

    pub(super) fn release_ports(ports: &mut Vec<u32>) {
        iokit::release_ports(ports);
    }

    fn rematch(ports: &mut Vec<u32>) {
        iokit::release_ports(ports);
        *ports = iokit::matching_services(c"IOAccelerator");
    }

    pub(super) fn sample(ports: &mut Vec<u32>) -> Option<GpuSnapshot> {
        if ports.is_empty() {
            rematch(ports);
        }
        let mut best = max_util(ports);
        if best.is_none() {
            rematch(ports);
            best = max_util(ports);
        }
        best.filter(|v| *v >= 0.0).map(|v| GpuSnapshot {
            scaled: (v / 100.0).clamp(0.0, 1.0),
            active: (v / 100.0).clamp(0.0, 1.0),
            ..GpuSnapshot::default()
        })
    }

    fn max_util(ports: &[u32]) -> Option<f32> {
        let mut best: Option<f32> = None;
        for &service in ports {
            if let Some(util) = util_for_service(service) {
                best = Some(best.map_or(util, |b| b.max(util)));
            }
        }
        best
    }

    fn util_for_service(service: u32) -> Option<f32> {
        let mut props: CfDictRef = ptr::null();
        let kr = unsafe {
            // SAFETY: `props` is an out-pointer; IOKit writes a +1 CF dict on success.
            IORegistryEntryCreateCFProperties(service, &raw mut props, ptr::null(), 0)
        };
        if kr != 0 || props.is_null() {
            return None;
        }
        let stats = iokit::dict_get(props, c"PerformanceStatistics");
        let util = stats.and_then(|s| {
            iokit::dict_f64(s, c"Device Utilization %")
                .or_else(|| iokit::dict_f64(s, c"GPU Activity(%)"))
                .or_else(|| iokit::dict_f64(s, c"Renderer Utilization %"))
        });
        iokit::cf_release(props);
        util
    }
}

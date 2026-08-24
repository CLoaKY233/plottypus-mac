use plottypus_core::GpuSnapshot;

pub(crate) fn sample() -> Option<GpuSnapshot> {
    #[cfg(target_os = "macos")]
    {
        macos::sample()
    }
    #[cfg(not(target_os = "macos"))]
    {
        None
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use plottypus_core::GpuSnapshot;
    use std::ffi::CStr;
    use std::ptr;

    type CfTypeRef = *const std::ffi::c_void;
    type CfDictRef = *const std::ffi::c_void;
    type CfStringRef = *const std::ffi::c_void;
    type CfAllocatorRef = *const std::ffi::c_void;

    #[link(name = "IOKit", kind = "framework")]
    unsafe extern "C" {
        fn IOServiceMatching(name: *const libc::c_char) -> CfDictRef;
        fn IOServiceGetMatchingServices(
            port: u32,
            matching: CfDictRef,
            existing: *mut u32,
        ) -> libc::kern_return_t;
        fn IOIteratorNext(iterator: u32) -> u32;
        fn IOObjectRelease(obj: u32) -> libc::kern_return_t;
        fn IORegistryEntryCreateCFProperties(
            entry: u32,
            properties: *mut CfDictRef,
            allocator: CfAllocatorRef,
            options: u32,
        ) -> libc::kern_return_t;
        fn CFDictionaryGetValue(dict: CfDictRef, key: CfTypeRef) -> CfTypeRef;
        fn CFStringCreateWithCString(
            alloc: CfAllocatorRef,
            c_str: *const libc::c_char,
            encoding: u32,
        ) -> CfStringRef;
        fn CFGetTypeID(cf: CfTypeRef) -> usize;
        fn CFNumberGetTypeID() -> usize;
        fn CFNumberGetValue(
            number: CfTypeRef,
            the_type: i32,
            value_ptr: *mut std::ffi::c_void,
        ) -> u8;
        fn CFRelease(cf: CfTypeRef);
    }

    const K_CF_STRING_ENCODING_UTF8: u32 = 0x0800_0100;
    const K_CF_NUMBER_FLOAT64: i32 = 6;
    const K_CF_NUMBER_SINT32: i32 = 3;

    pub(super) fn sample() -> Option<GpuSnapshot> {
        let matching = unsafe { IOServiceMatching(c"IOAccelerator".as_ptr()) };
        if matching.is_null() {
            return None;
        }
        let mut iter: u32 = 0;
        let kr = unsafe { IOServiceGetMatchingServices(0, matching, &raw mut iter) };
        if kr != 0 {
            return None;
        }
        let mut best: Option<f32> = None;
        loop {
            let service = unsafe { IOIteratorNext(iter) };
            if service == 0 {
                break;
            }
            if let Some(util) = util_for_service(service) {
                best = Some(best.map_or(util, |b| b.max(util)));
            }
            unsafe { IOObjectRelease(service) };
        }
        unsafe { IOObjectRelease(iter) };
        best.filter(|v| *v >= 0.0).map(|v| GpuSnapshot {
            scaled: (v / 100.0).clamp(0.0, 1.0),
            active: (v / 100.0).clamp(0.0, 1.0),
            ..GpuSnapshot::default()
        })
    }

    fn util_for_service(service: u32) -> Option<f32> {
        let mut props: CfDictRef = ptr::null();
        let kr =
            unsafe { IORegistryEntryCreateCFProperties(service, &raw mut props, ptr::null(), 0) };
        if kr != 0 || props.is_null() {
            return None;
        }
        let stats = cf_dict_get(props, c"PerformanceStatistics");
        let util = stats.and_then(|s| {
            cf_dict_f64(s, c"Device Utilization %")
                .or_else(|| cf_dict_f64(s, c"GPU Activity(%)"))
                .or_else(|| cf_dict_f64(s, c"Renderer Utilization %"))
        });
        unsafe { CFRelease(props) };
        util
    }

    fn cf_dict_get(dict: CfDictRef, key: &CStr) -> Option<CfDictRef> {
        let cfkey = unsafe {
            CFStringCreateWithCString(ptr::null(), key.as_ptr(), K_CF_STRING_ENCODING_UTF8)
        };
        if cfkey.is_null() {
            return None;
        }
        let val = unsafe { CFDictionaryGetValue(dict, cfkey.cast()) };
        unsafe { CFRelease(cfkey.cast()) };
        if val.is_null() { None } else { Some(val) }
    }

    fn cf_dict_f64(dict: CfTypeRef, key: &CStr) -> Option<f32> {
        let val = cf_dict_get(dict, key)?;
        let tid = unsafe { CFGetTypeID(val) };
        if tid != unsafe { CFNumberGetTypeID() } {
            return None;
        }
        let mut f = 0.0_f64;
        let ok = unsafe { CFNumberGetValue(val, K_CF_NUMBER_FLOAT64, (&raw mut f).cast()) };
        if ok != 0 {
            return Some(f as f32);
        }
        let mut i = 0_i32;
        let ok = unsafe { CFNumberGetValue(val, K_CF_NUMBER_SINT32, (&raw mut i).cast()) };
        if ok != 0 { Some(i as f32) } else { None }
    }
}

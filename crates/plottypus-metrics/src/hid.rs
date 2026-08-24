use plottypus_core::SensorsSnapshot;

pub(crate) fn sample_temps() -> SensorsSnapshot {
    #[cfg(target_os = "macos")]
    {
        macos::sample()
    }
    #[cfg(not(target_os = "macos"))]
    {
        SensorsSnapshot::default()
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use plottypus_core::SensorsSnapshot;
    use std::ffi::c_void;
    use std::ptr;

    type CfTypeRef = *const c_void;
    type CfDictRef = *const c_void;
    type CfArrayRef = *const c_void;
    type CfStringRef = *const c_void;
    type CfAllocatorRef = *const c_void;

    #[repr(C)]
    struct CfDictKeyCallBacks {
        _priv: [usize; 8],
    }

    #[repr(C)]
    struct CfDictValueCallBacks {
        _priv: [usize; 8],
    }

    #[allow(clippy::duplicated_attributes)]
    #[link(name = "IOKit", kind = "framework")]
    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        fn IOHIDEventSystemClientCreate(allocator: CfAllocatorRef) -> *mut c_void;
        fn IOHIDEventSystemClientSetMatching(client: *mut c_void, matching: CfDictRef);
        fn IOHIDEventSystemClientCopyServices(client: *mut c_void) -> CfArrayRef;
        fn IOHIDServiceClientCopyProperty(service: CfTypeRef, key: CfStringRef) -> CfTypeRef;
        fn IOHIDServiceClientCopyEvent(
            service: CfTypeRef,
            event_type: i64,
            matching: CfTypeRef,
            timeout: i64,
        ) -> *mut c_void;
        fn IOHIDEventGetFloatValue(event: *mut c_void, field: i32) -> f64;
        fn CFArrayGetCount(array: CfArrayRef) -> isize;
        fn CFArrayGetValueAtIndex(array: CfArrayRef, idx: isize) -> CfTypeRef;
        fn CFDictionaryCreate(
            allocator: CfAllocatorRef,
            keys: *const CfTypeRef,
            values: *const CfTypeRef,
            num: isize,
            key_callbacks: *const CfDictKeyCallBacks,
            value_callbacks: *const CfDictValueCallBacks,
        ) -> CfDictRef;
        static kCFTypeDictionaryKeyCallBacks: CfDictKeyCallBacks;
        static kCFTypeDictionaryValueCallBacks: CfDictValueCallBacks;
        fn CFNumberCreate(
            allocator: CfAllocatorRef,
            the_type: i32,
            value_ptr: *const c_void,
        ) -> CfTypeRef;
        fn CFStringCreateWithCString(
            alloc: CfAllocatorRef,
            c_str: *const libc::c_char,
            encoding: u32,
        ) -> CfStringRef;
        fn CFStringGetCString(
            the_string: CfStringRef,
            buffer: *mut libc::c_char,
            size: isize,
            encoding: u32,
        ) -> u8;
        fn CFGetTypeID(cf: CfTypeRef) -> usize;
        fn CFStringGetTypeID() -> usize;
        fn CFRelease(cf: CfTypeRef);
    }

    const K_CF_STRING_ENCODING_UTF8: u32 = 0x0800_0100;
    const K_CF_NUMBER_SINT32: i32 = 3;
    const HID_TEMP_EVENT: i64 = 15;
    const HID_TEMP_FIELD: i32 = 15 << 16;

    pub(super) fn sample() -> SensorsSnapshot {
        let client = unsafe { IOHIDEventSystemClientCreate(ptr::null()) };
        if client.is_null() {
            return SensorsSnapshot::default();
        }
        let Some(matching) = matching_dict() else {
            unsafe { CFRelease(client.cast()) };
            return SensorsSnapshot::default();
        };
        unsafe { IOHIDEventSystemClientSetMatching(client, matching) };
        unsafe { CFRelease(matching.cast()) };
        let services = unsafe { IOHIDEventSystemClientCopyServices(client) };
        if services.is_null() {
            unsafe { CFRelease(client.cast()) };
            return SensorsSnapshot::default();
        }
        let count = unsafe { CFArrayGetCount(services) };
        let mut named = Vec::new();
        for i in 0..count {
            let service = unsafe { CFArrayGetValueAtIndex(services, i) };
            if service.is_null() {
                continue;
            }
            let name = product_name(service);
            let Some(temp) = read_temp(service) else {
                continue;
            };
            named.push((name, temp));
        }
        unsafe {
            CFRelease(services);
            CFRelease(client.cast());
        }
        crate::zones::snapshot_from_named(&named, crate::zones::Source::Hid)
    }

    fn matching_dict() -> Option<CfDictRef> {
        let key_page = cf_str(c"PrimaryUsagePage")?;
        let key_usage = cf_str(c"PrimaryUsage")?;
        let mut page: i32 = 0xff00;
        let mut usage: i32 = 0x0005;
        let num_page =
            unsafe { CFNumberCreate(ptr::null(), K_CF_NUMBER_SINT32, (&raw mut page).cast()) };
        let num_usage =
            unsafe { CFNumberCreate(ptr::null(), K_CF_NUMBER_SINT32, (&raw mut usage).cast()) };
        if num_page.is_null() || num_usage.is_null() {
            return None;
        }
        let keys = [key_page.cast(), key_usage.cast()];
        let vals = [num_page, num_usage];
        let dict = unsafe {
            CFDictionaryCreate(
                ptr::null(),
                keys.as_ptr(),
                vals.as_ptr(),
                2,
                &raw const kCFTypeDictionaryKeyCallBacks,
                &raw const kCFTypeDictionaryValueCallBacks,
            )
        };
        unsafe {
            CFRelease(key_page.cast());
            CFRelease(key_usage.cast());
            CFRelease(num_page);
            CFRelease(num_usage);
        }
        if dict.is_null() { None } else { Some(dict) }
    }

    fn cf_str(text: &std::ffi::CStr) -> Option<CfStringRef> {
        let s = unsafe {
            CFStringCreateWithCString(ptr::null(), text.as_ptr(), K_CF_STRING_ENCODING_UTF8)
        };
        if s.is_null() { None } else { Some(s) }
    }

    fn product_name(service: CfTypeRef) -> String {
        let Some(key) = cf_str(c"Product") else {
            return String::new();
        };
        let val = unsafe { IOHIDServiceClientCopyProperty(service, key) };
        unsafe { CFRelease(key.cast()) };
        if val.is_null() {
            return String::new();
        }
        let name = if unsafe { CFGetTypeID(val) } == unsafe { CFStringGetTypeID() } {
            let mut buf = [0_i8; 128];
            let ok = unsafe {
                CFStringGetCString(
                    val,
                    buf.as_mut_ptr(),
                    buf.len() as isize,
                    K_CF_STRING_ENCODING_UTF8,
                )
            };
            if ok != 0 {
                let bytes: Vec<u8> = buf
                    .iter()
                    .take_while(|&&c| c != 0)
                    .map(|&c| c as u8)
                    .collect();
                String::from_utf8_lossy(&bytes).into_owned()
            } else {
                String::new()
            }
        } else {
            String::new()
        };
        unsafe { CFRelease(val) };
        name
    }

    fn read_temp(service: CfTypeRef) -> Option<f32> {
        let event = unsafe { IOHIDServiceClientCopyEvent(service, HID_TEMP_EVENT, ptr::null(), 0) };
        if event.is_null() {
            return None;
        }
        let t = unsafe { IOHIDEventGetFloatValue(event, HID_TEMP_FIELD) } as f32;
        unsafe { CFRelease(event.cast()) };
        if t.is_finite() && t > 0.0 && t <= 150.0 {
            Some(t)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hid_sample_does_not_panic() {
        let snap = sample_temps();
        if let Some(c) = snap.cpu_c.or(snap.hotspot_c) {
            assert!((0.0..=150.0).contains(&c), "{c}");
        }
    }
}

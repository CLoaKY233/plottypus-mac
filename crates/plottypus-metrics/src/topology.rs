use plottypus_core::ClusterKind;

pub(crate) fn core_kinds(has_super_level: bool) -> Vec<ClusterKind> {
    #[cfg(target_os = "macos")]
    {
        macos::core_kinds(has_super_level)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = has_super_level;
        Vec::new()
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use super::super::zones::cluster_letter_kind;
    use plottypus_core::ClusterKind;
    use std::ffi::c_void;
    use std::ptr;

    type CfTypeRef = *const c_void;
    type CfStringRef = *const c_void;
    type CfAllocatorRef = *const c_void;

    #[allow(clippy::duplicated_attributes)]
    #[link(name = "IOKit", kind = "framework")]
    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        fn IOServiceMatching(name: *const libc::c_char) -> *const c_void;
        fn IOServiceGetMatchingServices(
            port: u32,
            matching: *const c_void,
            existing: *mut u32,
        ) -> libc::kern_return_t;
        fn IOIteratorNext(iterator: u32) -> u32;
        fn IOObjectRelease(obj: u32) -> libc::kern_return_t;
        fn IORegistryEntryCreateCFProperty(
            entry: u32,
            key: CfStringRef,
            allocator: CfAllocatorRef,
            options: u32,
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
        fn CFDataGetTypeID() -> usize;
        fn CFNumberGetTypeID() -> usize;
        fn CFDataGetBytePtr(data: CfTypeRef) -> *const u8;
        fn CFDataGetLength(data: CfTypeRef) -> isize;
        fn CFNumberGetValue(number: CfTypeRef, the_type: i32, value_ptr: *mut c_void) -> u8;
        fn CFRelease(cf: CfTypeRef);
    }

    const K_CF_STRING_ENCODING_UTF8: u32 = 0x0800_0100;
    const K_CF_NUMBER_SINT32: i32 = 3;

    pub(super) fn core_kinds(has_super_level: bool) -> Vec<ClusterKind> {
        let matching = unsafe { IOServiceMatching(c"IOPlatformDevice".as_ptr()) };
        if matching.is_null() {
            return Vec::new();
        }
        let mut iter: u32 = 0;
        let kr = unsafe { IOServiceGetMatchingServices(0, matching, &raw mut iter) };
        if kr != 0 || iter == 0 {
            return Vec::new();
        }
        let mut found: Vec<(usize, char)> = Vec::new();
        loop {
            let service = unsafe { IOIteratorNext(iter) };
            if service == 0 {
                break;
            }
            if let Some(entry) = read_cpu(service) {
                found.push(entry);
            }
            unsafe { IOObjectRelease(service) };
        }
        unsafe { IOObjectRelease(iter) };
        assemble(found, has_super_level)
    }

    fn read_cpu(service: u32) -> Option<(usize, char)> {
        let name = cf_stringish(prop(service, c"name")?)?;
        if !name.starts_with("cpu") {
            return None;
        }
        let letter = cf_stringish(prop(service, c"cluster-type")?)?
            .chars()
            .next()?;
        let logical = prop(service, c"logical-cpu-id")
            .and_then(cf_u32)
            .map(|n| n as usize)
            .or_else(|| parse_cpu_index(&name))?;
        Some((logical, letter))
    }

    fn assemble(found: Vec<(usize, char)>, has_super_level: bool) -> Vec<ClusterKind> {
        if found.is_empty() {
            return Vec::new();
        }
        let letters: String = found.iter().map(|(_, c)| c.to_ascii_uppercase()).collect();
        let mut slots: Vec<Option<ClusterKind>> = Vec::new();
        for (index, letter) in found {
            let Some(kind) = cluster_letter_kind(letter, &letters, has_super_level) else {
                return Vec::new();
            };
            if index >= slots.len() {
                slots.resize(index + 1, None);
            }
            if let Some(slot) = slots.get_mut(index) {
                *slot = Some(kind);
            }
        }
        if slots.is_empty() || slots.iter().any(Option::is_none) {
            return Vec::new();
        }
        slots.into_iter().flatten().collect()
    }

    fn parse_cpu_index(name: &str) -> Option<usize> {
        name.strip_prefix("cpu")?.parse().ok()
    }

    fn prop(service: u32, key: &std::ffi::CStr) -> Option<CfTypeRef> {
        let cf_key = unsafe {
            CFStringCreateWithCString(ptr::null(), key.as_ptr(), K_CF_STRING_ENCODING_UTF8)
        };
        if cf_key.is_null() {
            return None;
        }
        let val = unsafe { IORegistryEntryCreateCFProperty(service, cf_key, ptr::null(), 0) };
        unsafe { CFRelease(cf_key.cast()) };
        if val.is_null() { None } else { Some(val) }
    }

    fn cf_stringish(val: CfTypeRef) -> Option<String> {
        let text = if unsafe { CFGetTypeID(val) } == unsafe { CFStringGetTypeID() } {
            let mut buf = [0_i8; 64];
            let ok = unsafe {
                CFStringGetCString(
                    val,
                    buf.as_mut_ptr(),
                    buf.len() as isize,
                    K_CF_STRING_ENCODING_UTF8,
                )
            };
            if ok == 0 {
                None
            } else {
                let bytes: Vec<u8> = buf
                    .iter()
                    .take_while(|&&c| c != 0)
                    .map(|&c| c as u8)
                    .collect();
                Some(String::from_utf8_lossy(&bytes).into_owned())
            }
        } else if unsafe { CFGetTypeID(val) } == unsafe { CFDataGetTypeID() } {
            let len = unsafe { CFDataGetLength(val) };
            let ptr = unsafe { CFDataGetBytePtr(val) };
            if ptr.is_null() || len <= 0 {
                None
            } else {
                let bytes = unsafe { std::slice::from_raw_parts(ptr, len as usize) };
                let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
                Some(String::from_utf8_lossy(&bytes[..end]).into_owned())
            }
        } else {
            None
        };
        unsafe { CFRelease(val) };
        text.filter(|s| !s.is_empty())
    }

    fn cf_u32(val: CfTypeRef) -> Option<u32> {
        let n = if unsafe { CFGetTypeID(val) } == unsafe { CFNumberGetTypeID() } {
            let mut out: i32 = 0;
            let ok = unsafe { CFNumberGetValue(val, K_CF_NUMBER_SINT32, (&raw mut out).cast()) };
            if ok == 0 || out < 0 {
                None
            } else {
                Some(out as u32)
            }
        } else {
            None
        };
        unsafe { CFRelease(val) };
        n
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_os = "macos")]
    #[test]
    fn live_kinds_cover_every_core_or_none() {
        let kinds = core_kinds(true);
        if kinds.is_empty() {
            return;
        }
        assert!(!kinds.is_empty());
        assert!(kinds.iter().all(|k| ClusterKind::ALL.contains(k)));
    }
}

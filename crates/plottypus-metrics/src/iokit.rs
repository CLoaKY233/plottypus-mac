//! Shared `IOKit` / CoreFoundation lookups. macOS only.

use std::ffi::{CStr, c_void};

pub(crate) type CfTypeRef = *const c_void;
pub(crate) type CfDictRef = *const c_void;

type CfStringRef = *const c_void;
type CfAllocatorRef = *const c_void;

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
    fn CFDictionaryGetValue(dict: CfDictRef, key: CfTypeRef) -> CfTypeRef;
    fn CFStringCreateWithCString(
        alloc: CfAllocatorRef,
        c_str: *const libc::c_char,
        encoding: u32,
    ) -> CfStringRef;
    fn CFGetTypeID(cf: CfTypeRef) -> usize;
    fn CFNumberGetTypeID() -> usize;
    fn CFNumberGetValue(number: CfTypeRef, the_type: i32, value_ptr: *mut c_void) -> u8;
    fn CFRelease(cf: CfTypeRef);
}

const K_CF_STRING_ENCODING_UTF8: u32 = 0x0800_0100;
const K_CF_NUMBER_FLOAT64: i32 = 6;
const K_CF_NUMBER_SINT32: i32 = 3;
const K_CF_NUMBER_SINT64: i32 = 4;

pub(crate) fn release_ports(ports: &mut Vec<u32>) {
    for port in ports.drain(..) {
        unsafe {
            // SAFETY: each port was retained by IOIteratorNext and is still owned here.
            IOObjectRelease(port);
        }
    }
}

pub(crate) fn matching_services(class: &CStr) -> Vec<u32> {
    let matching = unsafe {
        // SAFETY: `class` is a valid C string; the matching dict is consumed by
        // IOServiceGetMatchingServices.
        IOServiceMatching(class.as_ptr())
    };
    if matching.is_null() {
        return Vec::new();
    }
    let mut iter: u32 = 0;
    let kr = unsafe {
        // SAFETY: `matching` is a live CF dict; `iter` is an out-iterator.
        IOServiceGetMatchingServices(0, matching, &raw mut iter)
    };
    if kr != 0 {
        return Vec::new();
    }
    let mut ports = Vec::new();
    loop {
        let service = unsafe {
            // SAFETY: `iter` is a live IOKit iterator from GetMatchingServices.
            IOIteratorNext(iter)
        };
        if service == 0 {
            break;
        }
        ports.push(service);
    }
    unsafe {
        // SAFETY: we own `iter` for the rest of this function.
        IOObjectRelease(iter);
    }
    ports
}

pub(crate) fn dict_get(dict: CfDictRef, key: &CStr) -> Option<CfTypeRef> {
    let cfkey = unsafe {
        // SAFETY: `key` is a valid C string; allocator is default.
        CFStringCreateWithCString(std::ptr::null(), key.as_ptr(), K_CF_STRING_ENCODING_UTF8)
    };
    if cfkey.is_null() {
        return None;
    }
    let val = unsafe {
        // SAFETY: `dict` and `cfkey` are live CF objects.
        CFDictionaryGetValue(dict, cfkey.cast())
    };
    unsafe {
        // SAFETY: we created `cfkey` above.
        CFRelease(cfkey.cast());
    }
    if val.is_null() { None } else { Some(val) }
}

pub(crate) fn dict_f64(dict: CfTypeRef, key: &CStr) -> Option<f32> {
    let val = dict_get(dict, key)?;
    let tid = unsafe {
        // SAFETY: `val` came from a live CF dictionary.
        CFGetTypeID(val)
    };
    let number_tid = unsafe {
        // SAFETY: CFNumberGetTypeID is a constant query.
        CFNumberGetTypeID()
    };
    if tid != number_tid {
        return None;
    }
    let mut f = 0.0_f64;
    let ok = unsafe {
        // SAFETY: `value_ptr` is a local f64.
        CFNumberGetValue(val, K_CF_NUMBER_FLOAT64, (&raw mut f).cast())
    };
    if ok != 0 {
        return Some(f as f32);
    }
    let mut i = 0_i32;
    let ok = unsafe {
        // SAFETY: `value_ptr` is a local i32.
        CFNumberGetValue(val, K_CF_NUMBER_SINT32, (&raw mut i).cast())
    };
    if ok != 0 { Some(i as f32) } else { None }
}

pub(crate) fn dict_u64(dict: CfTypeRef, key: &CStr) -> Option<u64> {
    let val = dict_get(dict, key)?;
    let tid = unsafe {
        // SAFETY: `val` came from a live CF dictionary.
        CFGetTypeID(val)
    };
    let number_tid = unsafe {
        // SAFETY: CFNumberGetTypeID is a constant query.
        CFNumberGetTypeID()
    };
    if tid != number_tid {
        return None;
    }
    let mut n = 0_i64;
    let ok = unsafe {
        // SAFETY: `value_ptr` is a local i64.
        CFNumberGetValue(val, K_CF_NUMBER_SINT64, (&raw mut n).cast())
    };
    if ok != 0 {
        return u64::try_from(n).ok();
    }
    let mut f = 0.0_f64;
    let ok = unsafe {
        // SAFETY: `value_ptr` is a local f64.
        CFNumberGetValue(val, K_CF_NUMBER_FLOAT64, (&raw mut f).cast())
    };
    if ok != 0 && f.is_finite() && f >= 0.0 {
        Some(f as u64)
    } else {
        None
    }
}

pub(crate) fn cf_type_id(val: CfTypeRef) -> usize {
    unsafe {
        // SAFETY: `val` is a live CF object.
        CFGetTypeID(val)
    }
}

pub(crate) fn cf_release(val: CfTypeRef) {
    unsafe {
        // SAFETY: caller owns `val`.
        CFRelease(val);
    }
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFDictionaryGetTypeID() -> usize;
}

pub(crate) fn dict_type_id() -> usize {
    unsafe {
        // SAFETY: CFDictionaryGetTypeID is a constant query.
        CFDictionaryGetTypeID()
    }
}

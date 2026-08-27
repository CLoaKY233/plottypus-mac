use plottypus_core::{FanSnapshot, SensorsSnapshot};

pub(crate) struct FanCollector {
    inner: Inner,
    hid: crate::hid::HidClient,
}

enum Inner {
    #[cfg(target_os = "macos")]
    Mac(macos::Client),
    Empty,
}

impl FanCollector {
    pub(crate) fn new() -> Self {
        #[cfg(target_os = "macos")]
        {
            Self {
                inner: macos::Client::open().map_or(Inner::Empty, Inner::Mac),
                hid: crate::hid::HidClient::new(),
            }
        }
        #[cfg(not(target_os = "macos"))]
        {
            Self {
                inner: Inner::Empty,
                hid: crate::hid::HidClient::new(),
            }
        }
    }

    pub(crate) fn sample(&mut self) -> FanSnapshot {
        match &mut self.inner {
            #[cfg(target_os = "macos")]
            Inner::Mac(client) => client.sample(),
            Inner::Empty => FanSnapshot::default(),
        }
    }

    pub(crate) fn sample_sensors(&mut self) -> SensorsSnapshot {
        let smc = match &mut self.inner {
            #[cfg(target_os = "macos")]
            Inner::Mac(client) => client.sample_sensors(),
            Inner::Empty => SensorsSnapshot::default(),
        };
        let hid = self.hid.sample();
        crate::zones::merge_sensors(smc, &hid)
    }
}

/// Decode SMC numeric bytes into RPM. Exported for tests.
pub(crate) fn decode_rpm(data_type: u32, bytes: &[u8], size: usize) -> Option<u16> {
    let rpm = decode_numeric(data_type, bytes, size)?;
    if !(0.0..=20_000.0).contains(&rpm) || !rpm.is_finite() {
        return None;
    }
    Some(rpm.round() as u16)
}

pub(crate) fn fourcc(key: [u8; 4]) -> u32 {
    u32::from_be_bytes(key)
}

pub(crate) fn type_code(label: [u8; 4]) -> u32 {
    u32::from_be_bytes(label)
}

pub(crate) fn decode_temp(data_type: u32, bytes: &[u8], size: usize) -> Option<f32> {
    let t = decode_numeric(data_type, bytes, size)?;
    if t.is_finite() && t > 0.0 && t <= 150.0 {
        Some(t)
    } else {
        None
    }
}

fn decode_numeric(data_type: u32, bytes: &[u8], size: usize) -> Option<f32> {
    if size == 0 || bytes.is_empty() {
        return None;
    }
    if data_type == type_code(*b"flt ") {
        let raw: [u8; 4] = bytes.get(..4)?.try_into().ok()?;
        return Some(f32::from_le_bytes(raw));
    }
    if data_type == type_code(*b"fpe2") {
        let raw: [u8; 2] = bytes.get(..2)?.try_into().ok()?;
        return Some(f32::from(u16::from_be_bytes(raw)) / 4.0);
    }
    if data_type == type_code(*b"ui8 ") || size == 1 {
        return Some(f32::from(*bytes.first()?));
    }
    if data_type == type_code(*b"ui16") || size == 2 {
        let raw: [u8; 2] = bytes.get(..2)?.try_into().ok()?;
        return Some(f32::from(u16::from_be_bytes(raw)));
    }
    if data_type == type_code(*b"ui32") || size == 4 {
        let raw: [u8; 4] = bytes.get(..4)?.try_into().ok()?;
        return Some(u32::from_be_bytes(raw) as f32);
    }
    if data_type == type_code(*b"sp78") {
        let raw: [u8; 2] = bytes.get(..2)?.try_into().ok()?;
        return Some(f32::from(i16::from_be_bytes(raw)) / 256.0);
    }
    None
}

#[cfg(target_os = "macos")]
mod macos {
    use super::{decode_rpm, decode_temp, fourcc};
    use plottypus_core::{FanMetric, FanSnapshot, SensorsSnapshot};
    use std::mem::size_of;
    use std::ptr;

    const KERNEL_INDEX_SMC: u32 = 2;
    const SMC_CMD_READ_BYTES: u8 = 5;
    const SMC_CMD_READ_INDEX: u8 = 8;
    const SMC_CMD_READ_KEYINFO: u8 = 9;
    const MAX_FANS: u8 = 8;
    const PROBE_TEMPS: [[u8; 4]; 24] = [
        *b"TCMz", *b"TCMb", *b"TC0P", *b"TC0D", *b"TC0C", *b"TC0E", *b"TG0P", *b"Tg05", *b"Tg0D",
        *b"Tg0L", *b"Tg0T", *b"Tp01", *b"Tp05", *b"Tp09", *b"Tp0T", *b"Tp0D", *b"Te05", *b"Ts0P",
        *b"TH0x", *b"TH0T", *b"TA0P", *b"TaLC", *b"Tm02", *b"Tm06",
    ];

    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    pub(super) struct SmcKeyData {
        key: u32,
        vers: [u8; 6],
        _vers_pad: [u8; 2],
        p_limit: [u8; 16],
        data_size: u32,
        data_type: u32,
        data_attributes: u8,
        _info_pad: [u8; 3],
        result: u8,
        status: u8,
        data8: u8,
        _cmd_pad: u8,
        data32: u32,
        bytes: [u8; 32],
    }

    #[link(name = "IOKit", kind = "framework")]
    unsafe extern "C" {
        fn IOServiceMatching(name: *const libc::c_char) -> *const std::ffi::c_void;
        fn IOServiceGetMatchingServices(
            port: u32,
            matching: *const std::ffi::c_void,
            existing: *mut u32,
        ) -> libc::kern_return_t;
        fn IOIteratorNext(iterator: u32) -> u32;
        fn IOObjectRelease(obj: u32) -> libc::kern_return_t;
        fn IOServiceOpen(service: u32, owning_task: u32, type_: u32, connect: *mut u32) -> i32;
        fn IOServiceClose(connect: u32) -> i32;
        fn IORegistryEntryGetName(entry: u32, name: *mut libc::c_char) -> libc::kern_return_t;
        fn IOConnectCallStructMethod(
            connection: u32,
            selector: u32,
            input: *const std::ffi::c_void,
            input_size: usize,
            output: *mut std::ffi::c_void,
            output_size: *mut usize,
        ) -> i32;
        fn mach_task_self() -> u32;
    }

    #[derive(Clone, Copy)]
    struct FanKey {
        index: u8,
        actual: u32,
        max: u32,
    }

    pub(super) struct Client {
        connect: u32,
        service: u32,
        keys: Vec<FanKey>,
        temp_keys: Vec<(u32, String)>,
    }

    impl Client {
        pub(super) fn open() -> Option<Self> {
            let matching = unsafe { IOServiceMatching(c"AppleSMC".as_ptr()) };
            if matching.is_null() {
                return None;
            }
            let mut iter: u32 = 0;
            let kr = unsafe { IOServiceGetMatchingServices(0, matching, &raw mut iter) };
            if kr != 0 {
                return None;
            }
            let (service, connect) = open_endpoint(iter)?;
            let mut client = Self {
                connect,
                service,
                keys: Vec::new(),
                temp_keys: Vec::new(),
            };
            client.keys = client.discover();
            client.temp_keys = client.discover_temps();
            Some(client)
        }

        fn discover(&self) -> Vec<FanKey> {
            let mut keys = Vec::new();
            let count = self
                .read_key(fourcc(*b"FNum"))
                .and_then(|(ty, bytes, size)| decode_rpm(ty, &bytes, size))
                .unwrap_or(MAX_FANS.into())
                .min(u16::from(MAX_FANS));
            for i in 0..count as u8 {
                let actual = fan_key(i, *b"Ac");
                if self.read_key(actual).is_some() {
                    keys.push(FanKey {
                        index: i,
                        actual,
                        max: fan_key(i, *b"Mx"),
                    });
                }
            }
            if keys.is_empty() {
                for key in self.list_keys() {
                    let bytes = key.to_be_bytes();
                    if bytes[0] == b'F' && bytes[2] == b'A' && bytes[3] == b'c' {
                        let index = bytes[1].saturating_sub(b'0');
                        keys.push(FanKey {
                            index,
                            actual: key,
                            max: fourcc([b'F', bytes[1], b'M', b'x']),
                        });
                    }
                }
            }
            keys
        }

        pub(super) fn sample(&self) -> FanSnapshot {
            let mut fans = Vec::new();
            for key in &self.keys {
                let Some((ty, bytes, size)) = self.read_key(key.actual) else {
                    continue;
                };
                let Some(rpm) = decode_rpm(ty, &bytes, size) else {
                    continue;
                };
                let max_rpm = self
                    .read_key(key.max)
                    .and_then(|(ty, bytes, size)| decode_rpm(ty, &bytes, size))
                    .unwrap_or(0);
                fans.push(FanMetric {
                    name: format!("Fan {}", key.index + 1),
                    rpm,
                    max_rpm,
                });
            }
            FanSnapshot { fans }
        }

        pub(super) fn sample_sensors(&self) -> SensorsSnapshot {
            let mut named = Vec::new();
            for (key, name) in &self.temp_keys {
                let Some((ty, bytes, size)) = self.read_key(*key) else {
                    continue;
                };
                let Some(c) = decode_temp(ty, &bytes, size) else {
                    continue;
                };
                named.push((name.clone(), c));
            }
            crate::zones::snapshot_from_named(&named, crate::zones::Source::Smc)
        }

        fn discover_temps(&self) -> Vec<(u32, String)> {
            let mut found = Vec::new();
            let listed = self.list_keys();
            let candidates: Vec<u32> = if listed.is_empty() {
                PROBE_TEMPS.iter().map(|k| fourcc(*k)).collect()
            } else {
                listed
                    .into_iter()
                    .filter(|k| {
                        let b = k.to_be_bytes();
                        b.first() == Some(&b'T')
                    })
                    .collect()
            };
            for key in candidates {
                let Some((ty, bytes, size)) = self.read_key(key) else {
                    continue;
                };
                if decode_temp(ty, &bytes, size).is_none() {
                    continue;
                }
                found.push((key, key_name(key)));
                if found.len() >= 32 {
                    break;
                }
            }
            found
        }

        fn list_keys(&self) -> Vec<u32> {
            let count = self
                .read_key(fourcc(*b"#KEY"))
                .and_then(|(ty, bytes, size)| decode_rpm(ty, &bytes, size))
                .map_or(256, u32::from)
                .min(512);
            let mut keys = Vec::new();
            for i in 0..count {
                let input = SmcKeyData {
                    data8: SMC_CMD_READ_INDEX,
                    data32: i,
                    ..SmcKeyData::default()
                };
                let Some(out) = self.call(&input) else {
                    break;
                };
                let key = out.key;
                if key == 0 {
                    continue;
                }
                keys.push(key);
            }
            keys
        }

        fn read_key(&self, key: u32) -> Option<(u32, [u8; 32], usize)> {
            let mut input = SmcKeyData {
                key,
                data8: SMC_CMD_READ_KEYINFO,
                ..SmcKeyData::default()
            };
            let info = self.call(&input)?;
            let size = {
                let s = info.data_size;
                s as usize
            };
            if size == 0 || size > 32 {
                return None;
            }
            let data_type = info.data_type;
            let data_size = info.data_size;
            input.data8 = SMC_CMD_READ_BYTES;
            input.data_size = data_size;
            input.data_type = data_type;
            let data = self.call(&input)?;
            Some((data_type, data.bytes, size))
        }

        fn call(&self, input: &SmcKeyData) -> Option<SmcKeyData> {
            let mut output = SmcKeyData::default();
            let mut out_size = size_of::<SmcKeyData>();
            let kr = unsafe {
                // SAFETY: input/output are packed SMC structs; sizes match IOConnectCallStructMethod.
                IOConnectCallStructMethod(
                    self.connect,
                    KERNEL_INDEX_SMC,
                    ptr::from_ref(input).cast(),
                    size_of::<SmcKeyData>(),
                    ptr::from_mut(&mut output).cast(),
                    &raw mut out_size,
                )
            };
            if kr != 0 {
                return None;
            }
            Some(output)
        }
    }

    impl Drop for Client {
        fn drop(&mut self) {
            if self.connect != 0 {
                unsafe { IOServiceClose(self.connect) };
            }
            if self.service != 0 {
                unsafe { IOObjectRelease(self.service) };
            }
        }
    }

    fn open_endpoint(iter: u32) -> Option<(u32, u32)> {
        let mut fallback: Option<(u32, u32)> = None;
        loop {
            let service = unsafe { IOIteratorNext(iter) };
            if service == 0 {
                break;
            }
            let name = registry_name(service);
            let mut connect: u32 = 0;
            let open = unsafe { IOServiceOpen(service, mach_task_self(), 0, &raw mut connect) };
            if open != 0 || connect == 0 {
                unsafe { IOObjectRelease(service) };
                continue;
            }
            if name == "AppleSMCKeysEndpoint" {
                unsafe { IOObjectRelease(iter) };
                return Some((service, connect));
            }
            if fallback.is_none() {
                fallback = Some((service, connect));
            } else {
                unsafe {
                    IOServiceClose(connect);
                    IOObjectRelease(service);
                }
            }
        }
        unsafe { IOObjectRelease(iter) };
        fallback
    }

    fn registry_name(service: u32) -> String {
        let mut buf = [0_i8; 128];
        let kr = unsafe { IORegistryEntryGetName(service, buf.as_mut_ptr()) };
        if kr != 0 {
            return String::new();
        }
        let bytes: Vec<u8> = buf
            .iter()
            .take_while(|&&c| c != 0)
            .map(|&c| c as u8)
            .collect();
        String::from_utf8_lossy(&bytes).into_owned()
    }

    fn fan_key(index: u8, suffix: [u8; 2]) -> u32 {
        let digit = b'0'.saturating_add(index);
        fourcc([b'F', digit, suffix[0], suffix[1]])
    }

    fn key_name(key: u32) -> String {
        String::from_utf8_lossy(&key.to_be_bytes())
            .trim()
            .to_string()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn smc_struct_is_80_bytes() {
        #[cfg(target_os = "macos")]
        assert_eq!(std::mem::size_of::<macos::SmcKeyData>(), 80);
    }

    #[test]
    fn fourcc_is_be() {
        assert_eq!(fourcc(*b"F0Ac"), 0x4630_4163);
        assert_eq!(type_code(*b"flt "), 0x666c_7420);
    }

    #[test]
    fn decode_fpe2() {
        let bytes = [0x1c, 0x20, 0, 0];
        let rpm = decode_rpm(type_code(*b"fpe2"), &bytes, 2).unwrap();
        assert_eq!(rpm, 1800);
    }

    #[test]
    fn decode_flt() {
        let bits = 1850.0_f32.to_le_bytes();
        let rpm = decode_rpm(type_code(*b"flt "), &bits, 4).unwrap();
        assert_eq!(rpm, 1850);
    }

    #[test]
    fn decode_rejects_garbage() {
        let bits = 99_000.0_f32.to_le_bytes();
        assert!(decode_rpm(type_code(*b"flt "), &bits, 4).is_none());
        assert!(decode_rpm(type_code(*b"flt "), &[], 0).is_none());
    }

    #[test]
    fn decode_temp_accepts_celsius() {
        let bits = 42.5_f32.to_le_bytes();
        let t = decode_temp(type_code(*b"flt "), &bits, 4).unwrap();
        assert!((t - 42.5).abs() < 0.01);
        let hot = 180.0_f32.to_le_bytes();
        assert!(decode_temp(type_code(*b"flt "), &hot, 4).is_none());
    }

    #[test]
    fn new_and_sample_do_not_panic() {
        let mut col = FanCollector::new();
        let snap = col.sample();
        for fan in &snap.fans {
            if fan.max_rpm > 0 {
                assert!(fan.rpm <= fan.max_rpm.saturating_add(200));
            }
        }
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn live_fan_sample() {
        let mut col = FanCollector::new();
        let snap = col.sample();
        if snap.is_present() {
            assert!(!snap.fans.is_empty());
        }
        let temps = col.sample_sensors();
        if let Some(c) = temps.cpu_c.or(temps.hotspot_c) {
            assert!((0.0..=150.0).contains(&c), "{c}");
        }
    }
}

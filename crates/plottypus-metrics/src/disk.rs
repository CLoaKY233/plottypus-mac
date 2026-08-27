use std::time::Instant;

use plottypus_core::{DiskSnapshot, DiskVolume};

const SKIP_FSTYPES: &[&str] = &["devfs", "autofs", "fdesc"];

const SKIP_MOUNT_PREFIXES: &[&str] = &[
    "/dev",
    "/System/Volumes/VM",
    "/System/Volumes/Preboot",
    "/System/Volumes/Update",
    "/System/Volumes/xarts",
    "/System/Volumes/iSCPreboot",
    "/System/Volumes/Hardware",
];

pub(crate) struct DiskCollector {
    prev: Option<(Instant, u64, u64)>,
    volumes: Vec<DiskVolume>,
    #[cfg(target_os = "macos")]
    ports: Vec<u32>,
}

impl DiskCollector {
    pub(crate) fn new() -> Self {
        Self {
            prev: None,
            volumes: Vec::new(),
            #[cfg(target_os = "macos")]
            ports: Vec::new(),
        }
    }

    pub(crate) fn refresh_volumes(&mut self) {
        #[cfg(target_os = "macos")]
        {
            self.volumes = macos::volumes();
        }
    }

    pub(crate) fn sample(&mut self) -> DiskSnapshot {
        if self.volumes.is_empty() {
            self.refresh_volumes();
        }
        #[cfg(target_os = "macos")]
        {
            macos::sample(self)
        }
        #[cfg(not(target_os = "macos"))]
        {
            DiskSnapshot {
                volumes: self.volumes.clone(),
                ..DiskSnapshot::default()
            }
        }
    }
}

#[cfg(target_os = "macos")]
impl Drop for DiskCollector {
    fn drop(&mut self) {
        macos::release_ports(&mut self.ports);
    }
}

fn is_apfs_or_hfs(fstype: &str) -> bool {
    fstype.eq_ignore_ascii_case("apfs") || fstype.eq_ignore_ascii_case("hfs")
}

fn skipped_fstype(fstype: &str) -> bool {
    SKIP_FSTYPES
        .iter()
        .any(|skip| fstype.eq_ignore_ascii_case(skip))
}

fn skipped_mount(mount: &str) -> bool {
    SKIP_MOUNT_PREFIXES.iter().any(|prefix| {
        mount
            .strip_prefix(prefix)
            .is_some_and(|rest| rest.is_empty() || rest.starts_with('/'))
    })
}

fn is_user_volume(mount: &str) -> bool {
    mount
        .strip_prefix("/Volumes/")
        .is_some_and(|rest| !rest.is_empty())
}

fn keep_volume(fstype: &str, mount: &str) -> bool {
    if skipped_fstype(fstype) || skipped_mount(mount) || mount.is_empty() {
        return false;
    }
    mount == "/" || is_user_volume(mount) || is_apfs_or_hfs(fstype)
}

fn volume_name(mount: &str) -> String {
    if mount == "/" {
        return String::from("Macintosh HD");
    }
    mount
        .rsplit('/')
        .find(|part| !part.is_empty())
        .unwrap_or(mount)
        .to_owned()
}

fn prefer_root_over_data(volumes: Vec<DiskVolume>) -> Vec<DiskVolume> {
    let has_root = volumes.iter().any(|v| v.mount == "/");
    if !has_root {
        return volumes;
    }
    volumes
        .into_iter()
        .filter(|v| v.mount != "/System/Volumes/Data")
        .collect()
}

#[cfg(target_os = "macos")]
mod macos {
    use super::{DiskCollector, keep_volume, prefer_root_over_data, volume_name};
    use crate::iokit::{self, CfDictRef};
    use plottypus_core::{DiskSnapshot, DiskVolume};
    use std::ptr;
    use std::time::Instant;

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
        *ports = iokit::matching_services(c"IOBlockStorageDriver");
    }

    pub(super) fn sample(col: &mut DiskCollector) -> DiskSnapshot {
        let volumes = col.volumes.clone();
        let (read, write) = io_bytes(&mut col.ports);
        let now = Instant::now();
        let (read_bps, write_bps) = if let Some((t0, r0, w0)) = col.prev {
            let dt = now.saturating_duration_since(t0).as_secs_f64().max(0.001);
            (
                (read.saturating_sub(r0) as f64 / dt) as u64,
                (write.saturating_sub(w0) as f64 / dt) as u64,
            )
        } else {
            (0, 0)
        };
        col.prev = Some((now, read, write));
        DiskSnapshot {
            volumes,
            read_bps,
            write_bps,
        }
    }

    pub(super) fn volumes() -> Vec<DiskVolume> {
        let mut buf: *mut libc::statfs = ptr::null_mut();
        let n = unsafe {
            // SAFETY: `buf` is an out-pointer; getmntinfo writes a libc-owned array.
            libc::getmntinfo(&raw mut buf, libc::MNT_NOWAIT)
        };
        if n <= 0 || buf.is_null() {
            return Vec::new();
        }
        let entries = unsafe {
            // SAFETY: getmntinfo returned `n` statfs records in a static buffer.
            std::slice::from_raw_parts(buf, n as usize)
        };
        let mut volumes = Vec::new();
        for fs in entries {
            let mount = c_fixed_str(&fs.f_mntonname);
            let fstype = c_fixed_str(&fs.f_fstypename);
            if !keep_volume(&fstype, &mount) {
                continue;
            }
            let bsize = block_size(fs);
            let total_bytes = fs.f_blocks.saturating_mul(bsize);
            let used_bytes = fs.f_blocks.saturating_sub(fs.f_bfree).saturating_mul(bsize);
            let name = volume_name(&mount);
            volumes.push(DiskVolume {
                name,
                mount,
                used_bytes,
                total_bytes,
            });
        }
        prefer_root_over_data(volumes)
    }

    fn block_size(fs: &libc::statfs) -> u64 {
        if fs.f_bsize > 0 {
            u64::from(fs.f_bsize)
        } else if fs.f_iosize > 0 {
            fs.f_iosize as u64
        } else {
            0
        }
    }

    fn c_fixed_str(buf: &[libc::c_char]) -> String {
        let n = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
        let bytes: Vec<u8> = buf[..n].iter().map(|&c| c as u8).collect();
        String::from_utf8_lossy(&bytes).into_owned()
    }

    fn io_bytes(ports: &mut Vec<u32>) -> (u64, u64) {
        if ports.is_empty() {
            rematch(ports);
        }
        let mut read = 0_u64;
        let mut write = 0_u64;
        let mut any = false;
        for &service in ports.iter() {
            if let Some((r, w)) = bytes_for_service(service) {
                any = true;
                read = read.saturating_add(r);
                write = write.saturating_add(w);
            }
        }
        if !any {
            rematch(ports);
            for &service in ports.iter() {
                if let Some((r, w)) = bytes_for_service(service) {
                    read = read.saturating_add(r);
                    write = write.saturating_add(w);
                }
            }
        }
        (read, write)
    }

    fn bytes_for_service(service: u32) -> Option<(u64, u64)> {
        let mut props: CfDictRef = ptr::null();
        let kr = unsafe {
            // SAFETY: `props` is an out-pointer; IOKit writes a +1 CF dict on success.
            IORegistryEntryCreateCFProperties(service, &raw mut props, ptr::null(), 0)
        };
        if kr != 0 || props.is_null() {
            return None;
        }
        let pair = iokit::dict_get(props, c"Statistics").and_then(|stats| {
            if iokit::cf_type_id(stats) != iokit::dict_type_id() {
                return None;
            }
            Some((
                iokit::dict_u64(stats, c"Bytes (Read)").unwrap_or(0),
                iokit::dict_u64(stats, c"Bytes (Write)").unwrap_or(0),
            ))
        });
        iokit::cf_release(props);
        pair
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    fn vol(name: &str, mount: &str) -> DiskVolume {
        DiskVolume {
            name: String::from(name),
            mount: String::from(mount),
            used_bytes: 1,
            total_bytes: 2,
        }
    }

    #[test]
    fn first_sample_rates_are_zero() {
        let mut col = DiskCollector::new();
        let snap = col.sample();
        assert_eq!(snap.read_bps, 0);
        assert_eq!(snap.write_bps, 0);
    }

    #[test]
    fn keep_root_and_user_volumes() {
        assert!(keep_volume("apfs", "/"));
        assert!(keep_volume("hfs", "/"));
        assert!(keep_volume("apfs", "/Volumes/External"));
        assert!(keep_volume("exfat", "/Volumes/USB"));
        assert!(keep_volume("APFS", "/"));
        assert!(keep_volume("apfs", "/System/Volumes/Data"));
    }

    #[test]
    fn skip_virtual_and_system_mounts() {
        assert!(!keep_volume("devfs", "/dev"));
        assert!(!keep_volume("autofs", "/System/Volumes/Data/home"));
        assert!(!keep_volume("fdesc", "/dev"));
        assert!(!keep_volume("apfs", "/dev"));
        assert!(!keep_volume("apfs", "/dev/disk3s1"));
        assert!(!keep_volume("apfs", "/System/Volumes/VM"));
        assert!(!keep_volume("apfs", "/System/Volumes/Preboot"));
        assert!(!keep_volume("apfs", "/System/Volumes/Update"));
        assert!(!keep_volume("apfs", "/System/Volumes/Update/mnt1"));
        assert!(!keep_volume("apfs", "/System/Volumes/xarts"));
        assert!(!keep_volume("apfs", "/System/Volumes/iSCPreboot"));
        assert!(!keep_volume("apfs", "/System/Volumes/Hardware"));
        assert!(!keep_volume("nfs", "/mnt/server"));
        assert!(!keep_volume("apfs", ""));
    }

    #[test]
    fn prefer_root_drops_data_firmlink() {
        let out = prefer_root_over_data(vec![
            vol("Data", "/System/Volumes/Data"),
            vol("Macintosh HD", "/"),
        ]);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].mount, "/");
        assert_eq!(out[0].name, "Macintosh HD");
    }

    #[test]
    fn data_volume_kept_when_root_absent() {
        let out = prefer_root_over_data(vec![vol("Data", "/System/Volumes/Data")]);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].mount, "/System/Volumes/Data");
    }

    #[test]
    fn volume_name_from_mount() {
        assert_eq!(volume_name("/"), "Macintosh HD");
        assert_eq!(volume_name("/Volumes/External SSD"), "External SSD");
        assert_eq!(volume_name("/System/Volumes/Data"), "Data");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn live_volumes_or_no_panic() {
        let mut col = DiskCollector::new();
        let snap = col.sample();
        if let Some(primary) = snap.primary() {
            assert!(primary.total_bytes > 0, "primary volume has no size");
        }
    }
}

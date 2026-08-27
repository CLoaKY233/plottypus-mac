use std::time::Instant;

use plottypus_core::NetworkSnapshot;

#[derive(Default)]
pub(crate) struct NetCollector {
    prev: Option<(Instant, u64, u64, String)>,
}

impl NetCollector {
    pub(crate) fn new() -> Self {
        Self { prev: None }
    }

    pub(crate) fn sample(&mut self) -> NetworkSnapshot {
        #[cfg(target_os = "macos")]
        {
            macos::sample(self)
        }
        #[cfg(not(target_os = "macos"))]
        {
            NetworkSnapshot::default()
        }
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use super::NetCollector;
    use plottypus_core::NetworkSnapshot;
    use std::ffi::CStr;
    use std::ptr;
    use std::time::Instant;

    pub(super) fn sample(col: &mut NetCollector) -> NetworkSnapshot {
        let (iface, rx, tx) = counters();
        let now = Instant::now();
        let snap = if let Some((t0, rx0, tx0, name)) = col.prev.as_ref() {
            let dt = now.saturating_duration_since(*t0).as_secs_f64().max(0.001);
            let same = name == &iface;
            NetworkSnapshot {
                iface: iface.clone(),
                rx_bps: if same {
                    ((rx.saturating_sub(*rx0) as f64 / dt) * 8.0) as u64
                } else {
                    0
                },
                tx_bps: if same {
                    ((tx.saturating_sub(*tx0) as f64 / dt) * 8.0) as u64
                } else {
                    0
                },
            }
        } else {
            NetworkSnapshot {
                iface: iface.clone(),
                rx_bps: 0,
                tx_bps: 0,
            }
        };
        col.prev = Some((now, rx, tx, iface));
        snap
    }

    fn counters() -> (String, u64, u64) {
        let mut ifap: *mut libc::ifaddrs = ptr::null_mut();
        let rc = unsafe {
            // SAFETY: `ifap` is an out-pointer; getifaddrs writes a linked list we free.
            libc::getifaddrs(&raw mut ifap)
        };
        if rc != 0 || ifap.is_null() {
            return (String::from("—"), 0, 0);
        }
        let mut best = (String::from("—"), 0_u64, 0_u64);
        let mut cur = ifap;
        while !cur.is_null() {
            // SAFETY: getifaddrs list, terminated by null.
            let node = unsafe {
                // SAFETY: `cur` walks the getifaddrs list until null.
                &*cur
            };
            if let Some(row) = read_link(node) {
                let total = row.1.saturating_add(row.2);
                let best_total = best.1.saturating_add(best.2);
                if total >= best_total && row.0 != "—" {
                    best = row;
                }
            }
            cur = node.ifa_next;
        }
        unsafe {
            // SAFETY: `ifap` is the list we got from getifaddrs.
            libc::freeifaddrs(ifap);
        }
        best
    }

    fn read_link(node: &libc::ifaddrs) -> Option<(String, u64, u64)> {
        let addr = node.ifa_addr;
        if addr.is_null() {
            return None;
        }
        let family = unsafe {
            // SAFETY: `addr` is a non-null sockaddr from ifaddrs.
            (*addr).sa_family
        };
        if i32::from(family) != libc::AF_LINK {
            return None;
        }
        let name = unsafe {
            // SAFETY: ifa_name is a kernel C string on this node.
            CStr::from_ptr(node.ifa_name)
        }
        .to_string_lossy()
        .into_owned();
        if name.starts_with("lo") || name.starts_with("awdl") || name.starts_with("llw") {
            return None;
        }
        if node.ifa_data.is_null() {
            return None;
        }
        let data = unsafe {
            // SAFETY: AF_LINK ifa_data is if_data on macOS.
            &*(node.ifa_data.cast::<libc::if_data>())
        };
        Some((name, u64::from(data.ifi_ibytes), u64::from(data.ifi_obytes)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_sample_is_zero_rate() {
        let mut col = NetCollector::new();
        let snap = col.sample();
        assert!(snap.rx_bps == 0 && snap.tx_bps == 0);
    }
}

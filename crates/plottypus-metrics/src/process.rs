use std::collections::HashMap;
use std::time::Instant;

use plottypus_core::{Process, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Signal {
    Term,
    Kill,
    Int,
}

struct CachedProc {
    name: String,
    cpu_ns: u64,
    start_sec: i64,
}

pub(crate) struct ProcessCollector {
    cache: HashMap<u32, CachedProc>,
    last: Option<Instant>,
    ncpu: u32,
}

impl ProcessCollector {
    pub(crate) fn new() -> Self {
        Self {
            cache: HashMap::new(),
            last: None,
            ncpu: logical_cpus(),
        }
    }

    pub(crate) fn sample(&mut self) -> Result<Vec<Process>> {
        #[cfg(target_os = "macos")]
        {
            macos::sample(self)
        }
        #[cfg(not(target_os = "macos"))]
        {
            Ok(Vec::new())
        }
    }
}

fn logical_cpus() -> u32 {
    #[cfg(target_os = "macos")]
    {
        crate::sys::logical_cpus()
    }
    #[cfg(not(target_os = "macos"))]
    {
        1
    }
}

pub fn send_signal(pid: u32, signal: Signal) -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        macos::send_signal(pid, signal)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (pid, signal);
        Ok(())
    }
}

/// Process CPU percent of the whole machine (`Δ / (interval × ncpu) × 100`).
pub(crate) fn cpu_percent(delta_ns: u64, interval_ns: u128, ncpu: u32) -> f32 {
    if interval_ns == 0 || ncpu == 0 {
        return 0.0;
    }
    let denom = interval_ns as f64 * f64::from(ncpu);
    ((delta_ns as f64) / denom * 100.0) as f32
}

pub(crate) fn basename(path: &str) -> &str {
    path.rsplit('/')
        .next()
        .filter(|s| !s.is_empty())
        .unwrap_or(path)
}

pub(crate) fn libc_signal(signal: Signal) -> libc::c_int {
    match signal {
        Signal::Term => libc::SIGTERM,
        Signal::Kill => libc::SIGKILL,
        Signal::Int => libc::SIGINT,
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use super::{CachedProc, ProcessCollector, Signal, basename, cpu_percent, libc_signal};
    use plottypus_core::{Error, Process, Result};
    use std::collections::HashSet;
    use std::ffi::CStr;
    use std::mem::{MaybeUninit, size_of};
    use std::time::Instant;

    const KINFO_PROC_LEN: usize = 648;
    const OFF_START_SEC: usize = 0;
    const OFF_PID: usize = 40;
    const OFF_COMM: usize = 243;
    const COMM_LEN: usize = 17;
    const OFF_PPID: usize = 560;

    struct Kinfo {
        pid: u32,
        ppid: u32,
        start_sec: i64,
        comm: String,
    }

    pub(super) fn sample(collector: &mut ProcessCollector) -> Result<Vec<Process>> {
        let now = Instant::now();
        let interval_ns = collector
            .last
            .map_or(0, |prev| now.saturating_duration_since(prev).as_nanos());
        collector.last = Some(now);

        let infos = kinfo_all()?;
        let live: HashSet<u32> = infos.iter().map(|k| k.pid).collect();
        collector.cache.retain(|pid, _| live.contains(pid));

        let ncpu = collector.ncpu.max(1);
        let mut out = Vec::with_capacity(infos.len());
        for info in infos {
            out.push(sample_one(collector, &info, interval_ns, ncpu));
        }
        Ok(out)
    }

    fn sample_one(
        collector: &mut ProcessCollector,
        info: &Kinfo,
        interval_ns: u128,
        ncpu: u32,
    ) -> Process {
        let cached = collector.cache.get(&info.pid);
        let name = match cached {
            Some(c) if c.start_sec == info.start_sec && !c.name.is_empty() => c.name.clone(),
            _ => resolve_name(info.pid, &info.comm),
        };

        let task = task_info(info.pid);
        let (cpu, cpu_ns, mem_bytes, threads) = match task {
            Some(task) => {
                let cpu_ticks = task.pti_total_user.saturating_add(task.pti_total_system);
                let cpu_ns = crate::sys::mach_ticks_to_ns(cpu_ticks);
                let prev_ns = cached
                    .filter(|c| c.start_sec == info.start_sec)
                    .map(|c| c.cpu_ns);
                let cpu = prev_ns.map_or(0.0, |prev| {
                    cpu_percent(cpu_ns.saturating_sub(prev), interval_ns, ncpu)
                });
                let mem_bytes = task.pti_resident_size;
                let threads = u32::try_from(task.pti_threadnum.max(0)).unwrap_or(0);
                (cpu, cpu_ns, mem_bytes, threads)
            }
            None => (0.0, cached.map_or(0, |c| c.cpu_ns), 0, 0),
        };

        collector.cache.insert(
            info.pid,
            CachedProc {
                name: name.clone(),
                cpu_ns,
                start_sec: info.start_sec,
            },
        );

        Process {
            pid: info.pid,
            ppid: info.ppid,
            name,
            cpu,
            mem_bytes,
            threads,
            gpu: 0.0,
        }
    }

    fn kinfo_all() -> Result<Vec<Kinfo>> {
        let mut mib = [libc::CTL_KERN, libc::KERN_PROC, libc::KERN_PROC_ALL, 0];
        let mut buf = Vec::new();
        for _ in 0..4 {
            let mut needed = 0usize;
            let probe = unsafe {
                // SAFETY: size probe; oldp is null.
                libc::sysctl(
                    mib.as_mut_ptr(),
                    mib.len() as libc::c_uint,
                    std::ptr::null_mut(),
                    &raw mut needed,
                    std::ptr::null_mut(),
                    0,
                )
            };
            if probe != 0 {
                return Err(Error::system(format!(
                    "KERN_PROC_ALL size: {}",
                    std::io::Error::last_os_error()
                )));
            }
            let cap = needed
                .saturating_add(needed / 4)
                .saturating_add(KINFO_PROC_LEN);
            buf.resize(cap, 0);
            let mut got = buf.len();
            let rc = unsafe {
                // SAFETY: `buf` is writable for `got` bytes.
                libc::sysctl(
                    mib.as_mut_ptr(),
                    mib.len() as libc::c_uint,
                    buf.as_mut_ptr().cast(),
                    &raw mut got,
                    std::ptr::null_mut(),
                    0,
                )
            };
            if rc == 0 {
                buf.truncate(got);
                return Ok(parse_kinfo(&buf));
            }
            let err = std::io::Error::last_os_error();
            if err.raw_os_error() != Some(libc::ENOMEM) {
                return Err(Error::system(format!("KERN_PROC_ALL: {err}")));
            }
        }
        Err(Error::system("KERN_PROC_ALL: table grew too fast"))
    }

    fn parse_kinfo(buf: &[u8]) -> Vec<Kinfo> {
        buf.as_chunks::<KINFO_PROC_LEN>()
            .0
            .iter()
            .filter_map(parse_one_kinfo)
            .collect()
    }

    fn parse_one_kinfo(chunk: &[u8; KINFO_PROC_LEN]) -> Option<Kinfo> {
        let proc_id = i32_at(chunk, OFF_PID)?;
        if proc_id < 1 {
            return None;
        }
        let parent = i32_at(chunk, OFF_PPID).unwrap_or(0).max(0);
        let start_sec = i64_at(chunk, OFF_START_SEC).unwrap_or(0);
        let comm = comm_at(chunk, OFF_COMM);
        Some(Kinfo {
            pid: proc_id as u32,
            ppid: parent as u32,
            start_sec,
            comm,
        })
    }

    fn i32_at(buf: &[u8], off: usize) -> Option<i32> {
        buf.get(off..off + 4)?
            .try_into()
            .ok()
            .map(i32::from_ne_bytes)
    }

    fn i64_at(buf: &[u8], off: usize) -> Option<i64> {
        buf.get(off..off + 8)?
            .try_into()
            .ok()
            .map(i64::from_ne_bytes)
    }

    fn comm_at(buf: &[u8], off: usize) -> String {
        let Some(raw) = buf.get(off..off + COMM_LEN) else {
            return String::new();
        };
        let end = raw.iter().position(|&b| b == 0).unwrap_or(raw.len());
        String::from_utf8_lossy(&raw[..end]).into_owned()
    }

    fn resolve_name(pid: u32, comm: &str) -> String {
        let argv0 = pid_argv0(pid);
        let path = pid_path(pid);
        preferred_name(pid, argv0.as_deref(), path.as_deref(), comm)
    }

    /// How the process was invoked (`KERN_PROCARGS2` argv[0]). Matches what
    /// `ps -o comm=` shows: launchers and symlinks keep their own name even
    /// when the real executable lives in a version folder.
    fn pid_argv0(pid: u32) -> Option<String> {
        let mut mib = [libc::CTL_KERN, libc::KERN_PROCARGS2, pid as libc::c_int];
        let mut needed = 0usize;
        let probe = unsafe {
            // SAFETY: size probe; oldp is null.
            libc::sysctl(
                mib.as_mut_ptr(),
                mib.len() as libc::c_uint,
                std::ptr::null_mut(),
                &raw mut needed,
                std::ptr::null_mut(),
                0,
            )
        };
        if probe != 0 || !(8..=16 * 1024).contains(&needed) {
            return None;
        }
        let mut buf = vec![0_u8; needed];
        let mut got = buf.len();
        let rc = unsafe {
            // SAFETY: `buf` is writable for `got` bytes.
            libc::sysctl(
                mib.as_mut_ptr(),
                mib.len() as libc::c_uint,
                buf.as_mut_ptr().cast(),
                &raw mut got,
                std::ptr::null_mut(),
                0,
            )
        };
        if rc != 0 || got < 8 {
            return None;
        }
        buf.truncate(got);
        // Layout: [i32 argc][exec_path\0][NUL padding][argv0\0]…
        let argc = i32::from_ne_bytes(buf[..4].try_into().ok()?);
        if argc < 1 {
            return None;
        }
        let mut i = 4;
        while i < buf.len() && buf[i] != 0 {
            i += 1;
        }
        while i < buf.len() && buf[i] == 0 {
            i += 1;
        }
        let start = i;
        while i < buf.len() && buf[i] != 0 {
            i += 1;
        }
        if start == i {
            return None;
        }
        Some(String::from_utf8_lossy(&buf[start..i]).into_owned())
    }

    fn pid_path(pid: u32) -> Option<String> {
        let mut buf = [0u8; libc::PROC_PIDPATHINFO_MAXSIZE as usize];
        let n = unsafe {
            // SAFETY: `buf` is a writable path buffer of the documented max size.
            libc::proc_pidpath(
                pid as libc::c_int,
                buf.as_mut_ptr().cast(),
                buf.len() as u32,
            )
        };
        if n <= 0 {
            return None;
        }
        let cstr = CStr::from_bytes_until_nul(&buf).ok()?;
        cstr.to_str().ok().map(str::to_owned)
    }

    /// Display name priority: argv[0] basename (matches `ps`), real path
    /// basename, kernel `comm` (truncated to 16 bytes), then `pid N`.
    pub(super) fn preferred_name(
        pid: u32,
        argv0: Option<&str>,
        path: Option<&str>,
        comm: &str,
    ) -> String {
        for candidate in [argv0, path].into_iter().flatten() {
            let base = basename(candidate);
            if !base.is_empty() {
                return base.to_owned();
            }
        }
        if !comm.is_empty() {
            return comm.to_owned();
        }
        format!("pid {pid}")
    }

    fn task_info(pid: u32) -> Option<libc::proc_taskinfo> {
        let mut info = MaybeUninit::<libc::proc_taskinfo>::zeroed();
        let want = size_of::<libc::proc_taskinfo>() as libc::c_int;
        let n = unsafe {
            // SAFETY: PROC_PIDTASKINFO writes a proc_taskinfo into `info`.
            libc::proc_pidinfo(
                pid as libc::c_int,
                libc::PROC_PIDTASKINFO,
                0,
                info.as_mut_ptr().cast(),
                want,
            )
        };
        if n != want {
            return None;
        }
        Some(unsafe {
            // SAFETY: kernel wrote a full proc_taskinfo.
            info.assume_init()
        })
    }

    pub(super) fn send_signal(pid: u32, signal: Signal) -> Result<()> {
        if pid < 1 {
            return Err(Error::process(pid, "invalid pid"));
        }
        let raw = i32::try_from(pid).map_err(|_| Error::process(pid, "invalid pid"))?;
        let rc = unsafe {
            // SAFETY: kill(2) on a validated positive pid.
            libc::kill(raw, libc_signal(signal))
        };
        if rc == 0 {
            Ok(())
        } else {
            Err(Error::process(
                pid,
                std::io::Error::last_os_error().to_string(),
            ))
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn cpu_percent_from_deltas() {
        // 0.2s of CPU in 1s on 4 cores → 5%
        let pct = cpu_percent(200_000_000, 1_000_000_000, 4);
        assert!((pct - 5.0).abs() < 0.01, "{pct}");
        assert!((cpu_percent(10, 0, 4) - 0.0).abs() < f32::EPSILON);
        assert!((cpu_percent(10, 1_000, 0) - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn basename_from_path() {
        assert_eq!(
            basename("/Applications/Xcode.app/Contents/MacOS/Xcode"),
            "Xcode"
        );
        assert_eq!(basename("kernel_task"), "kernel_task");
        assert_eq!(basename("/"), "/");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn argv0_wins_over_version_folder_paths() {
        // Regression: /Users/x/.local/share/claude/versions/2.1.241 used to
        // display as "2.1.241" — both via path basename and, worse, via comm,
        // which is the executable file name and really is "2.1.241".
        assert_eq!(
            macos::preferred_name(
                50,
                Some("claude"),
                Some("/Users/x/.local/share/claude/versions/2.1.241"),
                "2.1.241"
            ),
            "claude"
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn name_falls_back_path_then_comm() {
        assert_eq!(
            macos::preferred_name(50, None, Some("/Applications/Safari.app/MacOS/Safari"), ""),
            "Safari"
        );
        // comm is capped at 16 bytes; better than nothing when args and path
        // are unreadable.
        assert_eq!(
            macos::preferred_name(50, None, None, "MTLCompilerServi"),
            "MTLCompilerServi"
        );
        assert_eq!(macos::preferred_name(50, Some("/"), Some(""), ""), "/");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn pid_label_when_nothing_else() {
        assert_eq!(macos::preferred_name(50, None, None, ""), "pid 50");
    }

    #[test]
    fn signal_numbers() {
        assert_eq!(libc_signal(Signal::Term), libc::SIGTERM);
        assert_eq!(libc_signal(Signal::Kill), libc::SIGKILL);
        assert_eq!(libc_signal(Signal::Int), libc::SIGINT);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn live_process_list_nonempty() {
        let mut col = ProcessCollector::new();
        let first = col.sample().expect("procs1");
        assert!(!first.is_empty());
        std::thread::sleep(std::time::Duration::from_millis(40));
        let second = col.sample().expect("procs2");
        assert!(!second.is_empty());
        let self_pid = std::process::id();
        assert!(
            second
                .iter()
                .any(|p| p.pid == self_pid && !p.name.is_empty()),
            "self pid {self_pid} missing"
        );
        assert!(second.iter().all(|p| p.pid >= 1));
        assert!(second.iter().all(|p| p.cpu.is_finite() && p.cpu >= 0.0));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn send_signal_missing_pid_errors() {
        let err = send_signal(u32::MAX, Signal::Int).expect_err("no such pid");
        let msg = err.to_string();
        assert!(msg.contains("process"), "{msg}");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn send_signal_rejects_pid_zero() {
        let err = send_signal(0, Signal::Term).expect_err("pid 0");
        assert!(err.to_string().contains("invalid pid"));
    }
}

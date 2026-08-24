# macOS metrics — how the machine actually talks

Last updated: 2026-08-24.

Focus: **Apple Silicon first**, Intel documented where it differs. Privilege: **no sudo** unless a row says otherwise. Cost is qualitative from reading macmon / macpow / btop / bottom / Stats / iSMC / Redline — we have not benchmarked Plottypus yet.

## 0. The five doors

Almost every number we want comes through one of these. Never spawn `powermetrics` in the default path.

| Door | What it is | Privilege | Cost | Good for |
| --- | --- | --- | --- | --- |
| **Mach / BSD** | `host_processor_info`, `host_statistics64`, `sysctl`, `libproc`, `getifaddrs`, `statvfs` | user | cheap | CPU %, RAM, swap, processes, net, disk *capacity* |
| **IOKit registry** | `IOServiceMatching`, `IORegistryEntryCreateCFProperties` | user | cheap–medium | Disk I/O, **IOAccelerator GPU % (Intel + AS)**, battery (IOPS), some SMC |
| **AppleSMC** | IOKit user client, 4-char keys | user to *read* | cheap if connection cached; do **not** scan all keys every tick | Fans, many temps, `PSTR` system power, battery keys |
| **IOReport** (private dylib) | `IOReportCreateSubscription` / `CreateSamples` / `CreateSamplesDelta` | user | medium; **requires a time window** | AS CPU/GPU residency & MHz, Energy Model watts (CPU/GPU/ANE/DRAM) |
| **IOHIDEventSystemClient** (private-ish) | AppleVendor temperature page `0xff00` / usage `0x0005` | user | medium | AS temps when SMC keys are empty (older macOS / M1) |

`powermetrics` is a *client of IOReport + HID*. macmon talks to the same backends directly. That is the whole trick.

Intel Macs: Mach + SMC are rich; IOReport Energy Model / GPU Performance States are an Apple Silicon (and some later) thing. Discrete GPUs: IOAccelerator / vendor libs, not IOReport.

## 1. CPU

### 1.1 Overall and per-logical-core % (all Macs)

```
host_processor_info(mach_host_self(), PROCESSOR_CPU_LOAD_INFO, &cpu_count, &info, &count)
```

Per CPU: `cpu_ticks[CPU_STATE_USER | NICE | SYSTEM | IDLE]`. **Delta two samples** / elapsed ticks = %.

btop: `src/osx/btop_collect.cpp` (~916, ~1034, ~1774). This is the portable "what Activity Monitor calls CPU".

- **Cost:** one Mach call, tiny.
- **Interval:** whatever the UI interval is. Need two samples; first tick is empty.
- **Caveat:** this is *residency in non-idle*, **not** frequency-weighted. A P-core at 600 MHz idle-ish and one at 4 GHz both report "100%" the same. On AS, also collect IOReport ratios (§1.2).

Load average: `getloadavg` or `sysctl vm.loadavg`. Uptime: `sysctl kern.boottime`.

**Do not** use `host_processors` / `host_get_host_priv_port` — those need root. `host_processor_info` is the user-safe path.

Free the info buffer with `vm_deallocate`. Need a previous snapshot for deltas.

### 1.1b Topology (once at start)

Documented: [Determining system capabilities](https://developer.apple.com/documentation/kernel/1387446-sysctlbyname/determining_system_capabilities).

| sysctl | Meaning |
| --- | --- |
| `hw.nperflevels` | Number of core *types*. Lower N = higher performance. |
| `hw.perflevel0.name` / `.physicalcpu` / `.logicalcpu` | Top tier (P on M1–M4, Super on M5) |
| `hw.perflevel1.*` | Next (E on two-level chips) |
| `hw.perflevel2.*` | Third tier when present (M5 family) |
| `machdep.cpu.brand_string` | e.g. `Apple M3 Pro` |
| `hw.model` | `Mac15,x` |
| `hw.optional.arm64` | detect AS vs Intel |
| `hw.memsize` | unified RAM |

AS: physical = logical (no HT). Intel HT shows extra logical CPUs. **Core order from `host_processor_info` is not guaranteed E-then-P** — map via `perflevel*` + IOReport channel names, not array index.

DVFS tables (AS, for weighting residencies) live in IORegistry `AppleARMIODevice` / `pmgr`:

- `voltage-states1-sram` — E
- `voltage-states5-sram` — P
- `voltage-states11-sram` — extra P cluster (Max/Ultra)
- `voltage-states9` — GPU
- M5+: `acc-clusters` (8-byte entries: byte0 = voltage-states index, byte1 = cluster type)

Freq scale: M1–M3 / A-series Hz ÷ 1e6. **M4+: kHz ÷ 1e3.** GPU tables stay Hz. (macmon `sources.rs`)

### 1.2 Frequency-scaled vs active ratios (Apple Silicon)

IOReport group **`CPU Stats`**, subgroup **`CPU Core Performance States`**.

Each channel is a core. Names (macmon `metrics.rs` comments, issue #47):

| Chip | Efficiency-ish | Performance-ish | Extra |
| --- | --- | --- | --- |
| M1–M4 | `ECPU*` | `PCPU*` | Ultra: `DIE_N_ECPU*` / `DIE_N_PCPU1_CPU0` |
| M5 base | `ECPU*` = E | `PCPU*` = **Super** | two tiers exposed |
| M5 Pro/Max | `MCPU*` = Performance (middle) | `PCPU*` = Super | `MCPU` is a real third design, not a renamed E |

States per channel: `DOWN` / `IDLE` / `OFF` then named MHz states. macmon:

```
active_ratio  = sum(active residencies) / sum(all)
avg_freq      = Σ (residency_i / active) * freq_i
scaled_ratio  = (avg_freq * active_ratio) / max_freq
```

Cluster MHz = mean of per-core MHz, floored at the DVFS table minimum so an idle cluster does not show 0.

DVFS tables: from SoC info (macmon `get_soc_info()` — sysctl / IORegistry chip ident + hardcoded or discovered freq arrays).

**This is the number that matches "how hard is the silicon working."** Show both: *active* (scheduler busy) and *scaled* (busy × clock). macmon toggles with `r`.

Filter the subscription — do **not** copy all IOReport channels. macmon `ioreport_channels_filter` keeps only:

- `Energy Model` × `GPU Energy` / `*CPU Energy` / `ANE*` / `DRAM*` / `GPU SRAM*`
- `CPU Stats` × `CPU Core Performance States`
- `GPU Stats` × `GPU Performance States`

### 1.3 Classic freq (Intel / fallback)

`sysctl hw.cpufrequency` is often the *nominal* max, not live. Intel live clocks are messy (some SMC, some not). Don't promise per-core MHz on Intel unless we verify on hardware.

## 2. GPU

### 2.1 Apple Silicon

Same IOReport world.

| Metric | Where | Notes |
| --- | --- | --- |
| Active / scaled / MHz | `GPU Stats` / `GPU Performance States` / channel **`GPUPH`** | Same residency math; skip `OFF`. Freq table from SoC (`gpu_freqs[1..]` in macmon — first entry is unused/off). |
| Power | `Energy Model` / **`GPU Energy`** | Watts via `IOReportSimpleGetIntegerValue` + unit label + `Δt` (`cfio_watts`). |
| Temp | SMC `Tg*` or HID `GPU MTR Temp Sensor*` | §8 |
| "VRAM" | **There is no VRAM** | Unified memory. Report system RAM or a GPU-wired estimate if we find one; don't invent a 8 GB bar. |

btop PR #1541 (`feat: Add Apple Silicon GPU support via IOReport API`) is the same recipe.

### 2.2 IOAccelerator — cheap GPU % (Intel **and** AS)

This is a **separate** path from IOReport. Use it as the portable utilization bar; use IOReport on AS when we also want clock + watts.

| Item | Detail |
| --- | --- |
| Match | `IOAccelerator` / `AGXAccelerator` |
| Property | `PerformanceStatistics` |
| Keys | `Device Utilization %` or `GPU Activity(%)`; AS also `Renderer Utilization %`, `Tiler Utilization %`; `In use system memory`, `Alloc system memory`; Intel dGPU sometimes `Temperature(C)`, `Core Clock(MHz)` |
| Cost | One registry walk. **Cache the `io_service_t`**, refresh properties. 500–1000 ms. |
| Accuracy | Instantaneous driver estimate. Tracks *active* more than frequency. On AS there is **no VRAM** — `Alloc system memory` can be huge for mapped weights. |
| Intel | iGPU + AMD/NVIDIA dGPU (class name contains `intel` / `amd` / `nvaccelerator`). dGPU switching is Intel-Mac-only. |

Used by Stats, Redline, btop-adjacent tools. `ioreg -r -c AGXAccelerator` is the spawned version — call IOKit instead.

AMD/NVIDIA *vendor* extras on Intel: out of v1.

**Do not** open a Metal device every tick to "query GPU". That *uses* the GPU. `objc2-metal` is irrelevant for a TUI.

**Per-process GPU:** no public API. `powermetrics --show-process-gpu` is sudo + flaky. Skip for v1.

## 3. ANE (Neural Engine)

`Energy Model` channels starting with **`ANE`** (`ANE`, `ANE0`, `ANE0_{die}` on Ultra). Watts only. No public utilization %. If watts ≈ 0, it's idle. Enough for a small gauge.

## 4. RAM and swap

### 4.1 Totals and breakdown

- Total: `sysctl hw.memsize`
- Pages: `host_statistics64(HOST_VM_INFO64)` → `vm_statistics64`

  Useful fields: `free_count`, `active_count`, `inactive_count`, `wire_count`, `speculative_count`, `compressor_page_count`, `purgeable_count`, `external_page_count`, `internal_page_count` (when available).

- Page size: `sysconf(_SC_PAGESIZE)` (not always 4096 on AS).

macmon "used" (Activity Monitor–like):

```
(active + inactive + wire + speculative + compressor - purgeable - external) * page_size
```

btop uses a similar `host_statistics64` path (`btop_collect.cpp` ~564, ~1249) and splits wired / compressed / cached for the stacked meter.

Swap: `sysctl VM_SWAPUSAGE` → `xsw_usage.{xsu_used, xsu_total}`.

**Unified memory:** CPU and GPU share this pool. A GPU-heavy workload raises *this* RAM, not a separate VRAM bar.

### 4.2 Memory pressure

Not the same as "% used". Headline should be used + pressure, never "free" (free is almost always tiny).

| API | What you get |
| --- | --- |
| `sysctlbyname("kern.memorystatus_vm_pressure_level")` | **1 = NORMAL, 2 = WARN, 4 = CRITICAL**. Hidden from `sysctl -a` (`CTLFLAG_MASKED`) but **readable by name, no root** ([psutil #2725](https://github.com/giampaolo/psutil/issues/2725)). **This is the one to use.** |
| `DISPATCH_SOURCE_TYPE_MEMORYPRESSURE` | event, don't poll |
| `sysctl vm.memory_pressure` | raw `vm_page_free_wanted` count, **not** 0–100% |
| `os_proc_available_memory()` | **this process's** jetsam leftover — **not** system RAM |

**Cost:** Mach calls, cheap. Don't walk every process to compute system RAM.

## 5. Storage

### 5.1 Capacity

`getmntinfo` / `statvfs` per mount. Filter to "real" volumes (APFS, skip `/dev`, `map`, vm, firmlinks duplicates). btop does this in `Mem::collect` osx path (~1368).

Show: size, used, available, % , mount point, volume name.

### 5.2 I/O rates

| Source | Keys | Notes |
| --- | --- | --- |
| `IOBlockStorageDriver` Statistics | `Bytes (Read/Write)`, `Operations (Read/Write)`, `Total Time (Read/Write)` ns | Classic. bottom, Stats, gopsutil. |
| `AppleAPFSVolume` (AS) | `Bytes read from block device`, `Bytes written to block device` | mactop prefers this; fall back to IOBlockStorageDriver. |
| `proc_pid_rusage` | `ri_diskio_bytesread/written` | per-process, extra cost |

**Delta / Δt** = B/s and IOPS. Cache last snapshot and the `io_service_t`.

Alternative: parse `iostat` — **don't**, process spawn.

### 5.3 NVMe SMART / NAND temp

| Path | Privilege | Notes |
| --- | --- | --- |
| SMC `TH0x` | user | cheap NAND proximity |
| HID `NAND CH% temp` | user | Stats HID list |
| `IONVMeSMARTInterface` (`SMARTReadData`) | often **works without sudo** on internal Apple NVMe | Kelvin in the log. Call **rarely** (30–60 s). COM-like, annoying in Rust. |
| `smartctl` / `diskutil` | spawn; may need root | avoid in the hot path |

**Cost:** IOKit walk is medium. Cache service ports; refresh properties only. Don't re-match all services every 200 ms. I/O rates at 1 s; SMART at 60 s.

## 6. Network

| API | Notes |
| --- | --- |
| `sysctl(CTL_NET, PF_ROUTE, 0, 0, NET_RT_IFLIST2, 0)` | `if_msghdr2` **64-bit** counters. **Preferred** (avoids 32-bit wrap). What `sysinfo` uses. |
| `getifaddrs()` | `ifa_data` as `if_data` / `if_data64`: `ifi_ibytes`, `ifi_obytes`, `ifi_ipackets`, `ifi_opackets`, `ifi_ierrors`. Fine; watch wrap on long uptimes. |

Delta / Δt = throughput. Skip `lo0` by default; keep `en0`, `en1`, `awdl0` optional, `utun*` optional (VPN).

btop: `src/osx/btop_collect.cpp` ~1441 (`getifaddr_wrapper`). Auto-scaling graphs.

Sequoia local-network TCC can prompt if you *connect* to LAN; **reading interface counters does not**.

**Cost:** cheap. 1 s interval. First sample is a baseline.

## 7. Fans

SMC keys (iSMC `src/fans.txt`, macmon `F??Ac`):

| Key | Meaning |
| --- | --- |
| `FNum` | fan count (sometimes) |
| `FnAc` | actual RPM |
| `FnMn` | min RPM |
| `FnMx` | max RPM |
| `FnSf` | safe RPM |
| `FnTg` | target RPM |

macmon: discover keys with `read_all_keys()` **once** at init, keep those matching `F` + `Ac`, then read only those. Max from sibling `Mx`.

Fanless Air / some Mini: keys missing or RPM = 0. Hide the panel; don't show a stuck 0.

**Writes** (`F0Md` manual, set `F0Tg`) need care and often root. **Out of scope.** Monitoring only.

Types: `flt `, `fpe2`, `ui8 `, `ui16`, `ui32`. macmon `smc_numeric_value`.

## 8. Temperature

Two backends. macmon rule of thumb (updated): **if SMC returned any CPU keys, use SMC; else HID**. They note it tracks **macOS version** (SMC float keys more available since Sonoma) more than chip gen.

### 8.1 SMC — discover, don't hardcode the universe

Scan once (`read_all_keys`), keep keys that:

- decode as a plausible °C (`0 < t ≤ 150`)
- match prefixes we care about

Apple Silicon prefixes used in the wild:

| Prefix / key | Meaning | Who uses it |
| --- | --- | --- |
| `Tp*` `Te*` `Ts*` | CPU / SoC clusters | macmon CPU set |
| `Tg*` | GPU | macmon GPU set |
| `TCMz` | CPU die hotspot (fast) | MacMonitor, TG Pro |
| `TCMb` | CPU die average | MacMonitor |
| `TRDX` / `Tg0e`… | GPU hotspot / cluster | MacMonitor |
| `TVm0` `Tm0B` `TMVR` | unified memory / VRM | MacMonitor |
| `TH0x` `TH0T` `TS0P` `TS1P` `T5SP` | NAND / SSD | iSMC, MacMonitor |
| `TB0T` `TB%T` | battery | classic |
| `TA*` `Ta*` | ambient / airflow | iSMC Apple list |

Intel classics (sysinfo, iSMC): `TC0P` proximity, `TC0D`/`TC0C` diode/core, `TG0P` GPU, `TB0T` battery, `TN0P` northbridge, `Tp0C` power supply.

iSMC `smc/sensors.go` + `src/temp.txt` is the most complete **named** map, including M5. Do not blindly read all 200 keys every second — pick a **display set** (hotspot, avg CPU, avg GPU, NAND, battery, ambient) after the one-time scan.

Units: historically `sp78` (signed 7.8). AS often `flt ` (IEEE LE float) or `sp1e`. A crate that only knows `sp78` will "see no temps" on AS.

### 8.2 HID sensor hub (AS, especially older OS)

```
IOHIDEventSystemClientCreate
IOHIDEventSystemClientSetMatching({ PrimaryUsagePage: 0xff00, PrimaryUsage: 0x0005 })
IOHIDEventSystemClientCopyServices
IOHIDServiceClientCopyProperty(service, "Product")
IOHIDServiceClientCopyEvent(service, kIOHIDEventTypeTemperature=15, …)
IOHIDEventGetFloatValue
```

Names (macmon + Stats `HIDSensorsList`):

| Product prefix | Meaning |
| --- | --- |
| `pACC MTR Temp Sensor*` | P-core |
| `eACC MTR Temp Sensor*` | E-core |
| `GPU MTR Temp Sensor*` | GPU |
| `SOC MTR Temp Sensor*` | SoC |
| `ANE MTR Temp Sensor*` | ANE |
| `ISP MTR Temp Sensor*` | ISP |
| `PMGR SOC Die Temp Sensor*` | PMU/die |
| `PMU tdie*` / `PMU tdev*` | PMU |
| `gas gauge battery` | Battery |
| `NAND CH% temp` | NAND |

Average the valid CPU/GPU ones for headlines; keep max as hotspot. HID can lag SMC hotspot by 2–4 s (MacMonitor note on `TCMz`).

**Cost:** HID copy-services each sample is not free. **Reuse the client** — macmon currently recreates it per `get_metrics`; we should not. Temps change slowly — **2 s** is enough even if CPU % is 1 s.

### 8.3 SMC keys by chip gen (Stats `Modules/Sensors/values.swift`)

Discover-at-init still wins. This table is for labeling keys we actually find.

**M1:** E `Tp09` `Tp0T`; P `Tp01` `Tp05` `Tp0D` `Tp0H` `Tp0L` `Tp0P` `Tp0X` `Tp0b`; GPU `Tg05` `Tg0D` `Tg0L` `Tg0T`; mem `Tm02` `Tm06` `Tm08` `Tm09`.

**M2:** E `Tp1h` `Tp1t` `Tp1p` `Tp1l`; P `Tp01` `Tp05` `Tp09` `Tp0D` `Tp0X` `Tp0b` `Tp0f` `Tp0j`; GPU `Tg0f` `Tg0j`.

**M3:** E `Te05` `Te0L` `Te0P` `Te0S`; P `Tf04` `Tf09` `Tf0A` `Tf0B` `Tf0D` `Tf0E` `Tf44` `Tf49` `Tf4A` `Tf4B` `Tf4D` `Tf4E`; GPU `Tf14` `Tf18` `Tf19` `Tf1A` `Tf24` `Tf28` `Tf29` `Tf2A`.

**M4:** E `Te05` `Te0S` `Te09` `Te0H`; P `Tp01` `Tp05` `Tp09` `Tp0D` `Tp0V` `Tp0Y` `Tp0b` `Tp0e`; GPU base `Tg0G` `Tg0H`; Pro/Max/Ultra `Tg1U` `Tg1k`; extra `Tg0K`–`Tg0k`; mem prox `Tm0p` `Tm1p` `Tm2p`. (Some M4 Pro keys missing on some SKUs — Stats #3271.)

**M5:** Super `Tp00` `Tp04` `Tp08` `Tp0C` `Tp0G` `Tp0K`; P `Tp0O` `Tp0R` `Tp0U` `Tp0X` `Tp0a` `Tp0d` `Tp0g` `Tp0j` `Tp0m` `Tp0p` `Tp0u` `Tp0y`; GPU `Tg0U` `Tg0X` `Tg0d` `Tg0g` `Tg0j` `Tg1Y` `Tg1c` `Tg1g`.

Intel classics: `TC0D` diode, `TC0E` virtual, `TC0F` filtered, `TC0P` proximity, `TCAD` package, `TC%c`/`TC%C` cores, `TCGC` iGPU, `TG0D`/`TG0P` dGPU, `TB1T` battery.

## 9. Power

### 9.1 Energy Model (AS, no sudo)

IOReport group **`Energy Model`**, same subscription as CPU/GPU states.

| Channel pattern | Metric |
| --- | --- |
| `*CPU Energy` / `DIE_N_CPU Energy` | CPU package W |
| `GPU Energy` | GPU W |
| `ANE*` | ANE W |
| `DRAM*` | DRAM W |
| `GPU SRAM*` | GPU SRAM W |

`cfio_watts`: integer energy / unit / Δt. First interval after subscribe is the first valid point.

`all_power = cpu + gpu + ane`. This is **silicon**, not wall. These are **Apple energy models**, not shunt resistors — they can under-count vs wall power. Still the same numbers powermetrics shows.

macpow (`k06a/macpow`) also parses extra Energy Model rails we can add later: `ISP*` (camera), `DISP*` / `DISPEXT*` (display), `AVE*` / `MSR*` (media), `PCIe Port*` / `apciec*`, `AMCC*` / `DCS*` / `FAB*` / `AFR*` (fabric), per-core `EACC*` / `PACC*` / `MCPU*`. Optional `backlight report` group: `UserBrightness`, `MilliNits` (absolute, no delta).

### 9.2 System / wall-adjacent

SMC **`PSTR`** — "System Total" (iSMC `src/power.txt`). macmon: `sys_power = max(PSTR, all_power)` when readable.

Intel has a zoo of `PC*`, `PG*`, `PDTR`, `PPBR` rail keys. Useful if we care about Intel power later.

### 9.3 Battery

IOKit Power Sources: `IOPSCopyPowerSourcesInfo` / `IOPSCopyPowerSourcesList` / `IOPSGetPowerSourceDescription`. Same as the menu bar: `%`, charging, time remaining, watts, cycle count, health, AC present.

SMC extras: `B0AC` current, `B0AV` voltage, `B0FC` full cap, `B0RM` remaining, `TB0T` temp. Desktop Macs: hide the panel.

**Cost:** IOPS is cheap. 2–5 s interval.

## 10. Thermal pressure

Not a temperature. It's the OS saying "I am throttling."

- `ProcessInfo.processInfo.thermalState` (`nominal / fair / serious / critical`) — Foundation, trivial via `objc2`. **Headline throttle flag.**
- `NSProcessInfoThermalStateDidChangeNotification` — event; don't poll.
- `notify_get_state(kOSThermalNotificationPressureLevelName)` — Nominal / Moderate / Heavy / Trapping / Sleeping (`thermald` → `notifyd`).
- `pmset -g therm` — spawn, skip.
- SMC thermal levels / `processor-hot` assertions (Netdata charts these).

Show a 4-level pill next to CPU temp. Cheap. High signal.

## 11. Processes

btop osx path (`btop_collect.cpp` ~1760+):

1. `sysctl KERN_PROC KERN_PROC_ALL` → `kinfo_proc[]` (pid, ppid, uid, nice, start time, stat).
2. New pids only: `proc_pidpath` for name, `sysctl KERN_PROCARGS2` for argv (cap ~1000 chars).
3. Every tick: `proc_pidinfo(pid, PROC_PIDTASKINFO)` → `pti_threadnum`, `pti_resident_size`, `pti_total_user + pti_total_system`.
4. CPU % = Δ task **Mach time** / (interval in Mach time × ncpus), **or** vs the sum of all processes' Mach deltas.

**Do not mix units.** `pti_total_user/system` are Mach time, not `host_processor_info` ticks. Mixing is why many Mac monitors disagree with Activity Monitor.

Memory column: `pti_resident_size` is RSS (overcounts shared). Activity Monitor's **"Memory"** is `proc_pid_rusage` → `ri_phys_footprint`. Prefer footprint if we want to match AM.

`proc_listpids(PROC_ALL_PIDS)` is an alternative to `KERN_PROC_ALL` (smaller). `PROC_PIDTBSDINFO` is the cheap name/ppid/uid flavor. `proc_pid_rusage` adds disk I/O + energy (`ri_billed_energy` / `ri_energy_nj`, version-dependent). Activity Monitor Energy Impact ≈ CPU core power, **not** GPU.

**Efficiency rules we must copy:**

- Cache name / path / user / argv. Those don't change.
- Don't call `getpwuid` every tick — cache uid → name.
- Skip pid < 1.
- When the user is only filtering/sorting, **don't re-collect** (btop `no_update`).
- Default sort CPU desc; tree is a view on the same snapshot (`ppid`).
- Interval ≥ 1 s. A 200 ms full walk on a 400-process Mac is how monitors get hated.
- Unreadable pids (SIP, other users): show name if we have it, zero cpu/mem; don't spam errors.

bottom uses **sysinfo** + a `ps -o pid=,pcpu=` **fallback**. The `ps` spawn is exactly what we want to avoid; `PROC_PIDTASKINFO` is enough.

Signals: `kill(pid, sig)` for TERM/KILL/INT. Confirm in a popup. No sudo needed for your own processes; others fail cleanly.

Per-process GPU / energy: not in libproc. `powermetrics --samplers tasks` is sudo + heavy. Skip for v1.

## 12. Display / brightness / other

Brightness, refresh rate, Clamshell: possible via private DisplayServices / IOKit. Low value for a resource monitor. Skip.

Clock: `chrono` / `time` in the title bar. Free.

## 13. SoC identity

macmon `SocInfo`: chip name (`Apple M3 Pro`), E/P/GPU core counts, memory GB, DVFS arrays, cluster labels. Sources: `sysctl` (`machdep.cpu.brand_string`, `hw.perflevel*`), IORegistry, maybe `system_profiler` **once at start** (slow — cache forever).

Show in the CPU box title: `Apple M3 Pro (6E+6P+18GPU 36GB)`.

## 14. Rust crates vs writing FFI

| Crate | Gives us | Gaps |
| --- | --- | --- |
| **`macmon` 0.8.2** | Production IOReport + SMC + HID + ram/swap + SoC. `Sampler::get_metrics(ms)` **blocks** for the interval. Documented to run on a worker thread. `cargo add macmon --no-default-features` for lib-only. | AS only. No processes, disks, net, battery IOPS, pressure. **MIT**. Viable AS sampler. |
| `sysinfo` **0.39.x** | CPU ticks, mem, disks, net, processes | No IOReport, weak AS sensors, RAM "used" may not match AM. Disable unused features. |
| `libproc` **0.14.x** | `proc_pidinfo` / listpids | Still need Mach-time CPU deltas |
| `mach2` 0.6 | Mach types | Some `host_statistics64` nits (macmon comments JohnTitor/mach2#34). `libc` is enough for host_*. |
| `core-foundation` 0.10 | CFDict/CFString | IOReport / IOKit glue |
| `io-kit-sys` 0.5 | Raw IOKit | Many projects bind themselves |
| `objc2` + `objc2-foundation` | `thermalState`, IOPS |  |
| `smc` 0.2.4 (2023) | Classic AppleSMC | Intel-era; often missing `flt ` / `sp1e`. Prefer copy macmon/macpow SMC (~100 LOC). |
| `objc2-metal` / `metal` | Don't |  |

**macpow** (`k06a/macpow`) is an app, not a polished lib — copy `ioreport.rs` / `smc.rs` as reference for extra Energy Model rails + M5 parsers.

**Prefer `dlopen`** for `libIOReport.dylib` and DisplayServices so Intel / older OS missing symbols don't abort the process. `#[link(name = "IOReport", kind = "dylib")]` is what macmon does (AS-only binary).

IOReport subscription is not `Send` unless wrapped; keep the sampler on one thread (`unsafe impl Send` like macmon).

**Recommendation:** either (a) depend on `macmon` as a library for AS silicon metrics and write Mach/IOKit ourselves for the rest, or (b) vendor the ~small IOReport/SMC/HID bindings (macmon is MIT). Do **not** re-derive Energy Model channel names from scratch — copy the filter list and the M5 naming notes.

IOReport / HID symbols are **private**. They have been stable enough for macmon, btop, Stats, asitop-alikes, but Apple can change names per OS. Feature-detect channels at runtime; never assume `GPUPH` exists.

## 15. Efficiency design (collector)

This is the difference between "pretty" and "usable."

### 15.1 Threading

```
[UI thread]  events + layout + draw     sleeps on channel / poll
[sample A]   IOReport window = interval  owns the sleep; emits silicon snapshot
[sample B]   Mach + disks + net + procs  can run at same or slower cadence
```

Never run IOReport on the UI thread (`get_metrics` sleeps).

macmon: one sampler thread, `mpsc`, UI draws on `Update`. Inputs on another thread with 250 ms poll. Copy this.

### 15.2 Cadence (starting point)

| Group | Interval | Why |
| --- | --- | --- |
| IOReport CPU/GPU/ANE/power | **1000 ms** | Needs a window; shorter = noisier + more wakeups |
| Mach CPU % | same tick | Tiny |
| RAM / swap / pressure | 1000 ms | Tiny |
| Net / disk I/O | 1000 ms | Counters |
| Processes | **1000–2000 ms** | The expensive walk |
| Temps / fans / battery / thermal | **2–10 s** | Slow hardware. Netdata samples SMC slowly — hammering AppleSMC can spam CoreAnalytics. |
| SoC info / SMC key list / mounts list | **once** (+ remount detect occasionally) |  |

Default UI interval 1000 ms (macmon) or 2000 ms (btop). Floor 250–500 ms. Ceiling several seconds. User adjustable, like both tools.

### 15.3 Don'ts

- Don't `Command::new("powermetrics")` / `iostat` / `vm_stat` / `ps`.
- Don't `IOReportCopyAllChannels` without a filter.
- Don't `read_all_keys()` every tick.
- Don't open/close AppleSMC every tick — one connection for the process life.
- Don't enumerate IOHID services from scratch if we can keep the client.
- Don't allocate a Metal device, don't use `ratatui-wgpu`, don't decode images.
- Don't walk processes to compute system CPU or RAM.
- Don't store unbounded history — ring buffer `width * 2` (braille) * panels.

### 15.4 Self-monitoring

Sample our own pid with the same `PROC_PIDTASKINFO` path. If we exceed ~2–3% CPU at 1 s interval, we have a bug (usually process walk or drawing every 16 ms).

## 16. Chip / model matrix

| Machine | CPU % | AS ratios / W | GPU | Fans | Temps | Notes |
| --- | --- | --- | --- | --- | --- | --- |
| M1–M4 (all SKUs) | Mach | IOReport | IOReport `GPUPH` | SMC or none | SMC and/or HID | Ultra = dual die channels |
| M5 | Mach | IOReport; **MCPU vs PCPU vs ECPU** | same | same | same | don't label MCPU as E |
| Fanless Air |  |  |  | hide |  |  |
| Intel + iGPU | Mach | limited / rails | IOAccelerator | SMC | SMC `TC*`/`TG*` | no ANE |
| Intel + dGPU |  |  | vendor |  |  | later |
| Desktop (Studio/Pro/Mini) |  |  |  | often 1–2 |  | hide battery |

## 17. Metric catalog (what we *can* show)

Priority: **P0** = empty product without it, **P1** = makes it a Mac tool, **P2** = polish.

| Metric | Pri | API | Sudo | Notes |
| --- | --- | --- | --- | --- |
| CPU % total + per core | P0 | Mach load info | no |  |
| Load avg / uptime / hostname | P0 | sysctl | no |  |
| RAM used/total + wired/compressed/cache | P0 | `HOST_VM_INFO64` | no |  |
| Swap | P0 | `VM_SWAPUSAGE` | no |  |
| Disk used/total | P0 | statvfs | no |  |
| Disk R/W B/s | P0 | IOKit IOMedia | no |  |
| Net R/W B/s | P0 | getifaddrs | no |  |
| Process table + tree + signals | P0 | kinfo + libproc | no |  |
| Chip name + core counts | P1 | sysctl / SoC | no |  |
| E/P/(S) active + scaled + MHz | P1 | IOReport CPU Stats | no | AS |
| GPU % (portable) | P1 | IOAccelerator `PerformanceStatistics` | no | Intel + AS |
| GPU active + scaled + MHz | P1 | IOReport GPU Stats | no | AS |
| CPU/GPU/ANE/DRAM W | P1 | IOReport Energy | no | AS |
| Package / `PSTR` W | P1 | SMC | no |  |
| CPU/GPU temp | P1 | SMC or HID | no |  |
| Fans RPM | P1 | SMC `FnAc` | no | hide if 0 |
| Thermal state | P1 | NSProcessInfo | no |  |
| Memory pressure | P1 | `kern.memorystatus_vm_pressure_level` | no | 1/2/4 |
| Battery % / W / health | P2 | IOPS + SMC | no | hide on desktop |
| NAND / SSD temp | P2 | SMC | no |  |
| Ambient temp | P2 | SMC `TA*` | no |  |
| Per-core temp | P2 | many SMC keys | no | noisy |
| NVMe SMART | P2 | IOKit | maybe |  |
| Per-process GPU | P3 | private / powermetrics | often yes | skip |
| Fan control | — | SMC write | often yes | **no** |

## 18. Worked sample (what one tick does on AS)

1. Wake: `IOReportCreateSamples` → delta vs previous → parse filtered channels → ratios + watts.
2. `host_processor_info` → per-core %.
3. `host_statistics64` + `VM_SWAPUSAGE` → RAM.
4. Every 1–2 s: `getifaddrs`, IOKit disk counters, `KERN_PROC` + `proc_pidinfo`.
5. Every 2 s: SMC floats for cached temp/fan keys; IOPS battery; thermal state.
6. Push into ring buffers. Send snapshot to UI. Sleep the remainder of the interval (IOReport path already slept).

No child processes. No GPU. No full SMC dump.

# Overview

Last updated: 2026-08-24.

## What we are aiming at

A **Ratatui** TUI that feels as considered as **btop++**: braille/"dot" graphs, truecolor gradients, rounded boxes, mouse + keyboard, layouts that collapse cleanly. Metric-wise it is **macOS first** — Apple Silicon deeply (E/P/S cores, GPU residency, ANE watts, fans, die temps), Intel Macs as a second-class but honest path. It has to stay cheap: the monitor must not become the hottest process on the machine.

This phase is **research only**. The crate is still `Hello, world!`. These docs exist so the next session can start from facts, not vibes.

## Non-goals (for now)

- Shipping a TUI
- Fan *control* (write to SMC)
- Spawning `powermetrics` / `iostat` / `ps` on every tick
- Using the GPU to render the TUI (`ratatui-wgpu` exists; we do not want it)
- Cross-platform Linux/Windows support in v1

## Hard constraints we already know

| Constraint | Why |
| --- | --- |
| No sudo for the default path | `powermetrics` is accurate and expensive and needs root. macmon, btop (read path), Stats.app, and iSMC all prove a sudoless path exists. |
| Do not spawn helper processes per sample | Process spawn + parse is the #1 way these tools become heavy. |
| Sample windows are the sleep | IOReport frequencies/power are *deltas*. The collector thread *is* the interval. |
| UTF-8 + braille font + truecolor | Same prerequisite as btop. Need a TTY/256-color fallback. Apple Terminal.app is weak at 24-bit — treat iTerm / Ghostty / Kitty / Wez as the real target. |
| Immediate-mode Ratatui | We own the event loop. Draw only on input or a new sample. |

## Aesthetic north star

btop's "dot" look is not a Chart widget. It is a **2×4 braille cell** that packs **two consecutive samples** into one column, colored by a usage gradient. The exact 5×5 symbol table lives in both:

- btop `src/btop_draw.cpp` (`graph_symbols["braille_up"]`)
- `tui-bar-graph` `BRAILLE_PATTERNS` (identical matrix)

```
 " ", "⢀", "⢠", "⢰", "⢸",
 "⡀", "⣀", "⣠", "⣰", "⣸",
 "⡄", "⣄", "⣤", "⣴", "⣼",
 "⡆", "⣆", "⣦", "⣶", "⣾",
 "⡇", "⣇", "⣧", "⣷", "⣿"
```

Index = `left_level * 5 + right_level`, each level `0..=4`. That is the look.

Rounded box drawing (`╭╮╰╯`) + truecolor + per-box titles is the rest of the skin. Ratatui gives us `BorderType::Rounded` and `Color::Rgb`.

## Metric north star (v1 candidate, not locked)

**Always on, cheap**

- CPU overall + per-core (Mach `host_processor_info`)
- Load average, uptime, hostname / chip name
- RAM used / wired / compressed / cached / swap / pressure
- Disk capacity + read/write rates
- Network per-interface bytes
- Process table (pid, name, cpu, mem, threads, user, tree)

**Apple Silicon, still no sudo, a bit more expensive**

- E / P / (M5) S-core *active* vs *frequency-scaled* ratios
- GPU active / scaled / MHz
- CPU / GPU / ANE / DRAM / package watts (IOReport Energy Model)
- Fans RPM
- CPU / GPU / NAND / battery temps
- Thermal pressure

**Nice, later**

- Battery charge, watts, cycles, time remaining
- Per-process GPU / energy (sparse, often private)
- NVMe SMART health
- Discrete GPU on Intel (IOAccelerator)

See [02-macos-metrics.md](02-macos-metrics.md) for how each of those is actually obtained.

## Recommended crate versions (as of this research)

| Crate | Version seen | Role |
| --- | --- | --- |
| `ratatui` | **0.30.2** | UI. Workspace split: `ratatui-core`, `ratatui-widgets`, `ratatui-crossterm`. |
| `crossterm` | 0.29 (via ratatui) | Events, raw mode, mouse. |
| `tui-bar-graph` | part of `tui-widgets` | Braille / octant / quadrant history graphs + colorgrad. |
| `colorgrad` | used by tui-bar-graph | Usage → color ramps (turbo, magma, viridis…). |
| `macmon` | **0.8.2** | Can be used as a *library* for AS power/freq/temp. Apple Silicon only. |
| `sysinfo` | used by bottom | Cheap CPU/mem/disk/net/process. Weak on SMC / IOReport. |
| `core-foundation` | 0.10 (macmon) | CF types for IOReport / IOKit. |
| `libc` / `mach2` / `libproc` | — | Mach + process APIs. |
| `tachyonfx` | optional | Transitions only. Easy to waste CPU. Default off. |

## How to use these docs when we start building

1. Pick a panel from [04-synthesis.md](04-synthesis.md).
2. Confirm the widget in [01-ratatui.md](01-ratatui.md).
3. Confirm the collector in [02-macos-metrics.md](02-macos-metrics.md).
4. Check how btop / macmon / bottom did the same thing in [03-prior-art.md](03-prior-art.md).
5. If we discover a new SMC key or a Ratatui API change, patch the matching file. Do not start a sixth scratchpad.

## Sources (primary)

- Ratatui book: <https://ratatui.rs>
- Ratatui widgets: <https://ratatui.rs/concepts/widgets/>
- Ratatui layout: <https://ratatui.rs/concepts/layout/>
- Ratatui rendering: <https://ratatui.rs/concepts/rendering/>
- Ratatui 0.30 architecture: cloned `ARCHITECTURE.md`
- btop: <https://github.com/aristocratos/btop> — especially `src/btop_draw.cpp`, `src/osx/`
- macmon: <https://github.com/vladkens/macmon> — `src_lib/{sources,metrics}.rs`, `src_app/tui.rs`
- bottom: <https://github.com/ClementTsang/bottom>
- Stats.app: <https://github.com/exelban/stats>
- iSMC: <https://github.com/dkorunic/iSMC>
- tui-bar-graph: <https://github.com/ratatui/tui-widgets/tree/main/tui-bar-graph>
- MacMonitor sensor map: <https://github.com/ryyansafar/MacMonitor/blob/main/SENSORS.md>
- macpow (broader Energy Model): <https://github.com/k06a/macpow>
- Stats SMC/HID atlas: `Modules/Sensors/values.swift`
- Memory pressure sysctl writeup: <https://github.com/giampaolo/psutil/issues/2725>

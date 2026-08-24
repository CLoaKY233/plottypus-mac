# Synthesis — how we can go at it

Last updated: 2026-08-24. Still research: no code landmines, just the shape that falls out of the other three docs.

## 1. One-sentence product

**btop's braille dashboard, macmon's Apple Silicon numbers, bottom's process/disk/net table — in Ratatui, sudoless, cheap.**

## 2. Aesthetic recipe (locked enough to design against)

| Decision | Choice | Why |
| --- | --- | --- |
| Graph glyph | **Braille** default; Block and TTY (░▒█) as fallbacks; Octant optional | Identical to btop; `tui-bar-graph` already has the table |
| Graph color | 101-stop LUT (start/mid/end). **By value** on 1-row, **by row** on tall | btop. Encode braille from `U+2800+bits`, do not copy their 25-string table |
| Sync output | DEC 2026 / `terminal_sync` if crossterm exposes it | kills flicker |
| Boxes | `BorderType::Rounded` + **`merge_borders`** so panes share one line | btop grid, no double seams |
| Truecolor | on, with 256 and 16-color fallback | btop + `termprofile`. **Terminal.app is a bad truecolor host** — document Ghostty/iTerm/Kitty/Wez |
| Chrome | title = name + live numbers; bottom title = hotkeys | macmon + btop |
| Motion | **none** by default | tachyonfx is a tax |
| Fonts | document: Braille + box drawing, like btop README |  |

Do **not** use Ratatui `Chart` for the main histories. Axes waste cells and `GraphType::Line` + `Marker::Braille` is a scatterline, not filled columns.

Implementation paths when we code, in order of laziness:

1. `tui-bar-graph` (`BarStyle::Braille`, `ColorMode::Solid` or `VerticalGradient`).
2. If we need btop's exact two-sample scroll (tui-bar-graph is one bar per datum, which at width W shows W samples; btop shows **2W**), write a ~80-line `BrailleHistory` widget using the same 5×5 table. Prefer this eventually — twice the time resolution is the whole point of braille.

## 3. Responsive layout recipe

Compute a `LayoutPlan` from `(cols, rows, flags, chip)` **before** rendering. Widgets do not guess.

### 3.1 Boxes

| Id | Min size (inside border) | Downgrade path |
| --- | --- | --- |
| `cpu` | 40×8 | tall braille → 2-row braille → sparkline → LineGauge + % |
| `cores` (inside cpu or own) | 12×(1+n/cols) | wide meters → 1-row sparks → hide |
| `gpu` | 24×6 | same as cpu; hide if no IOReport GPU |
| `power` | 24×5 | 3× LineGauge (CPU/GPU/ANE W) → one "SoC W" |
| `mem` | 24×6 | stacked bars + swap → one Gauge |
| `disks` | 28×5 | table+io sparks → table → hide |
| `net` | 28×5 | dual braille → sparkline → hide |
| `proc` | 40×8 | full table → no per-row graph → hide |
| `sensors` | 20×4 | fans+temps → one line in cpu title → hide |

### 3.2 Breakpoints (starting guess)

| Size | Plan |
| --- | --- |
| ≥ 160×40 | cpu+cores \| mem+disks ; gpu+power \| net ; proc full width |
| 120×32 | cpu (no cores) + gpu ; mem + net ; proc |
| 80×24 | cpu spark + mem gauge ; proc |
| < 80 or < 20 rows | single box + `Tab` to cycle, or a "resize me" Paragraph |

Surfaces (see rewritten PRD — not five presets):

1. **Work** — wide default: compact health header + process table
2. **Glance** — small pane / `g`: silicon instrument cluster

Disk, net, sensors are revealed later, not first paint. `e` expand, `f` freeze stay.

Min sizes to start from (btop’s, then tune): CPU 60×8, MEM 36×10, NET 36×6, PROC 44×16, GPU 41×8. Below that, **hide in priority** (net → disks → cores → gpu → mem → proc), never clip mid-glyph.

`h` / `1`–`5` switch presets. Boxes can still be toggled.

`cpu_bottom` / `proc_left` are cheap flags; don't need them on day one.

Per-core columns: copy btop's three widths (see prior-art). On M-series, **group E then P (then S)** with a 1-cell gutter, don't interleave.

## 4. Metric layers

Build collectors as **independent sources** with their own period. UI just reads the last snapshot.

```
Layer A  1.0s   Mach CPU %, IOReport (AS), RAM/swap
Layer B  1.0s   net, disk IO
Layer C  1–2s   processes
Layer D  2.0s   SMC/HID temps, fans, battery, thermal, pressure
Layer E  once   SoC info, SMC key list, mount list, theme
```

Intel: Layer A loses IOReport; GPU becomes IOAccelerator or hidden.

### 4.1 What the first *useful* UI needs

Not everything in the catalog.

**Must:** CPU history + %, RAM gauge + breakdown, process table, (AS) GPU % + package watts.

**Should:** per-core, E/P split, ANE W, fans, CPU/GPU temp, disks, net, thermal pill, memory pressure.

**Later:** battery detail, sensor browser, SMART, Intel dGPU, per-process GPU.

## 5. Efficiency recipe (non-negotiable)

1. **Two or three threads.** UI never calls `Sampler::get_metrics`.
2. **No child processes** in the sample path.
3. **One SMC connection**, key list at init.
4. **Filtered IOReport subscription** (macmon's five channel rules).
5. **Process cache** for name/path/user; Δ cpu only.
6. **Draw on event** (key, resize, new snapshot), not on a 60 fps timer.
7. **Ring buffers** capped to `2 * width` samples.
8. **Hide missing hardware** so we don't poll empty fans.
9. Default interval **1000 ms** (AS power is bursty; btop's 2000 feels sleepy). Floor **500 ms**.
10. SMC / HID at **2–10 s** (Netdata: SMC discovery can spam CoreAnalytics if you hammer it).
11. Self-stat our pid; treat >3% CPU at 1 s as a regression.
12. Graphs are O(width) appends into a ring buffer, not a full rebuild of history strings.

GPU usage of *this app* should be ~0. We never create a Metal device.

## 6. Suggested crate graph (when we build)

```
plottypus
├── ratatui 0.30 + crossterm          UI
├── tui-bar-graph + colorgrad         histories (or our BrailleHistory)
├── tui-popup / tui-input             overlays, filter
├── tui-tree-widget                   optional, process tree
│
├── macmon (lib, optional feature)    AS silicon — OR vendored MIT bindings
├── libc + mach2 + libproc            Mach / processes
├── core-foundation                   IOReport / IOKit glue
├── objc2-foundation                  thermalState, maybe IOPS
└── sysinfo                           only if we want a lazy Intel/disk fallback
```

Start **without** sysinfo if we can — its RAM definition and AS sensors will fight macmon's. bottom shows it's fine as a base, but we already know the native calls.

Feature flags later: `apple-silicon` (default on aarch64), `intel-smc`.

## 7. App architecture (TEA + panels)

```
Model
  config (theme, preset, interval, graph_symbol, ratio_mode)
  layout: LayoutPlan
  snap: Snapshot            // last collector output
  hist: Histories           // ring buffers
  proc: ProcViewState       // TableState, filter, sort, tree, follow
  ui:   focus, modal

Message
  Key / Mouse / Resize
  Tick(Snapshot)
  ChangePreset / ToggleBox / ChangeInterval / ChangeSymbol
  ProcSort / ProcFilter / ProcSignal / ProcFollow

update → maybe another Message
view   → LayoutPlan.split → panel widgets
```

Collector(s) live outside TEA and only inject `Tick`.

## 8. Interaction map (first cut)

| Key | Action |
| --- | --- |
| `q` / `Esc` | quit (Esc closes modal first) |
| `?` / `h` | help overlay |
| `1`–`5` | presets |
| `c` | cycle CPU box density / color theme (pick one; don't overload) |
| `v` | graph style braille ↔ block |
| `r` | AS ratio scaled ↔ active |
| `t` | process tree |
| `/` | filter processes |
| `Enter` | process detail |
| `k` | kill (confirm) |
| `e` | expand focused box to fullscreen (bottom) |
| `f` | freeze snapshot |
| `+/-` or `]`/`[` | interval |
| `m` | toggle mem/disks |
| `n` | toggle net |
| `g` | toggle gpu/power |
| `p` | toggle proc |
| arrows / `j` `k` | move in focused table |
| `Tab` | focus next box |
| mouse | click titles/toggles, wheel in proc |

Visible footer always. Mouse mappings rebuilt each frame from `Rect`s.

## 9. Theme

Start with **one** built-in dark theme (near btop default: dark bg, cyan cpu, green mem, magenta gpu, blue net, yellow proc accent) plus a high-contrast / 16-color variant.

Roles, not hex-in-widgets:

`bg, fg, dim, title, border, border_focus, cpu, mem, gpu, net, proc, ok, warn, crit, gradient_cpu[0..1], gradient_temp, gradient_net`.

Files later (btop-compatible or TOML). Not a v1 blocker.

## 10. Risks we already know

| Risk | Mitigation |
| --- | --- |
| IOReport / HID are private | Feature-detect channels; degrade to Mach-only |
| SMC key soup per chip | Discover + prefix, don't hardcode M3-only lists; keep iSMC map as reference |
| M5 core naming | Use macmon's E/P/M rules; label from SoC, not "E-core" blindly |
| `tui-bar-graph` is 1 sample/cell | Accept at first or write BrailleHistory |
| Process walk cost | ≥1 s, cache, no `ps` |
| Terminal without braille | `block` / `tty` auto if we detect replacement chars (hard) or a config flag (easy) |
| Depending on macmon | MIT, AS-only, `get_metrics` blocks — wrap it. If they break M6 names, we feel it. Vendoring bindings = more control. |
| Scope creep (Bluetooth, SMART, fan control) | Catalog is in 02; v1 is the P0+P1 table |

## 11. Build order (for whenever we leave research)

This is a suggested order, **not started**:

1. Ratatui skeleton: alt screen, TEA, footer, rounded empty boxes, resize-based `LayoutPlan`.
2. Mach CPU % + RAM + a braille history on fake-then-real data. Prove the look.
3. Process table (cache, sort, filter). Prove we stay cheap.
4. AS sampler (macmon lib or vendored): GPU, watts, E/P, temps, fans. Silicon preset.
5. Disks + net.
6. Themes, mouse, presets, help, kill confirm.
7. Intel SMC / IOAccelerator pass.
8. Battery, pressure, sensor browser.

Do not start 4 before 2 looks like btop. The whole point of this repo's taste constraint is the graph.

## 12. Open questions (answer when we implement, then write the answer back here)

- Depend on `macmon` (`--no-default-features`) or vendor IOReport bindings? `dlopen` if we want one binary on Intel + AS.
- Show GPU % from IOAccelerator (portable) *and* IOReport clocks/watts (AS), or pick one bar?
- Process memory: RSS vs `ri_phys_footprint` (Activity Monitor)?
- Is `tui-bar-graph` close enough, or do we want 2 samples/cell from day one?
- Process tree via `tui-tree-widget` or a custom Table indent?
- Config file path (`~/.config/plottypus/` vs XDG)?
- Do we care about Intel Macs in v1 or just "doesn't crash"?
- Name/branding in the title bar — keep plottypus, pick a wordmark?

## 13. Pointers

- Widgets and layout: [01-ratatui.md](01-ratatui.md)
- Every metric API: [02-macos-metrics.md](02-macos-metrics.md)
- Who already did it: [03-prior-art.md](03-prior-art.md)
- Goal and constraints: [00-overview.md](00-overview.md)

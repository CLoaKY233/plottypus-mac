# Roadmap

Companion to [docs/research/05-product-design.md](docs/research/05-product-design.md) (PRD) and
[docs/research/06-product-critique.md](docs/research/06-product-critique.md) (locked decisions).
Research files are reference; this file is what we build next, in order.

## Phase 0 — reported issues (fixed in this change)

Found in hands-on review against the running binary; each has a regression test or a live-machine check:

1. **Panic below 60 columns** — `proc_w.clamp(24, width - 36)` inverts once `width < 60`, but the
   Work gate only required 40 (`crates/plottypus-ui/src/layout.rs` vs
   `plottypus-core/src/surface.rs`). Any terminal between 40–59 cols panicked on first draw.
   Fix: gates agree at 60, clamp can no longer invert, size-sweep render test added.
2. **GPU temperature never fills on real hardware** — CPU had a package/hotspot fallback in
   `Sampler::tick`; GPU had none, so machines without a dedicated GPU zone sensor show nothing.
   Fix: same fallback chain (zone → readings scan), verified by ticking the real sampler; if the
   hardware exposes no die temp we still show nothing rather than a fake number.
3. **Process names show version folders** — names preferred `proc_pidpath` basename, so apps
   installed under version directories displayed as e.g. "2.1.241". Fix: prefer kernel `comm`
   (what `ps -o comm=` shows), fall back to path basename when comm is empty.
4. **Stale help text after click-to-open changed** — settings pane said "click a process twice",
   detail popup said "click again already selected"; both described the old two-click behavior.
   Fix: text matches single-click behavior.
5. **Bare fan numbers in the sensors headline** — expanded panel prints `1850 rpm`, headline
   printed bare `0`. Fix: unit everywhere; width math updated to match.

## Phase 1 — Truth pass (the reason we exist)

- [x] UI reads the frequency-weighted ratio everywhere the headline appears, with a dim `busy`
      figure beside it when the two diverge. **Open:** the collector still reports
      scaled = active; real residency sampling via IOReport is its own project.
- [ ] Cluster + core frequency (no sysctl source exists on AS; needs IOReport residency work).
- [ ] Total SoC watts (IOReport `PSTR`) as the power headline in Glance and Work health band.
- [x] Graph stain ramp: idle stays dim; load stains accent→gold→red as thermal leaves nominal.
- [x] ~~Peak pip~~ shipped, then removed by product call: unexplainable noise over data.
- [ ] Throttle marks where frequency slumps while watts flatten (blocked on a freq source).

## Phase 2 — Process pass (finish the job)

- [x] Detail popup actions: TERM / KILL / INT clickable inside the popup.
- [x] Pin `selected_pid == detail_pid`: arming a kill while the popup shows pid A targets A.
- [x] Detail enrichment: user, full command path, state, started-ago.
- [x] Per-pid CPU sparkline from the collector's cached deltas.
- [x] Tree view from `ppid`.
- [x] Search matches pids too; highlight matches.

## Phase 3.5 — Cockpit / expanded rewrite

- [x] Graph/spark visibility follows panel inner height (80×24 Work SENS/MEM keep a spark).
- [x] Shaped downsample (last-value recent, peak older) on braille and sparks.
- [x] 250 ms cheap collectors; cached GPU/disk ports; reused HID client; 1 s procs; 2 s sensors.
- [x] Expanded packer (`grid::pack`) + graph-first cells. No empty stat cathedrals.
- [x] Cluster load histories + zone °C graphs on CPU and SENS (never labeled per-core).
- [x] Related-family hops (Tab / ← → / click `→`).

## Phase 3 — Layout pass

- [x] Expanded views rebuilt as macmon-style grids: every metric and graph lives in its own
      bordered, titled cell; layouts are symmetric and fill the body with no dead bands.
- [x] Terminal owns the background (no painted black); net pane back on first paint.
- [x] Work rebalance: process column default 55% (clamp 35–72).
- [ ] Auto-promote net/disk during sustained IO, demote after.
- [x] Degradation ladder for shrinking widths (Full → Tight → Minimal, defined hide order).
- [x] Quiet contextual footer: base `? / q`; verbs appear per focus/selection only; paused chip.
- [x] Slimmer axis gutters: percent/celsius tick columns removed entirely; bits keeps its gutter.
- [ ] Revisit the Work/Glance column gate (~100 cols in PRD vs 60 today) with real small-pane use.

## Phase 4 — Feel pass

- [x] Worker-thread sampler: FFI never blocks draw/poll (mpsc command/snapshot channels).
- [x] DEC 2026 synchronized updates to kill frame flicker.
- [x] Persist settings after first change (interval, pane toggles, sort, surface, proc ratio);
      zero config on first run preserved.
- [ ] Battery + power source (IOPS APIs) — core persona works on a laptop.
- [x] Status messages expire instead of living in the footer forever.
- [x] Adaptive background (terminal decides; no painted bg). Colorblind ramp audit still open.
- [x] Measure self CPU: ~1% at 1 s on the reference machine (gate <2%).

## Phase 5 — later

`--json` headless pipe · remembered layouts/presets · light theme · per-process GPU if Apple ever
ships a cheap API. Non-goals stay non-goals for v1: Intel-first layouts, Prometheus exporter,
theme marketplace, fan control.

## Success gates (unchanged from PRD)

Insight < 10s · filter→kill < 5s after insight · a week of use without opening `?` ·
self CPU < 2% at 1s · 80×20 is a complete Glance.

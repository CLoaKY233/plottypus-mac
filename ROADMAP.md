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

- [ ] Headline CPU % = **scaled** ratio; Mach `busy NN%` dim beside it. Locked decision #4 — today
      every widget renders `.active` and `scaled` is collected but never drawn.
- [ ] Cluster + core frequency: `Cluster::freq_mhz` is hardcoded 0 (`sampler.rs`), `CoreSample`
      has no freq field.
- [ ] Total SoC watts (IOReport `PSTR`) as the power headline in Glance and Work health band.
- [ ] Graph stain ramp: idle stays empty/dim; load stains mint→gold→red as thermal leaves nominal
      (`Theme::graph` exists but the render path ignores cell intensity).
- [ ] Peak pip on the max-in-view column.
- [ ] Throttle marks where frequency slumps while watts flatten.

## Phase 2 — Process pass (finish the job)

- [ ] Detail popup actions: TERM / KILL / INT clickable inside the popup (`Signal` already exists;
   UI only ever sends TERM).
- [ ] Pin `selected_pid == detail_pid`: arming a kill while the popup shows pid A must target A.
- [ ] Detail enrichment from data we already touch: user (`kinfo_proc` uid), full command path,
   state, started-ago.
- [ ] Per-pid CPU sparkline (collector cache already keeps per-pid deltas).
- [ ] Tree view from `ppid`.
- [ ] Search matches pids too; highlight matches.

## Phase 3 — Layout pass

- [ ] Work rebalance: one health band on top, process table ≥60% of width (critique §7: PROC is
      the product, not a side pane at a 48% clamp).
- [ ] Net/disk demoted off first paint; auto-promote during sustained IO, demote after.
- [ ] Degradation ladder for shrinking widths (defined hide order; never squeeze into garbage).
- [ ] Quiet contextual footer: base `? / q`; verbs appear per focus/selection only.
- [ ] Slimmer axis gutters; drop ticks that duplicate title numbers.
- [ ] Revisit the Work/Glance column gate (~100 cols in PRD vs 60 today) with real small-pane use.

## Phase 4 — Feel pass

- [ ] Worker-thread sampler: SMC/HID/IOReport FFI must never block draw/poll (macmon's lesson,
      not yet applied).
- [ ] DEC 2026 synchronized updates to kill frame flicker.
- [ ] Persist settings after first change (interval, pane toggles, surface, proc ratio); zero
      config on first run preserved.
- [ ] Battery + power source (IOPS APIs) — core persona works on a laptop.
- [ ] Status messages expire instead of living in the footer forever.
- [ ] Adaptive background (stop forcing black), colorblind-safe accent ramp.
- [ ] Measure self CPU against the <2% @ 1s success gate.

## Phase 5 — later

`--json` headless pipe · remembered layouts/presets · light theme · per-process GPU if Apple ever
ships a cheap API. Non-goals stay non-goals for v1: Intel-first layouts, Prometheus exporter,
theme marketplace, fan control.

## Success gates (unchanged from PRD)

Insight < 10s · filter→kill < 5s after insight · a week of use without opening `?` ·
self CPU < 2% at 1s · 80×20 is a complete Glance.

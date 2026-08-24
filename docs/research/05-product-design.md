# PRD — plottypus

Last updated: 2026-08-24. Rewritten after [06-product-critique.md](06-product-critique.md). Visuals: [../mockups/dashboard.html](../mockups/dashboard.html).

## 1. Product

**The Mac monitor that stays quiet until the machine isn’t — then tells you who, how hard, and lets you act.**

Not “btop + macmon.” A daily-driver TUI for Apple Silicon people who already live in a terminal. First paint answers one question. Next key finishes the job.

| | |
| --- | --- |
| **User** | Apple Silicon developer / power user. Wide daily driver *or* a small multiplexer pane. |
| **Job** | “Why is this Mac hot/slow/loud, and what do I do?” |
| **Moment** | Fans, beachball, LLM/Xcode load, “is it throttling or just busy?” |
| **Done when** | They know *who*, *how hard* (not just how scheduled), *how hot*, and they have filtered/killed or decided to wait. |

### Why this, not btop

btop is prettier than Activity Monitor and **wrong on AS** in the way that matters: 100% busy at 600 MHz looks like 100% busy at 4 GHz. No watts, no ANE, no honest GPU clock. We win on **truth next to beauty**, and on a **calmer default**.

### Why this, not macmon

macmon already has the physics. It cannot finish the job. No processes, no kill, no disks/net when you need them, sparkline-only. We win on **action** and on **craft** (braille, quiet chrome) while keeping a **small-pane glance** that is automatic, not a hidden preset.

### Non-goals (v1)

Intel as a first-class layout. Prometheus. Theme marketplace. Fan control. Per-process GPU. Sensor laboratory. Five named presets. Matching Activity Monitor’s 0–100 pressure graph. Wall-outlet watts.

### Success

- Insight in < 10s (who + how hard + thermal).
- Filter → kill in < 5s after that.
- A week of use without opening `?`.
- Self CPU < 2% at 1s. Ours is not in the top processes unless the machine is idle.
- Small pane (80×20) is a complete Glance, not a “widen me.”

## 2. Two surfaces

Not five presets. Two products that share chrome.

| | **Glance** | **Work** |
| --- | --- | --- |
| Job | Health of the silicon | Act on processes |
| When | Small window, or `g` | Wide default, or `w` |
| Hero | CPU + GPU/watts | Process table |
| Feels like | a calm instrument cluster | htop that understands a Mac |

**Rule:** terminal below ~100×24 **always** opens Glance. Wide terminals open Work with a compact health header (CPU graph + one GPU/mem line), not the kitchen dashboard.

`Tab` cycles focus inside a surface. `g` / `w` switch surfaces. That is the whole information architecture for v1.

Disks and net are **not** on first paint. They appear in Glance when they are the story (sustained IO), or behind one extra toggle later. They are not a junk drawer under memory.

## 3. Layout

### 3.1 Work (wide default)

Health header is one band. The rest is PROC.

```
╭─ cpu  18%  8.2W ─────────────────────────────────────────── 42° · nominal ─╮
│ ⣿⣷⣧⣇⣀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀ │ E ▂▂▃▂▂▁   P █▆▄▂██   busy 41%     │
│ ⣿⣿⣿⣿⣷⣦⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀ │ 3.2 GHz               gpu 12% 1.1W │
├─ mem  22 / 36G  ● ─────────────────────────────────────────────────────────┤
│                                                                            │
│  PID   name             cpu    mem     threads                             │
│  312   WindowServer     5.2    312M    18                                  │
│  904   Xcode            48.1   4.2G    62                                  │
│  …                                                                         │
╰────────────────────────────────────────────────────────────────────────────╯
 ? help   / filter   q quit
```

Title: three tokens. Dim subline / right side holds temp, thermal, busy %, GPU. MEM is a **single line**, not a composition essay. Process table gets the pixels.

### 3.2 Glance (small, or `g`)

```
╭─ M4 Pro ─────────────────────────────────────────── 42° nominal ─╮
│ cpu  18%  8.2W     ⣿⣷⣧⣇⡄⢀⠀⠀⠀⠀⠀⠀⠀⠀  busy 41%          │
│ gpu  12%  1.1W     ⣿⣆⡀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀  461 MHz            │
│ ane   0%  0.0W                                                   │
│ mem  22 / 36G  ●                                                 │
╰──────────────────────────────────────────────────────────────────╯
 ?   q
```

This *is* macmon’s job, in our skin. No process list until they hit `w` or grow the window.

### 3.3 Responsive

`LayoutPlan` from `(cols, rows, surface, chip)` before draw.

- Downgrade graphs: tall braille → 2-row → 1-row → number.
- Hide order (Work): per-core → GPU numbers in the CPU subline → mem line stays as long as possible → proc never hides on Work (if it cannot fit, we are in Glance).
- Missing hardware omitted. Fanless: no rpm. No ANE channel: no ane row.

## 4. Visual system

### 4.1 Principle

**Calm until it isn’t.**

Idle: dim borders, empty graphs, almost no color. Load: mint fills the hero. Throttle / warn / critical: the hero **stains** gold then red, thermal word appears, we do not need a second box.

One hero accent (CPU mint). GPU magenta and proc amber exist but **only on their numbers**, not as four competing picture frames.

### 4.2 Tokens

Same palette as before (`#0b0d10` bg, mint/gold/red ramp) but **usage changed**:

- Borders default to a single dim line. Accent the **focused** box only.
- Gradient on the graph, not on every label.
- Thermal `nominal` is omitted. We show `fair` / `serious` / `critical` when it leaves home.
- Pressure `●` is dim; warn/crit recolors it.

### 4.3 Chrome

- Titles: `name  primary  secondary`. Max two numbers in the bright style.
- Footer: `? / q` only. Focused proc adds `k` in the footer dynamically.
- `?` is a one-screen overlay: surfaces, filter, kill, expand, freeze, interval.
- Click targets = the same keys `?` lists. Mouse is real, not a tour.

### 4.4 Graph quality

Custom `BrailleHistory`. Craft bar is still “as good as btop,” with product rules on top:

| Rule | Why |
| --- | --- |
| 2 samples / cell, 4 levels / row | density |
| **15 min** ring at 1 s (~900 points); draw path buckets to `2 × width` | diagnosis, not a 2-minute toy |
| Peak pip on the max-in-view column | operators |
| **Throttle mark:** column where thermal ≠ nominal *or* (busy high ∧ freq slumped ∧ watts flat) | the Mac story |
| Idle = empty. No `no_zero` heartbeat | calm |
| No tweening | honesty |
| Auto-scale only net/disk, labeled | no jumping CPU axis |
| `block` / `tty` fallbacks | unreadable braille is worse than ugly |

Encode `U+2800 + bits`. Do not copy btop tables.

## 5. How it works

Unchanged physically, stricter product cadence:

- Sampler thread: IOReport sleep **is** the 1 s window. Mach/net/disk cheap. Processes every tick in Work, every 2 s in Glance. SMC/HID 2–10 s.
- UI thread: TEA, draw on dirty only, DEC 2026 sync.
- First second: `…` not `0`.
- Zero config file until the user changes surface, interval, or kill-confirm off.
- `f` freeze. `e` expand focused (proc detail, or a tall CPU graph).

Keys v1 (short list on purpose):

| Key | |
| --- | --- |
| `q` / Esc | quit / close overlay |
| `?` | help |
| `g` / `w` | Glance / Work |
| `/` | filter (Work) |
| `t` | tree (Work) |
| `k` | kill + confirm (Work, footer shows it when a row is selected) |
| `e` | expand |
| `f` | freeze |
| `[` `]` | 0.5 / 1 / 2 s |

No `r` on the home screen. If we add it, it swaps which number is large; both labels stay.

## 6. Ease without losing power

| Power | Where it lives |
| --- | --- |
| Scaled vs busy | both visible; no mode |
| E/P/S | compact meters on Work header; full on expand |
| Watts / ANE / MHz | Glance always; Work header subline |
| Sensors | expand CPU or a later page — not a preset |
| Disk / net | later toggle or “io is busy” promotion, not default |
| Themes | one excellent dark in v1 |
| Kill / tree / follow | Work, standard keys |

The functionality is still there. It is **revealed**, not tiled.

## 7. Accurate vs assumed

The honesty table in the previous draft stands. Product-facing rules:

- Headline `%` on AS = **scaled**. Word `busy` prefixes the Mach/active figure.
- Watts labeled by context as SoC, never wall.
- `°` is grouped hotspot/avg; expand for keys.
- RAM `22 / 36G` is the AM-style mix; we do not say “free.”
- Pressure is three states, not a fake 0–100.
- No VRAM. No per-pid GPU. No first-sample zero.

## 8. Plan (process + build)

### Process

1. Name the job and what we **removed** before adding a pane.
2. No third surface until Glance and Work feel inevitable.
3. Craft gate: braille quality vs btop. Product gate: the ten-second test.
4. v1 hardware: Apple Silicon. Intel hide-or-degrade.

### Build order

1. Calm chrome: one box, dim border, `? / q`, no kitchen footer.
2. `BrailleHistory` + Mach/IOReport CPU. Empty at idle. Stain + peak pip. **Craft gate.**
3. Work: process table, `/`, `k` confirm. **Product gate.**
4. Header: watts, busy %, GPU, mem line, thermal word.
5. Glance surface + auto-switch on small terminals.
6. Expand, freeze, tree.
7. Disk/net promotion (only if a week of living with 1–6 still hurts).
8. Intel, sensors page, themes — after people use it.

Until 3 ships, we do not have a product. Until 2 is beautiful, we do not have taste.

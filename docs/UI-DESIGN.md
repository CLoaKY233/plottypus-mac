# plottypus UI design

The design contract for this app. If code and this file disagree, one of them is wrong — fix it
here first, then the code. Companion: [ROADMAP.md](../ROADMAP.md) tracks what is left to build.

## 1. Stance

A minimalist Apple Silicon monitor that stays quiet until the machine isn't. One hero story per
surface, one accent family per panel, no decoration without information.

| | btop | macmon | plottypus |
| --- | --- | --- | --- |
| First 3 seconds | cinematic density | small silicon readout | **one labeled hero + quiet chrome** |
| CPU truth on AS | Mach ticks only ("100% busy" at 600 MHz looks like 4 GHz) | scaled + active, labeled | **scaled headline, `busy` rides along dim** |
| Act on processes | excellent | none | **filter → detail popup → kill without leaving it** |
| Small pane | ugly squeeze | native | **Glance surface, auto-selected** |
| Background | paints its own | paints its own | **terminal decides** (transparency-safe) |
| Renderer | custom C++ | ratatui sparklines | **ratatui braille history** |

We win by refusing btop's density and macmon's passivity at the same time.

## 2. Information architecture

Two surfaces, nothing else:

- **Work** (wide default): health rail on the left (`cpu | gpu` heroes over `mem | sens`, net
  opt-in), process table owns ~55% of width on the right. Enter expands any card into a full-body
  cell grid.
- **Glance** (small terminal or `g`): cpu hero absorbs all slack, compact strips pinned to the
  bottom border.

Panel order is fixed everywhere: `cpu · gpu · mem · net · disk · sens · proc`. Tab cycles it,
Enter expands it, Esc returns home.

A cell or graph is drawn only when the snapshot has a real value. Missing Option metrics
(`watts`, live MHz, ANE, zone °C, GPU die temp) stay off the board. History rings never
receive `0.0` for a missing sample. Per-process GPU is not collected on macOS and is not
shown. Disk compact/expanded graphs are I/O bytes, not the near-static used-ratio.

| Panel | Compact | Expanded | Graph |
| --- | --- | --- | --- |
| cpu | % + busy (if scaled≠active) + °C + thermal word + SoC spec | load, power/clock/temp only if present, load + temp graphs, cluster bars, cores if `show_cores` | load auto (10% floor); package °C band |
| gpu | % + °C + watts if present + core count | util, power/ANE/clock/temp/cores only if present | util auto (10% floor); temp band |
| mem | used/total + pressure + wired/compr/cache/swap | used, swap/cache if nonzero, composition bars | used % fixed 0–100 |
| net | iface ↓ ↑ | down / up cells | rx + tx bits, auto scale |
| disk | volume used/total bar + R/W | volume bars + activity + split read/write | I/O bytes, auto scale |
| sens | named zone/package °C + fan RPM | per-fan RPM graph, cpu/gpu temp band graphs, readings list | fans RPM auto; temps band |
| proc | pid name cpu% mem (+ threads) | same table + dossier (identity, live, command, cpu spark, family) | per-pid cpu spark in dossier |

## 3. Component hierarchy

```
Surface (Work | Glance)
└── Panel (bordered, titled, ↗ / × corner mark, focus ring on border color)
    ├── compact view   — title tokens + graph + optional subline
    └── expanded view  — Grid of Cells (macmon-style)
        ├── stat band   : 3–4 titled cells, one line each
        ├── graph band  : full-width or split bordered graphs
        └── detail band : cluster / composition / fan cells with bottom-anchored bars
Footer (contextual verbs only)   Overlays (help, settings, kill confirm, process detail)
```

Primitives live in `crates/plottypus-ui/src/chrome.rs`: `panel_block`, `panel_title`,
`push_token`, `push_kv`, `Graph { .. }` + `render_scaled_graph`, `render_fill_bar`.
Cells live in `widgets/expanded.rs::cell`. Nothing draws raw borders outside these two files.

## 4. Layout algebra

- `plan(area, surface, flags) -> LayoutPlan` is pure geometry; widgets never self-measure.
- **Work gate:** 60 cols × 16 rows (24 proc + 36 metrics); below it, Glance — never squeeze.
- **Process share:** default 55%, clamp 35..72 (`PROC_RATIO_*` in core), drag-resizable.
- **Degrade ladder:** `Full → Tight → Minimal` from left-rail width × body height.
  Hide order: cores grid → zone graphs → mem specs → fans graph → mid-row graphs → spec lines.
  Never wrap text, never collide meters. Expanded views never degrade.
- **Slack policy:** leftover vertical space goes to the current hero (Work: cpu/gpu row;
  Glance: the cpu panel unions the fill row directly beneath it).
- Rows use odd heights (3/5/7) so rounded borders stay symmetric top and bottom.

## 5. Spacing law

Positive space — measured in terminal columns/rows:

1. Border → content: exactly **1 space**. Title prefix is `" label"` inside its own span.
2. Between tokens: exactly **2 spaces**, emitted only by `push_token`/`push_kv` (the helper skips
   the separator when the previous span already ends in whitespace — separators never stack).
3. Key/value pairs render as `key␣␣value`; keys are `theme.dim()`, values carry the data style.
4. Numbers are right-aligned in fixed-width spans (`{:>5}`); names truncate to width with `…`.
5. Bars anchor to the **bottom row** of their region; text anchors top-left.
6. Corner mark occupies the last 3 columns of the title row: `" ↗ "` home, `" × "` expanded.

Negative space — what we deliberately leave empty:

- Idle graphs stay empty (no heartbeat dots, no fake axis ink).
- Bit/byte axes keep a 7-col gutter. Auto percent and band °C/RPM show a faint corner hint
  (`10%`, `45°`, `1.8k`) so a zoomed graph is never mistaken for 0–100. Fixed percent has no
  hint (the top is 100%).
- Thermal `nominal` prints nothing. Pressure prints a dot, colored only when it leaves nominal.
- Footer lists base verbs `? help  q quit`; contextual verbs appear only in context
  (`x kill` over the process table, `esc home` expanded, `f paused` frozen).
- Status messages expire after 4 s instead of squatting in the footer.

## 6. Typography

A TUI cannot choose fonts — the terminal emulator does. What we own:

**Glyph inventory** (all must render in the user's font):

| Glyphs | Use |
| --- | --- |
| `U+2800..U+28FF` braille | history graphs, two samples per cell, bottom-fill |
| `━ ─` | fill bars (filled/track), same row height as text |
| `╭ ╮ ╰ ╯ ─ │` | rounded panel/cell borders |
| `↓ ↑ ● ° … — ▌` | rates, pressure, degrees, pending state, empty value, cursor |

**Recommended fonts** (README-worthy): JetBrains Mono, Berkeley Mono, SF Mono, Iosevka — any
mono with full braille coverage and slab-ish dots. Fonts whose braille glyphs are thin gaps
(e.g. some default Windows consoles) degrade graph readability; the `tty` fallback idea remains
a non-goal until someone ships us a screenshot that needs it.

**Type ramp** (weight, not size — terminals give us one size):

- Hero numbers: `title` color + BOLD modifier, one per view max.
- Titles: `dim` label + `title` values.
- Sublines/specs: all `dim`.
- Data accents: one color family per panel (cpu mint, gpu mint, mem/disk/fan gold, net lavender,
  temp blue), brightened for translucent backgrounds.

## 7. Color system

No painted background anywhere — `Color::Reset` semantics: the terminal decides. Tokens
(`theme.rs`), tuned for blurry/translucent backdrops:

| token | value | used for |
| --- | --- | --- |
| `fg` | `#f2f2f2` | body text |
| `title` | `#ffffff` | headline numbers (+ BOLD for heroes) |
| `dim` | `#9a9a9a` | labels, sublines, secondary |
| `cpu` `gpu` `ok` | `#82e2aa` | load ink, positive |
| `mem` `disk` `fan` | `#eed97e` | capacity ink |
| `net` | `#c3bdf7` | transfer ink |
| `temp` | `#79b8f0` | temperature ink |
| `warn` / `crit` | `#f5c542` / `#ff7070` | thermal stain ramp endpoints |
| `border` / `border_focus` | `#c0c0c0` / `#ffffff` | unfocused / focused frame |

Rules: the stain ramp lerps `dim → accent` by cell intensity; thermal Fair repaints the whole
series gold, Serious/Critical red. Selection highlight is the one permitted background paint
(dark red) because it must survive any terminal theme.

## 8. Rendering strategy

- **Sampler thread** owns the collectors (confinement — no locks, no shared mutable state);
  commands flow down `mpsc` (`Interval/Paused/Quit`), `Result<Snapshot>` flows up. FFI latency
  can never stall a frame.
- **Draw cadence:** the event loop redraws once per interval tick or on input; ratatui diffs
  against the previous buffer so quiet frames cost ~nothing.
- **History:** 900-sample rings (~15 min at 1 s), downsampled to `2 × width` buckets in the draw
  path, drawn bottom-fill braille with a small-value bias so 2% still lights a dot.
- **Verification:** every layout change gets eyeballed through the ignored
  `visual_dump::dump_all_expanded` test (ASCII frames of every expanded panel), plus pty runs at
  boundary sizes (61×16, 60×24) checking boot/render/quit and a size-sweep test that replans and
  hit-tests hundreds of geometries for panics.

## 9. Code patterns (the house style)

Workspace lints deny `unwrap`, `expect`, `panic!`, `dbg!`, `process::exit`; production code
returns `Result`. Tests opt back in per module. The UI crate contains **zero `unsafe`** — FFI is
confined to `plottypus-metrics`, cfg-gated, each block carrying a SAFETY comment.

Helpers over literals:

```rust
pub fn panel_title(label: &str, theme: &Theme) -> Line<'static> {
    Line::from(vec![Span::styled(format!(" {label}"), theme.dim())])
}

pub fn push_token(spans: &mut Vec<Span<'static>>, text: String, style: Style) {
    if !spans.is_empty()
        && !spans.last().is_some_and(|span| span.content.ends_with(' '))
    {
        spans.push(Span::raw("  "));
    }
    spans.push(Span::styled(text, style));
}
```

One graph type, eight fields behind a struct — call sites stay flat:

```rust
render_scaled_graph(
    frame,
    inner,
    Graph {
        history: view.cpu_history,
        accent: theme.cpu,
        theme,
        scale: Scale::Fixed(1.0),
        axis: Axis::Percent,
        ink: GraphInk::Load(view.snapshot.thermal),
    },
);
```

Cells are the only way to draw a boxed section in an expanded view:

```rust
fn cell(frame: &mut Frame, area: Rect, title: &str, theme: &Theme) -> Rect {
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .title(Line::from(Span::styled(format!(" {title}"), theme.dim())))
        .border_style(theme.border(false));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    inner
}
```

Geometry degrades before content does:

```rust
impl Degrade {
    fn for_left_rail(width: u16, height: u16) -> Self {
        if width < 50 || height < 17 {
            Self::Minimal
        } else if width < 64 || height < 22 {
            Self::Tight
        } else {
            Self::Full
        }
    }
}
```

## 10. Checklist status

- [x] Minimalist, aligned layouts (spacing law enforced by helpers + contract test)
- [x] Positive/negative spacing defined and applied
- [x] Glyph/typography system documented; font recommendations for users
- [x] Clean, comment-light samples; zero unsafe in the UI crate
- [x] btop/macmon deltas stated above
- [x] Empty Option cells (power / clock / zone °C / per-pid GPU) stay off the board
- [ ] Remaining build work lives in [ROADMAP.md](../ROADMAP.md): battery (IOPS), and
      the IOReport residency project that unlocks real `scaled`, frequencies, PSTR watts
      and throttle marks.

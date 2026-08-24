# Product critique — Head of Product

Last updated: 2026-08-24.

This is a red-pen on [05-product-design.md](05-product-design.md), the mockup, the synthesis plan, and how we have been working. The improved PRD is the same `05` file, rewritten. This file stays so we do not walk the decisions back.

## Verdict

The research is excellent. The draft PRD would ship a **beautiful btop remix with extra Mac numbers**. That is not enough. Users already have btop. Users who care about watts already have macmon. A mashup inherits both audiences’ disappointments: *“pretty but wrong on my M-series”* and *“correct but I still open Activity Monitor to kill something.”*

We do not win by displaying everything we learned. We win if a Mac user opens this, immediately sees **whether the machine is working hard or just busy**, and can **act** without leaving the terminal — while the idle screen stays almost silent.

**Ship/no-ship test:** a stranger with a hot MacBook can answer “who, what, how hot” in ten seconds, then kill or ignore, without reading a footer of twelve keybindings.

The draft fails that test. Too many boxes, too many meanings of `%`, too many presets, too little product spine.

---

## What is actually good (keep)

- Honesty taxonomy (measured / derived / modeled / grouped). Rare and valuable.
- Watts + MHz next to % — the only reason this can beat btop on a Mac.
- E/P/S as families, not a flat core list.
- No sudo, no `powermetrics`, no self-DDoS sample rate.
- Braille as craft, not Chart-with-axes.
- Hide missing hardware.
- Expand + freeze (bottom) are the right *power* verbs.
- “Don’t tween the silicon.”

None of that is the product. That is the **physics and the craft**. The product is what is on screen at first launch, and what the user does next.

---

## The hard problems

### 1. There is no user, only a crate list

“btop’s skin + macmon’s silicon + bottom’s table” is an engineering brief. It is not a job-to-be-done.

Who opens this, when?

| Person | Moment | Job |
| --- | --- | --- |
| AS developer | fans spin, Xcode/LLM, laptop on thighs | Is this load real? Which process? Kill or wait? |
| Same person | small pane in a multiplexer | Glance watts / GPU / temp without a dashboard |
| btop refugee | every day, wide terminal | Keep the ritual, stop being lied to about CPU % |
| macmon fan | already has JSON/Prometheus | They do not need us unless we add **action** |

Draft PRD tries to be all four at once, at first paint. That is how you get five presets and a footer that looks like a vim cheatsheet.

**Decision:** v1 is the first two people. Daily driver + small-pane glance. Prometheus, Intel, sensor lab, theme files are later.

### 2. The default is a kitchen sink wearing minimalism

The dashboard mockup has CPU, per-core, fans, temp, mem composition, swap, pressure, GPU, ANE, disk, net up, net down, process table, chip identity, and eight footer verbs.

That is btop density. btop earns density after years of muscle memory. We have not earned it. Calling the interiors “quiet” does not make the *composition* quiet.

**Minimalism is not fewer colors. It is fewer decisions on first paint.**

Disk + net glued into one pane is the tell: we knew it was too much and compressed two products into a junk drawer.

### 3. `%` is an unforced identity crisis

`r` toggles whether the headline `%` means “scheduled” or “frequency-weighted.” That is a researcher’s control. For everyone else, **the same glyph just changed meaning.** That is how you get “this app is lying” tweets.

btop is wrong-but-stable (Mach ticks). macmon is right-but-explicit (two ratios, labeled). We proposed wrong-or-right depending on a hidden mode.

**Decision:** one headline. On Apple Silicon it is **scaled** (how hard). Active sits next to it in dim type: `busy 41%`. No toggle required to read the screen. `r` can swap emphasis later; it must never silently redefine `%`.

### 4. Five presets is a museum of indecision

dashboard / silicon / classic / proc / sensors is us refusing to choose a home screen. Users will not learn five. They will use one and think the rest is bloat.

btop’s 1–9 works because the *default* is already the product. Presets are seasoning.

**Decision:** two surfaces.

- **Glance** — machine health. Auto when the window is small.
- **Work** — processes dominate. One key away.

Sensors is a later page. “Classic btop clone” is not a product; it is nostalgia.

### 5. The visual system is tasteful and slightly generic

Mint CPU, green mem, magenta GPU, amber proc is the “good TUI theme pack.” Fine. It is not memorable. Combined with rainbow identity borders on every pane, the eye has **five heroes**. There should be **one**.

Idle state is wrong. `no_zero` heartbeat so the pane “never looks dead” is game-UI anxiety. A calm machine should look **empty**. Luxury is negative space. When thermal goes `fair` or watts spike, *then* the graph stains. That contrast is the brand.

Titles are status bars. `cpu  18%  ·  E 6%  P 31%  ·  8.2W  42°  nominal` is a paragraph. The title should be `cpu  18%  8.2W`. Supporting facts belong on a dim second line or on expand.

Footer with `q ? 1-5 e f r / t` trains people that this is a tool they must study. Show `?  /  q`. Discover the rest inside `?` and on focus.

### 6. Charts are high craft, short memory, no story

Two minutes of history at 1 s is a screensaver, not a diagnosis. “Why was it hot while I was in that call?” needs **10–15 minutes**, downsampled in the draw path, not dropped.

There is no event language. The Mac-specific story is **busy but throttled** (freq drops, watts flatten, thermal leaves nominal). If the graph does not mark that, we are btop with a watt number in the title.

No peak pip. No “this column is the max in view.” Operators use that constantly.

### 7. Process action is a side quest

The stated working surface is PROC, then it is given the leftover rectangle under a junk-drawer IO pane. The loop that makes someone **stay** is:

`/ xcode` → see CPU + mem → `k` → confirm.

That loop must be beautiful and default-close. Work view exists so this is 80% of the pixels, not 30%.

### 8. Positioning vs btop and macmon is not sharp enough

| | btop | macmon | draft plottypus | what we should be |
| --- | --- | --- | --- | --- |
| First 3s | pretty boxes | small silicon | pretty boxes + more numbers | **quiet, one story** |
| CPU truth on AS | weak | best | toggle | **obvious without a toggle** |
| Act on a process | excellent | none | planned, cramped | **two keys** |
| Small pane | ugly | native | “find preset 2” | **automatic glance** |
| Weight | low | very low | unknown, risky | **as light as macmon + a table** |

Why switch from btop? Not “rounded corners.” Because **100% stops meaning nothing** when the machine is cooking, and because GPU/ANE/watts exist.

Why switch from macmon? Not “we also have braille.” Because **you can finish the job** (find / kill) and it still fits a pane.

If we ship the draft default, btop users say “ok clone,” macmon users say “too big,” and we have no tribe.

### 9. Intel and the kitchen-sink roadmap are process smells

Intel in v1 is a second product (different GPU, different temps, no Energy Model). It delays the screenshot that wins Twitter.

Build order “braille until it looks like btop” is a craft gate, not a product gate. The product gate is the **vertical slice**: AS CPU truth + watts + process filter/kill + calm chrome. A gorgeous empty graph with no job-to-be-done is a demo reel.

We have been in research-maximalism. That was correct last turn. It is incorrect as a way of life. Critique should have happened *before* five presets were written down as if they were requirements.

---

## Pros of the current direction (if we shipped it unchanged)

- Instantly readable as “serious TUI”
- Mac metrics catalog is best-in-class on paper
- Honesty about models vs measurements
- Cheap collector design
- Path to both pretty and useful

## Cons (if we shipped it unchanged)

- No first-run story; looks like btop homework
- `%` ambiguity
- Default density fights the “simple vibe” we claim
- Five presets = we did not decide
- Short graphs, no throttle language
- Process workflow is second-class
- Footer and titles are noisy
- Intel + sensors + classic + Prometheus fantasy in the same breath as v1
- Easy to lose to btop on vibe and to macmon on focus

---

## Decisions that actually move the product

These are locked unless a later critique beats them. They are implemented in the rewritten PRD.

1. **One sentence:** *The Mac monitor that stays quiet until the machine isn’t — then tells you who, how hard, and lets you act.*
2. **Two surfaces, not five presets.** Glance and Work. Small window ⇒ Glance, automatically.
3. **Default (wide) is CPU + GPU/power + one MEM line + PROC.** No disk, no net, until they are the story or the user asks.
4. **One meaning of `%`.** Scaled is the headline on AS. `busy` is dim, always visible. No silent toggle.
5. **Idle is empty.** No heartbeat dots. Accents dim. When thermal/watts/pressure leave nominal, the hero graph **stains**.
6. **Titles are three tokens max.** `cpu  18%  8.2W`. Everything else is a dim subline.
7. **Footer is `? / q`.**
8. **History is 15 minutes**, drawn at cell resolution, with a peak pip and throttle marks.
9. **Work view is the process product.** Glance is the health product. Both are first-class; default wide opens closer to Work (proc is large). Default small opens Glance.
10. **v1 is Apple Silicon only.** Intel = don’t crash, hide what we don’t have.
11. **Vertical slice before dashboard complete.** CPU truth + proc kill + calm frame. Then GPU/watts. Then mem. Then io.
12. **Zero config on first run.** Write a file only after the user changes something.

## Process changes

- Every layout proposal must name the **job** and what was **removed**.
- No third view until Glance and Work feel inevitable.
- “Looks like btop” is a graph-quality bar, not a product bar.
- Success: time-to-insight < 10s, time-to-kill < 5s after insight, self CPU < 2% at 1s, a user can use it for a week without opening `?`.

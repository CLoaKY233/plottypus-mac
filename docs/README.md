# Plottypus docs

Research notes for a **macOS-first**, **Ratatui** system monitor. Nothing here is an implementation plan that has been executed — it is a reusable reference so we do not re-learn Ratatui, SMC, IOReport, or btop every time we sit down to build.

**Do not treat these files as stale scratch.** Update them when we learn something new about a widget, an API, or a chip generation.

| File | What it is |
| --- | --- |
| [research/00-overview.md](research/00-overview.md) | Goal, constraints, source list, how to use this set |
| [research/01-ratatui.md](research/01-ratatui.md) | Every built-in widget, layout, style, canvas/markers, backends, third-party crates, performance |
| [research/02-macos-metrics.md](research/02-macos-metrics.md) | How macOS exposes CPU, GPU, ANE, RAM, storage, net, fans, temp, power, battery, processes — APIs, privilege, cost |
| [research/03-prior-art.md](research/03-prior-art.md) | btop, macmon, bottom, Stats.app, iSMC, asitop, and what each actually does well |
| [research/04-synthesis.md](research/04-synthesis.md) | How those three threads combine: aesthetics, metric set, efficiency, responsive layout. Still not code. |
| [research/05-product-design.md](research/05-product-design.md) | PRD — two surfaces, calm default, accuracy |
| [research/06-product-critique.md](research/06-product-critique.md) | Head-of-product red pen and locked decisions |
| [research/08-dashboard-ux-rewrite.md](research/08-dashboard-ux-rewrite.md) | Training-run cockpit: cross-links, packer, smoothness. Next implementation plan. |
| [mockups/dashboard.html](mockups/dashboard.html) | Work + Glance visual target |

Cloned sources used while writing these (not vendored into this repo):

- `https://github.com/ratatui/ratatui` (0.30.2 workspace)
- `https://github.com/aristocratos/btop`
- `https://github.com/vladkens/macmon`
- `https://github.com/ClementTsang/bottom`
- `https://github.com/exelban/stats`
- `https://github.com/dkorunic/iSMC`
- `https://github.com/ratatui/tui-widgets`
- `https://github.com/ratatui/awesome-ratatui`

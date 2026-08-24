# plottypus

Apple Silicon system monitor TUI. Quiet until the machine isn’t.

```
cargo run -p plottypus
```

| Crate | Role |
| --- | --- |
| `plottypus-core` | Types, errors, history, config |
| `plottypus-metrics` | Collectors (no ratatui) |
| `plottypus-ui` | Theme, braille, layout, widgets |
| `plottypus` | Binary / event loop |

**Read first:** [docs/README.md](docs/README.md) · [crates/ARCHITECTURE.md](crates/ARCHITECTURE.md)

| Doc | Contents |
| --- | --- |
| [docs/research/00-overview.md](docs/research/00-overview.md) | Goal, constraints, sources |
| [docs/research/01-ratatui.md](docs/research/01-ratatui.md) | Widgets, layout, braille/dot graphs, third-party crates |
| [docs/research/02-macos-metrics.md](docs/research/02-macos-metrics.md) | CPU / GPU / ANE / RAM / disks / net / fans / temp / power / processes |
| [docs/research/03-prior-art.md](docs/research/03-prior-art.md) | btop, macmon, bottom, Stats, iSMC |
| [docs/research/04-synthesis.md](docs/research/04-synthesis.md) | How those combine — still not an implementation |
| [docs/research/05-product-design.md](docs/research/05-product-design.md) | PRD |
| [docs/research/06-product-critique.md](docs/research/06-product-critique.md) | Product critique |
| [docs/mockups/dashboard.html](docs/mockups/dashboard.html) | Work + Glance mockup |

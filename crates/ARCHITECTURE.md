# Crate map

| Crate | Role | May depend on |
| --- | --- | --- |
| `plottypus-core` | Errors, snapshot types, history, config, surfaces | nothing in this workspace |
| `plottypus-metrics` | Sampling. No ratatui. | core |
| `plottypus-ui` | Theme, braille, layout, widgets | core, ratatui |
| `plottypus` | Binary: event loop, TEA | core, metrics, ui |

`plottypus-metrics` must never import `ratatui` or `plottypus-ui`.
`plottypus-ui` must never import `plottypus-metrics`.

Vertical slice: Work + Glance chrome, Mach CPU, memory, processes, filter/kill, braille history.

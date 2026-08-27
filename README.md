# plottypus

[![CI](https://github.com/CLoaKY233/plottypus-mac/actions/workflows/ci.yml/badge.svg)](https://github.com/CLoaKY233/plottypus-mac/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

A native system monitor for Apple Silicon Macs, in your terminal. Quiet until the machine isn't.

No Electron, no sysinfo-scraping, no `ps` shells-out. plottypus talks to macOS directly through raw FFI — IOKit's SMC for fans and temperatures, `IOHIDEventSystemClient` for sensor data, and sysctl/host APIs for CPU, memory, and processes — then renders it all with [ratatui](https://ratatui.rs) at a fraction of the cost of comparable tools.

## Features

- **Two surfaces**: **Work** for deep inspection, **Glance** for a compact always-on view (auto-selected by terminal size, switch anytime)
- **CPU** per-core utilization grouped into efficiency / performance / super clusters, with frequency
- **GPU** load and temperature
- **Memory** pressure, swap, wired/compressed breakdown
- **Thermals** merged from SMC keys and HID sensor nodes into P/E/GPU zones — real °C or nothing
- **Fans** current RPM with per-fan readouts
- **Network** RX/TX rates, **disks** capacity and activity
- **Process table** with live search, sort by CPU/memory/PID, details on click, and kill (with confirmation)
- Braille sparkline history for every metric; mouse support throughout
- Toggles to hide anything you don't care about (`s` for settings)

## Install

Requires macOS on Apple Silicon and Rust 1.88+.

Tagged releases attach `plottypus-<tag>-macos-aarch64.tar.gz` (and a Linux stub build) as GitHub Release assets.

```sh
cargo install --git https://github.com/CLoaKY233/plottypus-mac
```

Or build from source:

```sh
git clone https://github.com/CLoaKY233/plottypus-mac
cd plottypus-mac
cargo install --path crates/plottypus
```

Other platforms compile (the collectors are stubbed out) but sample nothing — this is an Apple Silicon tool.

`cargo test --workspace` is the default suite. Live `Sampler` tests take an exclusive IOKit lock so two collectors cannot open SMC/HID at once; they run in parallel with everything else, just not with each other.

## Usage

```sh
plottypus
```

### Keys

| Key | Action |
| --- | --- |
| `w` / `g` | Switch Work / Glance surface |
| `Tab` / `Shift+Tab` | Cycle panels |
| `Enter` | Expand panel / open process details |
| `/` | Search processes |
| `j` `k`, arrows, scroll | Move selection |
| `x` | Kill selected process (`y` confirms) |
| `[` `]` | Cycle refresh interval (0.5s / 1s / 2s) |
| `f` | Freeze display |
| `s` | Settings |
| `?` | Help |
| `q` or `Ctrl+C` | Quit |

In settings: `1` interval · `2` GPU · `3` network · `4` cores · `5` disks · `6` fans · `7` sort order · `8` threads.

## Architecture

Four crates with a strict dependency direction — core ← {metrics, ui} ← bin:

| Crate | Role |
| --- | --- |
| [`plottypus-core`](crates/plottypus-core) | Snapshot types, errors, history buffers, config |
| [`plottypus-metrics`](crates/plottypus-metrics) | Collectors + sampler. No ratatui, no UI types |
| [`plottypus-ui`](crates/plottypus-ui) | Theme, braille graphs, layout, widgets. No metrics imports |
| [`plottypus`](crates/plottypus) | Binary: event loop, input handling |

Each frame, `Sampler::tick()` builds one immutable `Snapshot`; widgets render it read-only. See [crates/ARCHITECTURE.md](crates/ARCHITECTURE.md).

The `docs/research/` directory is a curated knowledge base covering ratatui internals, every macOS metrics API we found (with privilege requirements and cost), and prior art (btop, macmon, bottom, Stats). If you're writing your own macOS monitor, start there.

## Development

```sh
cargo fmt --all                          # required before committing
cargo clippy --workspace --all-targets   # must be clean (unwrap/panic/expect denied)
cargo test --workspace                   # note: plain `cargo test` only covers the bin crate
cargo run -p plottypus
```

CI runs fmt check, clippy with `-D warnings`, and the full test suite on macOS and Linux.

## License

[MIT](LICENSE)

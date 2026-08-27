# AGENTS.md

Apple Silicon system monitor TUI (`plottypus`). macOS-first: metrics collectors use raw FFI (IOKit SMC, IOHIDEventSystemClient, sysctl/host APIs) behind `#[cfg(target_os = "macos")]`; other targets compile against stubs but sample nothing. Edition 2024, MSRV 1.88.

## Commands

```sh
cargo fmt --all                      # required step; default rustfmt settings (no rustfmt.toml)
cargo clippy --workspace --all-targets   # must be clean
cargo test --workspace               # NOT plain `cargo test`: default-members is the bin crate only
                                     # CI runs this with -- --test-threads=1 (live IOKit contends)
cargo run -p plottypus
```

Single test example: `cargo test -p plottypus-metrics fan::tests::smc_struct_is_80_bytes`

Order matters: fmt → clippy → test. CI (`.github/workflows/ci.yml`) runs exactly these on macOS + Linux, clippy with `-D warnings`. All tests are inline `#[cfg(test)] mod tests` next to the code — there are no `tests/` directories.

## Hard workspace rules

- Dependency direction (`crates/ARCHITECTURE.md`): core ← {metrics, ui} ← bin.
  - `plottypus-metrics` must never depend on ratatui or `plottypus-ui`.
  - `plottypus-ui` must never import `plottypus-metrics`.
- Clippy lints (root `Cargo.toml` `[workspace.lints]`) deny `unwrap`, `expect`, `panic`, `dbg!`, and `process::exit`; pedantic is warn. Production code returns `Result` instead. Test modules opt back in with `#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]`.
- `unsafe_op_in_unsafe_fn` is denied; every `unsafe` block carries a SAFETY comment. Keep that convention for new FFI.
- New deps go in `[workspace.dependencies]` and are referenced as `name.workspace = true`.

## Entrypoints

- Binary/event loop: `crates/plottypus/src/{main,app,event,tui,worker}.rs` (worker thread owns the Sampler; the App drains snapshots over mpsc and owns all UI state).
- Sampling: `Sampler::tick()` runs on the worker thread and produces one `Snapshot` per interval; `App::apply_snapshot` pushes histories; UI widgets consume snapshots read-only.
- Per-collector modules live flat under `crates/plottypus-metrics/src/` (cpu, gpu, fan, thermal, memory, net, disk, process, hid, soc, zones, topology).

## Docs are living reference

`docs/research/*.md` is a curated knowledge base, not stale scratch — update it when we learn something new about an API, widget, or chip generation (`docs/README.md`). Read `crates/ARCHITECTURE.md` before touching crate boundaries.

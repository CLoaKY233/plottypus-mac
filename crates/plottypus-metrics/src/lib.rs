// Off macOS every collector is a stub: their helpers and fields are kept for
// symmetry with the real implementations but intentionally never used.
#![cfg_attr(
    not(target_os = "macos"),
    allow(
        dead_code,
        unused_imports,
        clippy::unused_self,
        clippy::unnecessary_wraps
    )
)]

mod cpu;
mod disk;
mod fan;
mod gpu;
mod hid;
mod memory;
mod net;
mod process;
mod sampler;
mod soc;
#[cfg(target_os = "macos")]
mod sys;
mod thermal;
mod topology;
mod zones;

pub use process::{Signal, send_signal};
pub use sampler::Sampler;

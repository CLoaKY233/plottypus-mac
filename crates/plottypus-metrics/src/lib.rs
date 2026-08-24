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

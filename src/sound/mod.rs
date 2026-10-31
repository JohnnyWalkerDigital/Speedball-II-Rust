//! Sound: a port of the original Amiga sound driver and a small emulation
//! of the Amiga's sound chip, playing the samples and sequences from the
//! memory they were in, with [`data`] laid back over it.

pub mod data;
pub mod driver;
mod export;
mod layout;
mod memory;
mod output;
pub mod paula;
pub mod sequences;
pub mod wav;

pub use export::{export_samples, export_sounds};
pub use output::SoundOutput;

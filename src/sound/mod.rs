//! Sound: a port of the original Amiga sound driver and a small emulation
//! of the Amiga's sound chip, playing the samples and sequences straight
//! from the match RAM dump.

pub mod driver;
mod export;
mod memory;
mod output;
pub mod paula;

pub use export::export_sounds;
pub use output::SoundOutput;

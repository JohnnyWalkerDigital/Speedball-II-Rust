//! Access to the original game data: disk images, RAM dumps and the
//! lookup tables the game relies on.

pub mod addr;
pub mod amiga;
pub mod animations;
pub mod bytes;
pub mod files;
pub mod lookups;
pub mod readable;
pub mod rom;
pub mod save_disk;

pub use files::GameFiles;
pub use rom::AtariRom;

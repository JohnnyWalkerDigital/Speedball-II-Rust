//! Access to the original game data: disk images, RAM dumps and the
//! lookup tables the 68000 code relies on.

pub mod addr;
pub mod amiga;
pub mod bytes;
pub mod files;
pub mod rom;
pub mod save_disk;
pub mod sounds;

pub use files::GameFiles;
pub use rom::AtariRom;

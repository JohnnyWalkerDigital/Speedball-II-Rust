//! Access to the original game data: disk images, RAM dumps and the
//! lookup tables the game relies on.

pub mod addr;
pub mod amiga;
mod amiga_memory;
pub mod animations;
pub mod bytes;
pub mod files;
pub mod lookups;
pub mod readable;
pub mod rom;
pub mod save_disk;

pub use files::GameFiles;
pub use rom::Rom;

/// The end of a check: fine if nothing was found, otherwise the problems,
/// one to a line.
pub fn report(problems: Vec<String>) -> Result<(), String> {
    if problems.is_empty() {
        Ok(())
    } else {
        Err(problems.join("\n"))
    }
}

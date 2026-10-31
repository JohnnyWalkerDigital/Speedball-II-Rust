//! The game save disk, as an Amiga disk image file.
//!
//! The original wrote its saves straight to sectors of a blank disk: a
//! league at sector 0, a cup at sector 22 (the second track) and goal
//! replays 1 to 9 on the tracks after. It asked for a "game save disk" or a
//! "goal save disk", but the two never overlap, so one image holds both.
//! Keeping a whole 880K image in the same layout means the saves are
//! exactly the original's.

use std::fs;
use std::io;
use std::path::PathBuf;

pub const SECTOR_SIZE: usize = 512;
const DISK_SIZE: usize = 901_120;
/// Sectors on a track (both sides).
const TRACK: usize = 22;

pub const LEAGUE_SECTOR: usize = 0;
pub const CUP_SECTOR: usize = TRACK;
pub const GAME_SECTORS: usize = 2;
pub const REPLAY_SECTORS: usize = 6;

/// Where goal replay `slot` (1 to 9) goes: from the third track, a track
/// each.
pub fn replay_sector(slot: u8) -> usize {
    2 * TRACK + (usize::from(slot) - 1) * TRACK
}

pub struct SaveDisk {
    path: PathBuf,
}

impl SaveDisk {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    #[cfg(test)]
    pub fn path(&self) -> &std::path::Path {
        &self.path
    }

    /// Reads `count` sectors from `start`. A missing disk reads as blank,
    /// which no save's magic matches.
    pub fn read(&self, start: usize, count: usize) -> io::Result<Vec<u8>> {
        let range = start * SECTOR_SIZE..(start + count) * SECTOR_SIZE;
        match fs::read(&self.path) {
            Ok(image) if image.len() >= range.end => Ok(image[range].to_vec()),
            Ok(_) => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "not a disk image",
            )),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(vec![0; range.len()]),
            Err(e) => Err(e),
        }
    }

    /// Writes `bytes` from sector `start`, padding to whole sectors and
    /// formatting a blank disk first if there is none.
    pub fn write(&self, start: usize, bytes: &[u8]) -> io::Result<()> {
        let mut image = match fs::read(&self.path) {
            Ok(image) if image.len() == DISK_SIZE => image,
            Ok(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "not a disk image",
                ))
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => vec![0; DISK_SIZE],
            Err(e) => return Err(e),
        };
        let at = start * SECTOR_SIZE;
        let padded = bytes.len().div_ceil(SECTOR_SIZE) * SECTOR_SIZE;
        image[at..at + padded].fill(0);
        image[at..at + bytes.len()].copy_from_slice(bytes);
        if let Some(dir) = self.path.parent() {
            fs::create_dir_all(dir)?;
        }
        fs::write(&self.path, image)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_and_reads_sectors_of_a_new_disk() {
        let dir = std::env::temp_dir().join(format!("sb2-disk-{}", std::process::id()));
        let disk = SaveDisk::new(dir.join("save.adf"));
        assert_eq!(
            disk.read(22, 2).unwrap(),
            vec![0; 1024],
            "no disk reads blank"
        );
        disk.write(22, b"cup data").unwrap();
        let sectors = disk.read(22, 2).unwrap();
        assert_eq!(&sectors[..8], b"cup data");
        assert_eq!(fs::metadata(disk.path()).unwrap().len() as usize, DISK_SIZE);
        fs::remove_dir_all(dir).unwrap();
    }
}

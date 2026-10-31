//! The Amiga disk's overlays and what the Amiga's memory holds in the menus.
//!
//! The Amiga game loads most of its data as numbered "overlays": runs of
//! sectors listed in a directory that starts with the magic `CA1990`. The
//! menus and the match need different overlays, so memory looks different
//! in each. `Amiga.dmp` was taken during a match; [`menu_ram`] rebuilds the
//! menus' memory from it the way the game does on entering the menus, so
//! menu code can read its strings, fonts and sounds at their original
//! addresses.

use std::ops::Range;

/// Where each overlay the menus need is loaded.
pub mod overlay {
    /// Title backdrop, menu fonts and other menu graphics.
    pub const MENU: usize = 1;
    pub const MENU_ADDRESS: usize = 0x361D6;
    /// Instrument samples for the menu sounds. The match loads its own
    /// samples (overlay 27) to the same place.
    pub const MENU_SAMPLES: usize = 28;
    pub const SAMPLES_ADDRESS: usize = 0x30D56;
    /// Full-screen pictures, loaded into the spare screen buffer.
    pub const VICTORY: usize = 12;
    pub const DEFEAT: usize = 13;
    pub const LEAGUE_WIN: usize = 14;
    pub const PROMOTION: usize = 15;
    pub const CUP_WIN: usize = 16;
    pub const KNOCKOUT_WIN: usize = 17;
}

const DIRECTORY_MAGIC: &[u8] = b"CA1990";
const SECTOR_SIZE: usize = 512;
/// Directory entries start 16 bytes into the directory sector.
const ENTRIES_OFFSET: usize = 0x10;

/// The overlay directory of an Amiga disk image.
#[derive(Clone, Copy)]
pub struct Overlays<'a> {
    disk: &'a [u8],
    directory: usize,
}

impl<'a> Overlays<'a> {
    /// Finds the directory sector. `None` if the image has none.
    pub fn new(disk: &'a [u8]) -> Option<Self> {
        let directory = disk
            .chunks_exact(SECTOR_SIZE)
            .position(|sector| sector.starts_with(DIRECTORY_MAGIC))?
            * SECTOR_SIZE;
        Some(Self { disk, directory })
    }

    /// The bytes of overlay `number`: each entry is a start sector and a
    /// sector count.
    pub fn get(&self, number: usize) -> &'a [u8] {
        let entry = self.directory + ENTRIES_OFFSET + number * 4;
        let word = |at: usize| usize::from(u16::from_be_bytes([self.disk[at], self.disk[at + 1]]));
        let sectors = word(entry)..word(entry) + word(entry + 2);
        let bytes = Range {
            start: sectors.start * SECTOR_SIZE,
            end: (sectors.end * SECTOR_SIZE).min(self.disk.len()),
        };
        &self.disk[bytes]
    }
}

/// Memory as the menus see it: the match dump with the menu graphics and
/// the menu sound samples loaded over the match's data.
pub fn menu_ram(match_dump: &[u8], overlays: &Overlays) -> Vec<u8> {
    let mut ram = match_dump.to_vec();
    load(&mut ram, overlay::MENU_ADDRESS, overlays.get(overlay::MENU));
    load(
        &mut ram,
        overlay::SAMPLES_ADDRESS,
        overlays.get(overlay::MENU_SAMPLES),
    );
    ram
}

fn load(ram: &mut [u8], address: usize, bytes: &[u8]) {
    let end = (address + bytes.len()).min(ram.len());
    ram[address..end].copy_from_slice(&bytes[..end - address]);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn disk_with_directory() -> Vec<u8> {
        let mut disk = vec![0; SECTOR_SIZE * 8];
        let dir = SECTOR_SIZE * 2;
        disk[dir..dir + 6].copy_from_slice(DIRECTORY_MAGIC);
        // Overlay 1: two sectors from sector 5.
        disk[dir + ENTRIES_OFFSET + 4..dir + ENTRIES_OFFSET + 8].copy_from_slice(&[0, 5, 0, 2]);
        disk[SECTOR_SIZE * 5] = 0xAB;
        disk
    }

    #[test]
    fn reads_overlays_from_the_directory() {
        let disk = disk_with_directory();
        let overlays = Overlays::new(&disk).unwrap();
        let one = overlays.get(1);
        assert_eq!(one.len(), 2 * SECTOR_SIZE);
        assert_eq!(one[0], 0xAB);
        assert!(Overlays::new(&[0; SECTOR_SIZE]).is_none());
    }

    #[test]
    fn loading_past_the_end_of_memory_is_cut_short() {
        let mut ram = vec![0; 4];
        load(&mut ram, 2, &[1, 2, 3]);
        assert_eq!(ram, [0, 0, 1, 2]);
    }
}

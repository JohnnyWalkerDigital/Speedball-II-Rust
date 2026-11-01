//! The Amiga's memory, rebuilt from the disk the way the game loads it.
//!
//! The boot block reads a small track loader, which reads the packed
//! program and the intro's data to 0x32000 and unpacks the program down to
//! 0x84: [`intro_memory`]. The rest of the game's data is in numbered
//! "overlays": runs of sectors listed in a directory that starts with the
//! magic `CA1990`. The menus and the match load different overlays over
//! the same memory: [`match_memory`] and [`menu_ram`]. Code that reads the
//! original's data then finds it at the original addresses.

use std::ops::Range;

use super::ipf::SECTOR_SIZE;

/// Where each overlay the match and the menus need is loaded.
pub mod overlay {
    /// The sound driver's tunes and instruments.
    pub const SOUND_BANK: usize = 0;
    pub const SOUND_BANK_ADDRESS: usize = 0x1BC3E;
    /// The match's sprites and other graphics.
    pub const MATCH_GRAPHICS: usize = 18;
    pub const MATCH_GRAPHICS_ADDRESS: usize = 0x3A2D6;
    /// The pitch's tiles.
    pub const PITCH: usize = 26;
    pub const PITCH_ADDRESS: usize = 0x4FED6;
    /// Instrument samples for the match's sounds.
    pub const MATCH_SAMPLES: usize = 27;
    /// Title backdrop, menu fonts and other menu graphics.
    pub const MENU: usize = 1;
    pub const MENU_ADDRESS: usize = 0x361D6;
    /// Instrument samples for the menu sounds, loaded to the same place.
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

/// The Amiga's chip memory.
pub const MEMORY_SIZE: usize = 0x80000;

/// What the track loader reads, and where.
const LOADED_SECTORS: Range<usize> = 22..622;
const LOAD_ADDRESS: usize = 0x32000;
/// The packed program, and where it unpacks to.
const PACKED_PROGRAM: usize = 0x3200A;
const PROGRAM: usize = 0x84;
/// Where the program the port reads ends, once unpacked. Other builds of
/// the game end elsewhere, and their data is laid out differently.
const PROGRAM_END: usize = 0x5CA22;

/// The copy protection, once satisfied, adds its key to these longs: a
/// panel message and two pieces of code.
const PROTECTED_LONGS: [usize; 3] = [0x25D6, 0xDCC8, 0x11006];
const PROTECTION_KEY: u32 = 0x78D4_C924;

/// The disk holds a build of the game other than the one the port reads.
#[derive(Debug, PartialEq, Eq)]
pub struct OtherBuild;

/// Memory as the intro starts: the loaded sectors, with the program
/// unpacked and unprotected.
pub fn intro_memory(disk: &[u8]) -> Result<Vec<u8>, OtherBuild> {
    let mut ram = vec![0; MEMORY_SIZE];
    let sectors = LOADED_SECTORS.start * SECTOR_SIZE..LOADED_SECTORS.end * SECTOR_SIZE;
    load(&mut ram, LOAD_ADDRESS, disk.get(sectors).ok_or(OtherBuild)?);
    if unpack(&mut ram, PACKED_PROGRAM, PROGRAM) != Some(PROGRAM_END) {
        return Err(OtherBuild);
    }
    for at in PROTECTED_LONGS {
        let long = u32::from_be_bytes([ram[at], ram[at + 1], ram[at + 2], ram[at + 3]]);
        ram[at..at + 4].copy_from_slice(&long.wrapping_add(PROTECTION_KEY).to_be_bytes());
    }
    Ok(ram)
}

/// The loader's unpacker, in place: blocks, each a little-endian length
/// and then codes. A code under 0x80 copies that many bytes as they are;
/// otherwise it copies earlier output, from an offset into the block's own
/// output, for the code's low seven bits of bytes (or, for 0xFF, a word's
/// worth). Returns where the output ends; `None` if it runs off memory.
fn unpack(ram: &mut [u8], mut from: usize, mut to: usize) -> Option<usize> {
    let word = |ram: &[u8], at: usize| Some(u16::from_le_bytes([*ram.get(at)?, *ram.get(at + 1)?]));
    loop {
        let length = usize::from(word(ram, from)?);
        from += 2;
        if length == 0 {
            return Some(to);
        }
        let end = from + length;
        let block = to;
        while from < end {
            let code = *ram.get(from)?;
            from += 1;
            if code < 0x80 {
                let count = usize::from(code);
                ram.copy_within(
                    from..from.checked_add(count).filter(|&e| e <= ram.len())?,
                    to,
                );
                (from, to) = (from + count, to + count);
                continue;
            }
            let count = if code == 0xFF {
                from += 2;
                usize::from(word(ram, from - 2)?)
            } else {
                usize::from(code & 0x7F)
            };
            let offset = word(ram, from)? as i16;
            from += 2;
            let source = block.checked_add_signed(isize::from(offset))?;
            // Byte by byte: a copy may overlap its own output.
            for i in 0..count {
                *ram.get_mut(to)? = *ram.get(source + i)?;
                to += 1;
            }
        }
    }
}

/// Memory in a match: the program with the sound bank, the samples (the
/// match's over the menus', which outrun them) and the match's graphics.
pub fn match_memory(intro_memory: &[u8], overlays: &Overlays) -> Vec<u8> {
    let mut ram = intro_memory.to_vec();
    for (number, address) in [
        (overlay::SOUND_BANK, overlay::SOUND_BANK_ADDRESS),
        (overlay::MENU_SAMPLES, overlay::SAMPLES_ADDRESS),
        (overlay::MATCH_SAMPLES, overlay::SAMPLES_ADDRESS),
        (overlay::MATCH_GRAPHICS, overlay::MATCH_GRAPHICS_ADDRESS),
        (overlay::PITCH, overlay::PITCH_ADDRESS),
    ] {
        load(&mut ram, address, overlays.get(number));
    }
    ram
}

/// Memory as the menus see it: the match's, with the menu graphics and the
/// menu sound samples loaded over the match's data.
pub fn menu_ram(match_memory: &[u8], overlays: &Overlays) -> Vec<u8> {
    let mut ram = match_memory.to_vec();
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
    fn unpacks_literals_and_copies_from_the_block() {
        let mut ram = vec![0; 64];
        let packed = [
            // A block of 12 bytes: "ABC"; 4 bytes from the block's start,
            // overlapping their own output; 2 from offset 1, the long way.
            12, 0, 3, b'A', b'B', b'C', 0x84, 0, 0, 0xFF, 2, 0, 1, 0, //
            // The end.
            0, 0,
        ];
        ram[32..32 + packed.len()].copy_from_slice(&packed);
        assert_eq!(unpack(&mut ram, 32, 0), Some(9));
        assert_eq!(&ram[..9], b"ABCABCABC");
    }

    #[test]
    fn loading_past_the_end_of_memory_is_cut_short() {
        let mut ram = vec![0; 4];
        load(&mut ram, 2, &[1, 2, 3]);
        assert_eq!(ram, [0, 0, 1, 2]);
    }
}

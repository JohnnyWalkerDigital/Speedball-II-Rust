//! Reading IPF disk images: the Software Preservation Society's format for
//! original floppies, protection tracks included.
//!
//! An IPF is a list of records. Each `IMGE` record describes a track, and
//! the `DATA` record with the same key holds its blocks: a descriptor per
//! block, then streams of elements (sync marks, data, gaps). On an
//! AmigaDOS track each block is one sector: a sync mark, then 540 bytes
//! that are the sector's header and data split into odd and even bits, as
//! the Amiga writes them. [`decode`] turns those back into an ADF's layout,
//! sector after sector; any other track (copy protection) is left empty.

use std::fmt;

pub const SECTOR_SIZE: usize = 512;
const SECTORS_PER_TRACK: usize = 11;
const TRACKS: usize = 160;
/// The size of the decoded image, as an ADF's.
pub const DISK_SIZE: usize = TRACKS * SECTORS_PER_TRACK * SECTOR_SIZE;

const MAGIC: &[u8] = b"CAPS";
const RECORD_HEADER: usize = 12;
const BLOCK_DESCRIPTOR: usize = 32;
/// The original CAPS encoder; the only one these images use.
const CAPS_ENCODER: u32 = 1;

const ELEMENT_END: u8 = 0;
const ELEMENT_SYNC: u8 = 1;
const ELEMENT_DATA: u8 = 2;
/// Two 0x4489 words: the mark that starts every AmigaDOS sector.
const AMIGA_SYNC: [u8; 4] = [0x44, 0x89, 0x44, 0x89];
/// An AmigaDOS sector without its clock bits: info, label, header
/// checksum, data checksum and data, each as its odd bits then its even
/// bits.
const AMIGA_BLOCK: usize = 540;
const AMIGA_FORMAT: u8 = 0xFF;

#[derive(Debug, PartialEq, Eq)]
pub enum IpfError {
    NotIpf,
    Truncated,
    /// Encoded with something other than the original CAPS encoder.
    Encoder(u32),
    /// A sector failed its checksum, or claimed to be somewhere it is not.
    BadSector {
        track: usize,
        sector: usize,
    },
}

impl fmt::Display for IpfError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IpfError::NotIpf => write!(f, "not an IPF image"),
            IpfError::Truncated => write!(f, "the IPF image is cut short"),
            IpfError::Encoder(encoder) => write!(f, "IPF encoder {encoder} is not supported"),
            IpfError::BadSector { track, sector } => {
                write!(f, "track {track} sector {sector} is damaged")
            }
        }
    }
}

impl std::error::Error for IpfError {}

/// A decoded image: the AmigaDOS sectors, and the SPS release number.
pub struct Disk {
    pub release: u32,
    pub sectors: Vec<u8>,
}

/// Decodes the AmigaDOS sectors of an IPF image.
pub fn decode(ipf: &[u8]) -> Result<Disk, IpfError> {
    if !ipf.starts_with(MAGIC) {
        return Err(IpfError::NotIpf);
    }
    let mut release = 0;
    let mut tracks = Vec::new();
    let mut data = Vec::new();
    let mut at = 0;
    while at < ipf.len() {
        let kind = slice(ipf, at, 4)?;
        let length = long(ipf, at + 4)? as usize;
        let body = at + RECORD_HEADER;
        at = at
            .checked_add(length.max(RECORD_HEADER))
            .ok_or(IpfError::Truncated)?;
        match kind {
            b"INFO" => {
                let encoder = long(ipf, body + 4)?;
                if encoder != CAPS_ENCODER {
                    return Err(IpfError::Encoder(encoder));
                }
                release = long(ipf, body + 12)?;
            }
            b"IMGE" => {
                let field = |i: usize| long(ipf, body + i * 4).map(|v| v as usize);
                tracks.push(Track {
                    cylinder: field(0)?,
                    side: field(1)?,
                    blocks: field(10)?,
                    key: field(13)?,
                });
            }
            b"DATA" => {
                // The record is followed by its blocks and streams.
                let extra = long(ipf, body)? as usize;
                let key = long(ipf, body + 12)? as usize;
                data.push((key, slice(ipf, at, extra)?));
                at += extra;
            }
            _ => {}
        }
    }
    let mut sectors = vec![0; DISK_SIZE];
    for track in tracks {
        let number = track.cylinder * 2 + track.side;
        if number >= TRACKS {
            continue;
        }
        let Some(&(_, blocks)) = data.iter().find(|(key, _)| *key == track.key) else {
            continue;
        };
        for block in 0..track.blocks {
            let descriptor = block * BLOCK_DESCRIPTOR;
            let offset = long(blocks, descriptor + 28)? as usize;
            let Some(raw) = amiga_block(blocks, offset)? else {
                // Not an AmigaDOS track.
                break;
            };
            let (sector, bytes) = amiga_sector(raw, number)?;
            let at = (number * SECTORS_PER_TRACK + sector) * SECTOR_SIZE;
            sectors[at..at + SECTOR_SIZE].copy_from_slice(&bytes);
        }
    }
    Ok(Disk { release, sectors })
}

struct Track {
    cylinder: usize,
    side: usize,
    blocks: usize,
    key: usize,
}

/// A block's data, if it is an AmigaDOS sector: one sync mark, then the
/// sector's bytes.
fn amiga_block(blocks: &[u8], mut at: usize) -> Result<Option<&[u8]>, IpfError> {
    let (mut sync, mut data) = (Vec::new(), Vec::new());
    loop {
        let head = *blocks.get(at).ok_or(IpfError::Truncated)?;
        at += 1;
        let kind = head & 0x1F;
        if kind == ELEMENT_END {
            break;
        }
        // The top three bits say how many bytes give the size.
        let width = usize::from(head >> 5);
        let size = slice(blocks, at, width)?
            .iter()
            .fold(0, |size, &b| size << 8 | usize::from(b));
        at += width;
        let bytes = slice(blocks, at, size)?;
        at += size;
        match kind {
            ELEMENT_SYNC => sync.push(bytes),
            ELEMENT_DATA => data.push(bytes),
            _ => {}
        }
    }
    Ok(match (sync.as_slice(), data.as_slice()) {
        ([mark], [bytes]) if *mark == AMIGA_SYNC && bytes.len() == AMIGA_BLOCK => Some(bytes),
        _ => None,
    })
}

/// An AmigaDOS sector's number and data, checked against its header.
fn amiga_sector(raw: &[u8], track: usize) -> Result<(usize, Vec<u8>), IpfError> {
    let info = join(&raw[0..2], &raw[2..4])[0];
    let header_sum = join(&raw[20..22], &raw[22..24])[0];
    let data_sum = join(&raw[24..26], &raw[26..28])[0];
    let [format, number, sector, _] = info.to_be_bytes();
    let sector = usize::from(sector);
    let bad = IpfError::BadSector { track, sector };
    if format != AMIGA_FORMAT
        || usize::from(number) != track
        || sector >= SECTORS_PER_TRACK
        || header_sum != checksum(&raw[0..20])
        || data_sum != checksum(&raw[28..])
    {
        return Err(bad);
    }
    let data = join(&raw[28..284], &raw[284..540])
        .into_iter()
        .flat_map(u32::to_be_bytes)
        .collect();
    Ok((sector, data))
}

/// Longs from their odd bits and even bits, 16 of each per long.
fn join(odd: &[u8], even: &[u8]) -> Vec<u32> {
    odd.chunks_exact(2)
        .zip(even.chunks_exact(2))
        .map(|(o, e)| {
            spread(u16::from_be_bytes([o[0], o[1]])) << 1 | spread(u16::from_be_bytes([e[0], e[1]]))
        })
        .collect()
}

/// The Amiga's checksum: the encoded longs XORed, clock bits masked off,
/// which leaves the data bits XORed in place.
fn checksum(raw: &[u8]) -> u32 {
    let xor = raw
        .chunks_exact(2)
        .fold(0, |x, w| x ^ u16::from_be_bytes([w[0], w[1]]));
    spread(xor)
}

/// A word's bits moved to the even bits of a long.
fn spread(word: u16) -> u32 {
    (0..16).fold(0, |long, bit| {
        long | u32::from(word >> bit & 1) << (bit * 2)
    })
}

fn slice(bytes: &[u8], at: usize, length: usize) -> Result<&[u8], IpfError> {
    bytes.get(at..at + length).ok_or(IpfError::Truncated)
}

fn long(bytes: &[u8], at: usize) -> Result<u32, IpfError> {
    let b = slice(bytes, at, 4)?;
    Ok(u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A long's odd bits and even bits, as two words each.
    fn split(long: u32) -> [u8; 4] {
        let squeeze =
            |bits: u32| (0..16).fold(0u16, |w, b| w | ((bits >> (b * 2) & 1) as u16) << b);
        let [o0, o1] = squeeze(long >> 1).to_be_bytes();
        let [e0, e1] = squeeze(long).to_be_bytes();
        [o0, o1, e0, e1]
    }

    fn sector_block(track: u8, sector: u8, fill: u8) -> Vec<u8> {
        let mut raw = vec![0; AMIGA_BLOCK];
        raw[0..4].copy_from_slice(&split(u32::from_be_bytes([
            0xFF,
            track,
            sector,
            11 - sector,
        ])));
        let data: Vec<u32> = (0..128).map(|i| u32::from(fill) << 24 | i).collect();
        for (i, long) in data.iter().enumerate() {
            let [o0, o1, e0, e1] = split(*long);
            raw[28 + i * 2..30 + i * 2].copy_from_slice(&[o0, o1]);
            raw[284 + i * 2..286 + i * 2].copy_from_slice(&[e0, e1]);
        }
        let header = split(checksum(&raw[0..20]));
        raw[20..24].copy_from_slice(&header);
        let data_sum = split(checksum(&raw[28..]));
        raw[24..28].copy_from_slice(&data_sum);
        raw
    }

    fn element(kind: u8, bytes: &[u8]) -> Vec<u8> {
        let mut out = vec![kind | 2 << 5];
        out.extend((bytes.len() as u16).to_be_bytes());
        out.extend(bytes);
        out
    }

    fn record(kind: &[u8], fields: &[u32]) -> Vec<u8> {
        let mut out = kind.to_vec();
        out.extend(((RECORD_HEADER + fields.len() * 4) as u32).to_be_bytes());
        out.extend([0; 4]);
        out.extend(fields.iter().flat_map(|f| f.to_be_bytes()));
        out
    }

    /// An image with one track (cylinder 1, side 0) of one sector.
    fn image(block: &[u8]) -> Vec<u8> {
        let mut stream = element(ELEMENT_SYNC, &AMIGA_SYNC);
        stream.extend(element(ELEMENT_DATA, block));
        stream.push(ELEMENT_END);
        let mut extra = vec![0; BLOCK_DESCRIPTOR];
        extra[28..32].copy_from_slice(&(BLOCK_DESCRIPTOR as u32).to_be_bytes());
        extra.extend(stream);
        let mut ipf = record(MAGIC, &[]);
        let mut info = [0; 24];
        info[1] = CAPS_ENCODER;
        info[3] = 180;
        ipf.extend(record(b"INFO", &info));
        let mut imge = [0; 14];
        imge[0] = 1;
        imge[10] = 1;
        imge[13] = 7;
        ipf.extend(record(b"IMGE", &imge));
        ipf.extend(record(b"DATA", &[extra.len() as u32, 0, 0, 7]));
        ipf.extend(extra);
        ipf
    }

    #[test]
    fn decodes_amigados_sectors_into_place() {
        let disk = decode(&image(&sector_block(2, 3, 0xAB))).unwrap();
        assert_eq!(disk.release, 180);
        assert_eq!(disk.sectors.len(), DISK_SIZE);
        let at = (2 * SECTORS_PER_TRACK + 3) * SECTOR_SIZE;
        assert_eq!(disk.sectors[at..at + 8], [0xAB, 0, 0, 0, 0xAB, 0, 0, 1]);
        assert_eq!(disk.sectors[at + 508..at + 512], [0xAB, 0, 0, 127]);
        assert!(disk.sectors[..at].iter().all(|&b| b == 0));
    }

    #[test]
    fn damaged_sectors_are_refused() {
        let mut block = sector_block(2, 3, 0xAB);
        block[100] ^= 1;
        assert!(matches!(
            decode(&image(&block)),
            Err(IpfError::BadSector {
                track: 2,
                sector: 3
            })
        ));
        assert!(matches!(decode(b"FORM"), Err(IpfError::NotIpf)));
    }
}

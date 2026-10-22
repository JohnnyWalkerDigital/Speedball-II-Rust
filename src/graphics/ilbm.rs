//! IFF ILBM pictures, as `unpack_iff_image` reads them.

use super::indexed::{ColorWords, IndexedImage};

/// A decoded picture and its colour map.
pub struct Picture {
    pub image: IndexedImage,
    pub palette: ColorWords,
}

#[derive(Debug, PartialEq, Eq)]
pub enum IlbmError {
    NotIff,
    MissingChunk(&'static str),
    Truncated,
}

impl std::fmt::Display for IlbmError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IlbmError::NotIff => write!(f, "not an IFF picture"),
            IlbmError::MissingChunk(name) => write!(f, "IFF picture has no {name} chunk"),
            IlbmError::Truncated => write!(f, "IFF picture is cut short"),
        }
    }
}

impl std::error::Error for IlbmError {}

/// The original only ever unpacks the first 200 rows.
pub const MAX_ROWS: usize = 200;

/// Decodes a picture, keeping at most [`MAX_ROWS`] rows.
pub fn decode(bytes: &[u8]) -> Result<Picture, IlbmError> {
    if !bytes.starts_with(b"FORM") {
        return Err(IlbmError::NotIff);
    }
    let header = chunk(bytes, b"BMHD").ok_or(IlbmError::MissingChunk("BMHD"))?;
    let cmap = chunk(bytes, b"CMAP").ok_or(IlbmError::MissingChunk("CMAP"))?;
    let body = chunk(bytes, b"BODY").ok_or(IlbmError::MissingChunk("BODY"))?;
    if header.len() < 11 {
        return Err(IlbmError::Truncated);
    }
    let width = usize::from(u16::from_be_bytes([header[0], header[1]]));
    let height = usize::from(u16::from_be_bytes([header[2], header[3]])).min(MAX_ROWS);
    let planes = usize::from(header[8]);
    let compressed = header[10] == 1;

    let row_bytes = width.div_ceil(16) * 2;
    let data = if compressed {
        unpack_byte_run(body, row_bytes * planes * height)?
    } else {
        body.to_vec()
    };
    if data.len() < row_bytes * planes * height {
        return Err(IlbmError::Truncated);
    }
    let pixels = (0..height)
        .flat_map(|y| (0..width).map(move |x| (x, y)))
        .map(|(x, y)| {
            (0..planes)
                .filter(|plane| {
                    let byte = data[(y * planes + plane) * row_bytes + x / 8];
                    byte & (0x80 >> (x % 8)) != 0
                })
                .fold(0u8, |index, plane| index | 1 << plane)
        })
        .collect();
    Ok(Picture {
        image: IndexedImage::new(width, height, pixels),
        palette: color_words(cmap),
    })
}

/// Finds a chunk's contents. Chunks are word aligned.
fn chunk<'a>(bytes: &'a [u8], name: &[u8; 4]) -> Option<&'a [u8]> {
    let mut at = 12;
    while at + 8 <= bytes.len() {
        let length = u32::from_be_bytes(bytes[at + 4..at + 8].try_into().ok()?) as usize;
        let contents = at + 8;
        if &bytes[at..at + 4] == name {
            return bytes.get(contents..(contents + length).min(bytes.len()));
        }
        at = contents + length + (length & 1);
    }
    None
}

/// `0xRR 0xGG 0xBB` becomes `0x0RGB`, keeping the top four bits of each.
fn color_words(cmap: &[u8]) -> ColorWords {
    let mut words = [0; 32];
    for (word, rgb) in words.iter_mut().zip(cmap.chunks_exact(3)) {
        *word =
            (u16::from(rgb[0] >> 4) << 8) | (u16::from(rgb[1] >> 4) << 4) | u16::from(rgb[2] >> 4);
    }
    words
}

/// ByteRun1: `n` < 128 copies the next `n + 1` bytes; `n` > 128 repeats the
/// next byte `257 - n` times; 128 does nothing.
fn unpack_byte_run(packed: &[u8], wanted: usize) -> Result<Vec<u8>, IlbmError> {
    let mut out = Vec::with_capacity(wanted);
    let mut bytes = packed.iter().copied();
    while out.len() < wanted {
        let n = bytes.next().ok_or(IlbmError::Truncated)?;
        match n {
            0..=127 => {
                for _ in 0..=n {
                    out.push(bytes.next().ok_or(IlbmError::Truncated)?);
                }
            }
            128 => {}
            _ => {
                let value = bytes.next().ok_or(IlbmError::Truncated)?;
                out.extend(std::iter::repeat(value).take(257 - usize::from(n)));
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn iff(chunks: &[(&[u8; 4], Vec<u8>)]) -> Vec<u8> {
        let mut body = b"ILBM".to_vec();
        for (name, data) in chunks {
            body.extend_from_slice(*name);
            body.extend_from_slice(&(data.len() as u32).to_be_bytes());
            body.extend_from_slice(data);
            if data.len() % 2 == 1 {
                body.push(0);
            }
        }
        let mut out = b"FORM".to_vec();
        out.extend_from_slice(&(body.len() as u32).to_be_bytes());
        out.extend(body);
        out
    }

    #[test]
    fn decodes_a_compressed_picture() {
        // 16x1, 2 planes, ByteRun1: plane 0 copied (0xFF00), plane 1 a
        // run of two 0x0F bytes.
        let header = vec![0, 16, 0, 1, 0, 0, 0, 0, 2, 0, 1, 0, 0, 0];
        let cmap = vec![0, 0, 0, 0xF0, 0x80, 0x10, 0, 0, 0, 0xFF, 0xFF, 0xFF];
        let body = vec![0x01, 0xFF, 0x00, 0xFF, 0x0F];
        let picture = decode(&iff(&[(b"BMHD", header), (b"CMAP", cmap), (b"BODY", body)])).unwrap();
        assert_eq!(picture.image.row(0)[..8], [1, 1, 1, 1, 3, 3, 3, 3]);
        assert_eq!(picture.image.row(0)[8..], [0, 0, 0, 0, 2, 2, 2, 2]);
        assert_eq!(picture.palette[1], 0x0F81);
        assert_eq!(picture.palette[3], 0x0FFF);
    }

    #[test]
    fn rejects_other_files() {
        assert_eq!(decode(b"RIFF....").err(), Some(IlbmError::NotIff));
        let missing = iff(&[(b"BMHD", vec![0; 20])]);
        assert_eq!(
            decode(&missing).err(),
            Some(IlbmError::MissingChunk("CMAP"))
        );
    }
}

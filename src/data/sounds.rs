//! The Amiga sound sample table and WAV export.
//!
//! The match RAM dump holds a table of 43 entries (14 bytes each). Each
//! gives a length in 16-bit words and an offset into the sample area. The
//! samples are raw signed 8-bit PCM. The playback rate is set by the code
//! that plays each sound, not by the table, so the export uses the common
//! Amiga rate of 8287 Hz (period 428).

use std::fs;
use std::io;
use std::path::Path;

use super::bytes::BigEndian;
use crate::sound::wav;

const TABLE: usize = 0x35F7C;
const ENTRY_SIZE: usize = 14;
const ENTRY_COUNT: usize = 43;
const SAMPLE_AREA: usize = 0x1BC3E;
pub const EXPORT_RATE: u32 = 8287;

/// One entry of the sample table.
pub struct Sample<'a> {
    /// Signed 8-bit PCM.
    pub data: &'a [u8],
}

pub fn read_samples(match_dump: &[u8]) -> Vec<Sample<'_>> {
    (0..ENTRY_COUNT)
        .map(|i| {
            let entry = TABLE + i * ENTRY_SIZE;
            let length = usize::from(match_dump.u16_at(entry + 4)) * 2;
            let offset = SAMPLE_AREA + match_dump.u32_at(entry + 6) as usize;
            Sample {
                data: &match_dump[offset..offset + length],
            }
        })
        .collect()
}

/// Writes every sample to `dir` as `sample_NN.wav`. Returns how many were written.
pub fn export_wavs(match_dump: &[u8], dir: &Path) -> io::Result<usize> {
    fs::create_dir_all(dir)?;
    let samples = read_samples(match_dump);
    for (i, sample) in samples.iter().enumerate() {
        fs::write(
            dir.join(format!("sample_{i:02}.wav")),
            wav::mono_8(sample.data, EXPORT_RATE),
        )?;
    }
    Ok(samples.len())
}

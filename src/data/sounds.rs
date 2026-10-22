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
            wav(sample.data, EXPORT_RATE),
        )?;
    }
    Ok(samples.len())
}

/// A mono 8-bit WAV file. WAV 8-bit samples are unsigned, so the signed
/// Amiga data is offset by 128.
fn wav(signed_pcm: &[u8], rate: u32) -> Vec<u8> {
    let data_len = signed_pcm.len() as u32;
    let mut out = Vec::with_capacity(44 + signed_pcm.len());
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes()); // fmt chunk size
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&1u16.to_le_bytes()); // mono
    out.extend_from_slice(&rate.to_le_bytes());
    out.extend_from_slice(&rate.to_le_bytes()); // bytes per second
    out.extend_from_slice(&1u16.to_le_bytes()); // block align
    out.extend_from_slice(&8u16.to_le_bytes()); // bits per sample
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    out.extend(signed_pcm.iter().map(|&b| b.wrapping_add(128)));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wav_header_and_sign_conversion() {
        let file = wav(&[0x00, 0x80, 0x7F], 8287);
        assert_eq!(&file[0..4], b"RIFF");
        assert_eq!(file.len(), 47);
        assert_eq!(&file[44..], &[128, 0, 255]);
    }
}

//! The game's sounds as WAV files, for listening outside the game: the
//! instruments' raw samples, and every sound played through the driver.

use std::fs;
use std::io;
use std::path::Path;

use super::driver::Driver;
use super::layout::{instrument, INSTRUMENT_SIZE, INSTRUMENT_TABLE_INDEX, SOUND_BASE, SOUND_COUNT};
use super::memory::Memory;
use super::wav;
use crate::data::bytes::BigEndian;

/// The match's instruments.
const INSTRUMENT_COUNT: usize = 43;

/// Writes each of the match's instrument samples to `dir` as
/// `sample_NN.wav`. Returns how many were written.
pub fn export_samples(match_dump: &[u8], dir: &Path) -> io::Result<usize> {
    fs::create_dir_all(dir)?;
    let base = SOUND_BASE as usize;
    let table = base + match_dump.u32_at(INSTRUMENT_TABLE_INDEX as usize) as usize;
    for i in 0..INSTRUMENT_COUNT {
        let record = table + i * INSTRUMENT_SIZE as usize;
        let length = usize::from(match_dump.u16_at(record + instrument::LENGTH as usize)) * 2;
        let start = base + match_dump.u32_at(record + instrument::SAMPLE as usize) as usize;
        fs::write(
            dir.join(format!("sample_{i:02}.wav")),
            wav::mono_8(&match_dump[start..start + length], wav::SAMPLE_RATE),
        )?;
    }
    Ok(INSTRUMENT_COUNT)
}

const RATE: u32 = 48_000;
/// Sounds that loop forever (crowd noise) are cut off here.
const MAX_SECONDS: u32 = 15;

/// Writes `sound_NN.wav` for every sound ID. Returns how many were written.
pub fn export_sounds(match_dump: Vec<u8>, dir: &Path) -> io::Result<usize> {
    fs::create_dir_all(dir)?;
    let memory = Memory::new(match_dump);
    for id in 0..SOUND_COUNT {
        let samples = render(&memory, id);
        fs::write(
            dir.join(format!("sound_{id:02}.wav")),
            wav_stereo_16(&samples, RATE),
        )?;
    }
    Ok(usize::from(SOUND_COUNT))
}

/// Plays one sound on a fresh driver until every channel has stopped.
fn render(memory: &Memory, id: u8) -> Vec<f32> {
    let mut driver = Driver::new(memory.clone(), RATE);
    driver.play(id, 0);
    let tick = vec![0.0; (RATE / 50 * 2) as usize];
    let mut out = Vec::new();
    for _ in 0..MAX_SECONDS * 50 {
        let mut chunk = tick.clone();
        driver.render(&mut chunk);
        out.extend_from_slice(&chunk);
        if driver.is_idle() {
            break;
        }
    }
    out
}

fn wav_stereo_16(samples: &[f32], rate: u32) -> Vec<u8> {
    let data_len = (samples.len() * 2) as u32;
    let mut out = Vec::with_capacity(44 + data_len as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&2u16.to_le_bytes()); // stereo
    out.extend_from_slice(&rate.to_le_bytes());
    out.extend_from_slice(&(rate * 4).to_le_bytes()); // bytes per second
    out.extend_from_slice(&4u16.to_le_bytes()); // block align
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for &sample in samples {
        let value = (sample.clamp(-1.0, 1.0) * f32::from(i16::MAX)) as i16;
        out.extend_from_slice(&value.to_le_bytes());
    }
    out
}

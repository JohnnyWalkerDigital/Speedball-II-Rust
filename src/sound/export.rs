//! Renders the game's sounds through the driver to WAV files, for listening
//! outside the game.

use std::fs;
use std::io;
use std::path::Path;

use super::driver::Driver;
use super::memory::Memory;

const RATE: u32 = 48_000;
/// Sounds that loop forever (crowd noise) are cut off here.
const MAX_SECONDS: u32 = 15;
/// Entries in the original `sound_table`.
pub const SOUND_COUNT: u8 = 56;

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

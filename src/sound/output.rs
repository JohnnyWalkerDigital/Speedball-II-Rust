//! Streams the driver's output to the computer's speakers.

use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use rodio::{OutputStream, Source};

use super::driver::Driver;
use super::memory::Memory;
use crate::game::events::Sound;

const SAMPLE_RATE: u32 = 48_000;
/// Frames rendered per lock of the driver.
const CHUNK_FRAMES: usize = 512;

pub struct SoundOutput {
    driver: Arc<Mutex<Driver>>,
    // Dropping the stream stops all sound, so it lives as long as we do.
    _stream: OutputStream,
}

impl SoundOutput {
    /// Opens the default audio device and starts the driver.
    pub fn start(match_dump: Vec<u8>) -> Result<Self, String> {
        let (stream, handle) = OutputStream::try_default().map_err(|e| e.to_string())?;
        let driver = Arc::new(Mutex::new(Driver::new(
            Memory::new(match_dump),
            SAMPLE_RATE,
        )));
        handle
            .play_raw(DriverSource::new(Arc::clone(&driver)))
            .map_err(|e| e.to_string())?;
        Ok(Self {
            driver,
            _stream: stream,
        })
    }

    pub fn play(&self, sound: Sound) {
        lock(&self.driver).play(sound.id, sound.priority);
    }
}

fn lock(driver: &Mutex<Driver>) -> MutexGuard<'_, Driver> {
    // A panic on the audio thread must not take the game down with it.
    driver
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// An endless stereo stream pulled from the driver in small chunks.
struct DriverSource {
    driver: Arc<Mutex<Driver>>,
    buffer: Vec<f32>,
    position: usize,
}

impl DriverSource {
    fn new(driver: Arc<Mutex<Driver>>) -> Self {
        Self {
            driver,
            buffer: vec![0.0; CHUNK_FRAMES * 2],
            position: CHUNK_FRAMES * 2,
        }
    }
}

impl Iterator for DriverSource {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        if self.position == self.buffer.len() {
            lock(&self.driver).render(&mut self.buffer);
            self.position = 0;
        }
        let sample = self.buffer[self.position];
        self.position += 1;
        Some(sample)
    }
}

impl Source for DriverSource {
    fn current_frame_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> u16 {
        2
    }

    fn sample_rate(&self) -> u32 {
        SAMPLE_RATE
    }

    fn total_duration(&self) -> Option<Duration> {
        None
    }
}

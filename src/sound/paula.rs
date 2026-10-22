//! Paula, the Amiga's sound chip, reduced to what the game's driver uses:
//! four DMA channels, each playing signed 8-bit samples from memory.
//!
//! The behaviours that matter:
//! - Turning DMA on (from off) restarts the channel: it latches the
//!   location and length registers and raises an interrupt as the block
//!   starts, not when it ends.
//! - Writing the registers while playing only affects the *next* block,
//!   which is latched (with another interrupt) when the current one ends.
//! - Period and volume writes take effect immediately.

use super::memory::Memory;

/// PAL colour clock divided by 2, the rate periods count in.
pub const PAL_CLOCK: f64 = 3_546_895.0;
/// Paula cannot fetch samples faster than this period.
const MIN_PERIOD: u16 = 124;
const MAX_VOLUME: u16 = 64;

/// The registers the CPU writes (`AUDxLC`, `AUDxLEN`, `AUDxPER`, `AUDxVOL`).
#[derive(Clone, Copy, Debug, Default)]
pub struct Registers {
    pub location: u32,
    /// In 16-bit words; 0 means 65536.
    pub length: u16,
    pub period: u16,
    pub volume: u16,
}

#[derive(Clone, Debug, Default)]
pub struct Voice {
    pub registers: Registers,
    /// `INTENA` bit for this channel.
    pub interrupt_enabled: bool,
    dma: bool,
    restart_pending: bool,
    block_start: u32,
    block_bytes: u32,
    position: u32,
    phase: f64,
}

impl Voice {
    fn latch(&mut self) {
        self.block_start = self.registers.location;
        let words = match self.registers.length {
            0 => 0x1_0000,
            n => u32::from(n),
        };
        self.block_bytes = words * 2;
        self.position = 0;
    }
}

pub struct Paula {
    pub voices: [Voice; 4],
    output_rate: f64,
}

impl Paula {
    pub fn new(output_rate: u32) -> Self {
        Self {
            voices: Default::default(),
            output_rate: f64::from(output_rate),
        }
    }

    /// `DMACON` for one channel.
    pub fn set_dma(&mut self, channel: usize, on: bool) {
        let voice = &mut self.voices[channel];
        if on && !voice.dma {
            voice.restart_pending = true;
        }
        if !on {
            voice.restart_pending = false;
        }
        voice.dma = on;
    }

    /// Produces one stereo frame. Returns it with a bit mask of the channels
    /// that started a new block (raising their audio interrupt).
    pub fn step(&mut self, memory: &Memory) -> ([f32; 2], u8) {
        let mut interrupts = 0;
        let mut levels = [0.0f32; 4];
        for (channel, voice) in self.voices.iter_mut().enumerate() {
            if !voice.dma {
                continue;
            }
            if voice.restart_pending {
                voice.restart_pending = false;
                voice.phase = 0.0;
                voice.latch();
                interrupts |= 1 << channel;
            }
            let sample = memory.i8(voice.block_start.wrapping_add(voice.position));
            let volume = voice.registers.volume.min(MAX_VOLUME);
            levels[channel] =
                f32::from(sample) * f32::from(volume) / (128.0 * f32::from(MAX_VOLUME));

            let period = voice.registers.period.max(MIN_PERIOD);
            voice.phase += PAL_CLOCK / f64::from(period) / self.output_rate;
            while voice.phase >= 1.0 {
                voice.phase -= 1.0;
                voice.position += 1;
                if voice.position >= voice.block_bytes {
                    voice.latch();
                    interrupts |= 1 << channel;
                }
            }
        }
        // Channels 0 and 3 are wired to the left output, 1 and 2 to the right.
        let left = (levels[0] + levels[3]) / 2.0;
        let right = (levels[1] + levels[2]) / 2.0;
        ([left, right], interrupts)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn memory() -> Memory {
        // A 4-byte "sample" at address 0x10: 127, 127, -128, -128.
        let mut bytes = vec![0u8; 0x20];
        bytes[0x10..0x14].copy_from_slice(&[0x7F, 0x7F, 0x80, 0x80]);
        Memory::new(bytes)
    }

    #[test]
    fn dma_start_latches_and_raises_an_interrupt() {
        let memory = memory();
        // One byte per output sample.
        let mut paula = Paula::new((PAL_CLOCK / 200.0) as u32);
        paula.voices[1].registers = Registers {
            location: 0x10,
            length: 2,
            period: 200,
            volume: 64,
        };
        paula.set_dma(1, true);
        let (frame, interrupts) = paula.step(&memory);
        assert_eq!(interrupts, 0b10);
        // Each side averages two channels, so one full-volume channel peaks at 0.5.
        assert!(
            frame[1] > 0.49 && frame[0] == 0.0,
            "channel 1 is on the right"
        );
        paula.step(&memory);
        paula.step(&memory);
        // The fourth byte ends the block: it reloads and interrupts again.
        let (frame, interrupts) = paula.step(&memory);
        assert!(frame[1] < -0.49);
        assert_eq!(interrupts, 0b10);
    }

    #[test]
    fn dma_off_is_silent() {
        let memory = memory();
        let mut paula = Paula::new(48_000);
        paula.voices[0].registers.volume = 64;
        paula.set_dma(0, true);
        paula.set_dma(0, false);
        assert_eq!(paula.step(&memory), ([0.0, 0.0], 0));
    }
}

//! A port of the Amiga Speedball 2 sound driver.
//!
//! The original (`sound_play` at `$13AA8` and friends) is a small tracker:
//! a *sound* names up to four *sequences*, one per Amiga channel. A sequence
//! is bytecode that picks instruments (samples) and plays notes at a pitch
//! for a duration, with envelopes, tremolo and vibrato. The driver steps
//! every channel once per vertical blank (50 Hz).
//!
//! Every routine is ported from the disassembly and named after its label;
//! every table is read from the RAM dump at its original address. 68000
//! word arithmetic is kept (wrapping `u16`), as the data relies on it.

use super::memory::Memory;
use super::paula::Paula;

// ---------------------------------------------------------------------------
// Addresses in the match RAM dump.
// ---------------------------------------------------------------------------

/// `sound_table`: 6 bytes per sound (priority, no-play flag, 4 sequences).
pub(super) const SOUND_TABLE: u32 = 0x13958;
/// `sound_effects`: 15 bytes per effect.
pub(super) const SOUND_EFFECTS: u32 = 0x13B5A;
/// `sound_envelopes`: 8 bytes per envelope.
const SOUND_ENVELOPES: u32 = 0x13CB4;
/// `sound_pitch_table_0..2`: note periods per octave, in quarter semitones.
pub(super) const PITCH_TABLES: [u32; 3] = [0x14A9A, 0x14AFA, 0x14B5A];
/// `sound_default_instrument`; also played as a tiny looping sample.
const DEFAULT_INSTRUMENT: u32 = 0x14E5A;
/// `sound_zero_sample`: 8 silent words.
const ZERO_SAMPLE: u32 = 0x14E68;
/// `sound_base`. Its first long is the offset of the sequence table.
pub(super) const SOUND_BASE: u32 = 0x1BC3E;
/// `instrument_table_index`: offset of the instrument table from `sound_base`.
pub(super) const INSTRUMENT_TABLE_INDEX: u32 = 0x1BC42;

pub(super) const INSTRUMENT_SIZE: u32 = 14;
pub(super) const EFFECT_SIZE: u32 = 15;
const ENVELOPE_SIZE: u32 = 8;
const STACK_SIZE: usize = 128;
const TICKS_PER_SECOND: u32 = 50;
/// Guards against sequences that loop without ever playing a note (the
/// original would hang).
const MAX_COMMANDS_PER_TICK: usize = 10_000;

/// One of the driver's four channel structures (`sound_channel_0..3`).
/// Field comments give the original struct offset.
#[derive(Clone)]
struct Channel {
    number: usize,
    /// 0x0e
    ints_remaining: u16,
    /// 0x10
    pitch_table: u32,
    /// 0x14: envelope address, 0 for none.
    envelope: u32,
    /// 0x18: sequence stack, growing down (pointer is an index into `stack`).
    stack: [u8; STACK_SIZE],
    stack_pointer: usize,
    /// 0x1c
    sample_start: u32,
    /// 0x24: negative means a looping sample.
    one_shot: i16,
    /// 0x26: loop start in bytes.
    loop_offset: u16,
    /// 0x28: in words.
    sample_len: u16,
    /// 0x2a
    pitch: u16,
    /// 0x2c
    hw_vol: u16,
    /// 0x2e
    needs_start: bool,
    /// 0x30
    envelope_pause: u8,
    /// 0x32
    held: bool,
    /// 0x34: 0 attack, 1 decay, 2 sustain.
    envelope_stage: u16,
    /// 0x36
    start_pointer: u32,
    /// 0x3a: 0 when the channel is idle.
    command_pointer: u32,
    /// 0x3e
    time_to_update: u16,
    /// 0x40
    duration: u16,
    /// 0x44
    vol_base: u16,
    /// 0x46
    period_base: u16,
    /// 0x48
    hw_period: u16,
    /// 0x4a, 0x4e: tremolo stage counters.
    tremolo_pause_count: [u16; 2],
    tremolo_length_count: [u16; 2],
    /// 0x52, 0x58: vibrato stage counters.
    vibrato_pause_count: [u16; 3],
    vibrato_length_count: [u16; 3],
    /// 0x5e..0x7b: the current effect, copied from `sound_effects`.
    effect: Effect,
    /// 0x86
    vol_adj: u16,
    /// 0x88
    period_adj: u16,
    /// 0x8a: bit 0 loops the tremolo, bit 1 the vibrato.
    effect_loops: u8,
    /// 0x8c. Not cleared when a new sound starts (see `play_channel`).
    transposition: u16,
}

/// Tremolo (volume) and vibrato (period) slides: each stage changes the
/// value by `rate` every `pause` ticks, `length` times.
#[derive(Clone, Copy, Default)]
struct Effect {
    tremolo_length: [u16; 2],
    vibrato_length: [u16; 3],
    tremolo_rate: [u16; 2],
    vibrato_rate: [u16; 3],
    tremolo_pause: [u16; 2],
    vibrato_pause: [u16; 3],
}

impl Channel {
    fn new(number: usize) -> Self {
        Self {
            number,
            ints_remaining: 2,
            pitch_table: 0,
            envelope: 0,
            stack: [0; STACK_SIZE],
            stack_pointer: STACK_SIZE,
            sample_start: ZERO_SAMPLE,
            one_shot: 0,
            loop_offset: 0,
            sample_len: 8,
            pitch: 0,
            hw_vol: 0,
            needs_start: false,
            envelope_pause: 0,
            held: false,
            envelope_stage: 0,
            start_pointer: 0,
            command_pointer: 0,
            time_to_update: 0,
            duration: 0,
            vol_base: 0,
            period_base: 0,
            hw_period: 0,
            tremolo_pause_count: [0; 2],
            tremolo_length_count: [0; 2],
            vibrato_pause_count: [0; 3],
            vibrato_length_count: [0; 3],
            effect: Effect::default(),
            vol_adj: 0,
            period_adj: 0,
            effect_loops: 0,
            transposition: 0,
        }
    }

    /// What `sound_play_channel` zeroes: offsets 0x10 to 0x8b. The channel
    /// number, interrupt count and transposition survive.
    fn clear(&mut self) {
        let keep = (self.number, self.ints_remaining, self.transposition);
        *self = Self::new(keep.0);
        self.ints_remaining = keep.1;
        self.transposition = keep.2;
        self.sample_start = 0;
        self.sample_len = 0;
    }

    fn push_long(&mut self, value: u32) {
        for byte in value.to_le_bytes() {
            self.push_byte(byte);
        }
    }

    fn push_word(&mut self, value: u16) {
        for byte in value.to_le_bytes() {
            self.push_byte(byte);
        }
    }

    /// Pushes in the order that leaves big-endian values in memory.
    fn push_byte(&mut self, byte: u8) {
        self.stack_pointer = self.stack_pointer.saturating_sub(1);
        self.stack[self.stack_pointer] = byte;
    }

    fn stack_word(&self, offset: usize) -> u16 {
        let i = self.stack_pointer + offset;
        u16::from_be_bytes([self.stack_byte(i), self.stack_byte(i + 1)])
    }

    fn stack_long(&self, offset: usize) -> u32 {
        (u32::from(self.stack_word(offset)) << 16) | u32::from(self.stack_word(offset + 2))
    }

    fn set_stack_word(&mut self, offset: usize, value: u16) {
        let i = self.stack_pointer + offset;
        for (k, byte) in value.to_be_bytes().into_iter().enumerate() {
            if let Some(slot) = self.stack.get_mut(i + k) {
                *slot = byte;
            }
        }
    }

    fn set_stack_long(&mut self, offset: usize, value: u32) {
        self.set_stack_word(offset, (value >> 16) as u16);
        self.set_stack_word(offset + 2, value as u16);
    }

    fn stack_byte(&self, i: usize) -> u8 {
        self.stack.get(i).copied().unwrap_or(0)
    }

    fn pop_long(&mut self) -> u32 {
        let value = self.stack_long(0);
        self.stack_pointer = (self.stack_pointer + 4).min(STACK_SIZE);
        value
    }

    fn tremolo_reset(&mut self) {
        self.tremolo_pause_count = self.effect.tremolo_pause;
        self.tremolo_length_count = self.effect.tremolo_length;
    }

    fn vibrato_reset(&mut self) {
        self.vibrato_pause_count = self.effect.vibrato_pause;
        self.vibrato_length_count = self.effect.vibrato_length;
    }
}

/// How a run of commands ended.
enum Step {
    /// Keep reading commands.
    Next,
    /// `sound_op_cont`: save state and wait `duration` ticks.
    Wait,
}

pub struct Driver {
    memory: Memory,
    paula: Paula,
    channels: [Channel; 4],
    /// `sound_priorities.sequences`: priority of what each channel plays.
    priorities: [u8; 4],
    /// `sound_tempo`: frames per quarter beat.
    tempo: u16,
    samples_per_tick: u32,
    samples_until_tick: u32,
}

impl Driver {
    /// `sound_init`.
    pub fn new(memory: Memory, output_rate: u32) -> Self {
        let mut paula = Paula::new(output_rate);
        for voice in &mut paula.voices {
            voice.interrupt_enabled = true;
            voice.registers.location = ZERO_SAMPLE;
            voice.registers.length = 8;
            voice.registers.period = 0x100;
        }
        Self {
            memory,
            paula,
            channels: std::array::from_fn(Channel::new),
            priorities: [0; 4],
            tempo: 0,
            samples_per_tick: output_rate / TICKS_PER_SECOND,
            samples_until_tick: 0,
        }
    }

    /// Swaps in different memory, as when the game loads the menu or match
    /// samples over each other. Whatever is playing carries on reading the
    /// new bytes, as it did on the Amiga.
    pub fn set_memory(&mut self, memory: Memory) {
        self.memory = memory;
    }

    /// Fills `out` with interleaved stereo samples, stepping the driver at
    /// 50 Hz of audio time like the vertical-blank interrupt.
    pub fn render(&mut self, out: &mut [f32]) {
        for frame in out.chunks_exact_mut(2) {
            if self.samples_until_tick == 0 {
                self.update_channels();
                self.samples_until_tick = self.samples_per_tick;
            }
            self.samples_until_tick -= 1;
            let (levels, interrupts) = self.paula.step(&self.memory);
            for channel in 0..4 {
                if interrupts & (1 << channel) != 0 && self.paula.voices[channel].interrupt_enabled
                {
                    self.service_channel(channel);
                }
            }
            frame.copy_from_slice(&levels);
        }
    }

    /// Whether every channel has finished its sequence.
    pub fn is_idle(&self) -> bool {
        self.channels.iter().all(|c| c.command_pointer == 0)
    }

    // -- Starting sounds -----------------------------------------------------

    /// `sound_play`: starts `sound`'s sequences unless a channel it needs
    /// (of the first three) is busy with something more important.
    pub fn play(&mut self, sound: u8, priority: u8) {
        let entry = SOUND_TABLE + u32::from(sound) * 6;
        if self.memory.u8(entry + 1) & 0x80 != 0 {
            return; // No-play flag.
        }
        let priority_value = priority & 0x7F;
        if priority != 0 {
            for channel in 0..3 {
                let wanted = self.memory.u8(entry + 2 + channel as u32) != 0;
                if wanted && priority_value < self.priorities[channel] {
                    // The original then records the sound in a table nothing
                    // reads; either way, it does not play.
                    return;
                }
            }
        }
        for channel in 0..4 {
            let sequence = self.memory.u8(entry + 2 + channel as u32);
            if sequence != 0 {
                self.priorities[channel] = priority_value;
                self.play_channel(sequence, channel);
            }
        }
    }

    /// `sound_play_channel`.
    fn play_channel(&mut self, sequence: u8, channel: usize) {
        self.channels[channel].clear();
        let voice = &mut self.paula.voices[channel];
        voice.interrupt_enabled = false;
        self.paula.set_dma(channel, false);
        let voice = &mut self.paula.voices[channel];
        voice.registers.volume = 0;
        voice.registers.location = ZERO_SAMPLE;
        voice.registers.length = 1;
        voice.registers.period = 1;
        self.paula.set_dma(channel, true);

        let start = self.sequence_address(sequence);
        let base_octave = self.memory.u32(DEFAULT_INSTRUMENT + 10);
        let ch = &mut self.channels[channel];
        ch.start_pointer = start;
        ch.command_pointer = start;
        ch.stack_pointer = STACK_SIZE;
        ch.one_shot = self.memory.u16(DEFAULT_INSTRUMENT) as i16;
        ch.loop_offset = self.memory.u16(DEFAULT_INSTRUMENT + 2);
        ch.sample_len = self.memory.u16(DEFAULT_INSTRUMENT + 4);
        ch.sample_start = ZERO_SAMPLE;
        ch.pitch_table = pitch_table(base_octave as u8);
        ch.period_base = 0x100;
        ch.hw_period = 0x100;
        let (start, length) = (ch.sample_start, ch.sample_len);
        let registers = &mut self.paula.voices[channel].registers;
        registers.location = start;
        registers.length = length;
        registers.period = 0x100;
    }

    fn sequence_address(&self, sequence: u8) -> u32 {
        let table = SOUND_BASE.wrapping_add(self.memory.u32(SOUND_BASE));
        let offset = self.memory.u32(table + u32::from(sequence) * 4);
        SOUND_BASE.wrapping_add(offset)
    }

    // -- Interrupts ------------------------------------------------------------

    /// `sound_service_channel`: called as a sample block starts playing.
    fn service_channel(&mut self, channel: usize) {
        let ch = &mut self.channels[channel];
        ch.ints_remaining = ch.ints_remaining.wrapping_sub(1);
        let voice = &mut self.paula.voices[channel];
        if ch.one_shot < 0 {
            // Looping: after the first pass, loop from the loop offset.
            voice.registers.location = ch.sample_start.wrapping_add(u32::from(ch.loop_offset));
            voice.registers.length = ch.sample_len.wrapping_sub(ch.loop_offset >> 1);
            voice.interrupt_enabled = false;
        } else if ch.ints_remaining == 0 {
            // One-shot, second interrupt: the silence has started; stop.
            voice.interrupt_enabled = false;
            self.paula.set_dma(channel, false);
        } else {
            // One-shot, first interrupt: queue silence after the sample.
            voice.registers.location = ZERO_SAMPLE;
            voice.registers.length = 1;
        }
    }

    // -- The 50 Hz update --------------------------------------------------------

    /// `sound_update_channels`.
    fn update_channels(&mut self) {
        for channel in 0..4 {
            if self.channels[channel].command_pointer == 0 {
                continue;
            }
            if self.channels[channel].time_to_update == 0 {
                if self.channels[channel].loop_offset == 0 {
                    self.cut_sample(channel);
                }
                self.run_commands(channel);
            }
            self.apply_modulation(channel);
        }
        for channel in 0..4 {
            self.envelope_channel(channel);
        }
        for channel in 0..4 {
            self.update_hardware_channel(channel);
        }
    }

    /// At a note boundary, samples without a loop point are stopped by
    /// pointing the channel at the default instrument (the original's
    /// comment suspects this should have been the zero sample).
    fn cut_sample(&mut self, channel: usize) {
        self.paula.set_dma(channel, false);
        let voice = &mut self.paula.voices[channel];
        voice.interrupt_enabled = false;
        voice.registers.volume = 0;
        voice.registers.location = DEFAULT_INSTRUMENT;
        voice.registers.length = 1;
        voice.registers.period = 1;
        self.paula.set_dma(channel, true);
    }

    /// `sound_next_command` loop.
    fn run_commands(&mut self, channel: usize) {
        let mut pc = self.channels[channel].command_pointer;
        for _ in 0..MAX_COMMANDS_PER_TICK {
            let command = self.memory.u8(pc);
            pc = pc.wrapping_add(1);
            let step = if command < 0x80 {
                self.start_note(channel, command);
                Step::Wait
            } else {
                self.run_op(channel, command, &mut pc)
            };
            if let Step::Wait = step {
                let ch = &mut self.channels[channel];
                ch.command_pointer = pc;
                ch.time_to_update = ch.duration;
                return;
            }
        }
        // A sequence that never yields: silence the channel.
        self.op_stop(channel, &mut pc);
        self.channels[channel].command_pointer = 0;
    }

    /// Plays note `note` (in semitones) on the current instrument.
    fn start_note(&mut self, channel: usize, note: u8) {
        self.paula.set_dma(channel, false);
        let voice = &mut self.paula.voices[channel];
        voice.interrupt_enabled = false;
        voice.registers.volume = 0;
        voice.registers.location = DEFAULT_INSTRUMENT;
        voice.registers.length = 1;
        voice.registers.period = 1;

        let ch = &mut self.channels[channel];
        ch.pitch = u16::from(note);
        if ch.envelope != 0 {
            ch.held = true;
            ch.envelope_stage = 0;
            ch.hw_vol = 0;
            ch.envelope_pause = 0;
        }
        // Pitch tables are in quarter semitones; transposition is added there.
        let index = (u16::from(note) * 4).wrapping_add(ch.transposition);
        let offset = i32::from(index.wrapping_mul(2) as i16);
        ch.period_base = self.memory.u16(ch.pitch_table.wrapping_add_signed(offset));
        ch.needs_start = true;
        ch.tremolo_reset();
        ch.vol_adj = 0;
        ch.vibrato_reset();
        ch.period_adj = 0;
    }

    /// Dispatches a command byte `0x80..` (`sound_switch`).
    fn run_op(&mut self, channel: usize, command: u8, pc: &mut u32) -> Step {
        let memory = self.memory.clone();
        let operand = |pc: &mut u32| next_byte(&memory, pc);
        match command {
            // sound_op_set_vol
            0x80 => self.channels[channel].vol_base = u16::from(operand(pc)),
            // sound_op_noop_84, _a0, _a4: unimplemented, one operand.
            0x84 | 0xA0 | 0xA4 => {
                operand(pc);
            }
            // sound_op_goto_start
            0x88 => {
                let ch = &mut self.channels[channel];
                *pc = ch.start_pointer;
                ch.command_pointer = *pc;
            }
            // sound_op_set_note_len: in quarter beats.
            0x8C => {
                let beats = u32::from(operand(pc));
                self.channels[channel].duration = (beats * u32::from(self.tempo)) as u16;
            }
            // sound_op_cont: a rest; the current note carries on.
            0x90 => return Step::Wait,
            // sound_op_set_tempo: beats per minute, stored as frames per beat.
            0x94 => {
                let bpm = u16::from(operand(pc));
                // (A zero would be a division-by-zero trap on the 68000.)
                if bpm != 0 {
                    self.tempo = 750 / bpm;
                }
            }
            // sound_op_silence: sum the next n bytes into the next duration.
            0x98 => {
                let count = match operand(pc) {
                    0 => 0x1_0000,
                    n => u32::from(n),
                };
                let total =
                    (0..count).fold(0u16, |sum, _| sum.wrapping_add(u16::from(operand(pc))));
                self.channels[channel].duration = (u32::from(total) * u32::from(self.tempo)) as u16;
            }
            // sound_op_effect
            0x9C => {
                let address = SOUND_EFFECTS + u32::from(operand(pc)) * EFFECT_SIZE;
                let byte = |i: u32| memory.i8(address + i) as i16 as u16;
                let ch = &mut self.channels[channel];
                ch.effect_loops &= 0xFC;
                ch.effect = Effect {
                    tremolo_length: [byte(0), byte(1)],
                    vibrato_length: [byte(2), byte(3), byte(4)],
                    tremolo_rate: [byte(5), byte(6)],
                    vibrato_rate: [byte(7), byte(8), byte(9)],
                    tremolo_pause: [byte(10), byte(11)],
                    vibrato_pause: [byte(12), byte(13), byte(14)],
                };
                // The length counters are left alone; the next note resets them.
                ch.tremolo_pause_count = [0; 2];
                ch.vibrato_pause_count = [0; 3];
                ch.vol_adj = 0;
                ch.period_adj = 0;
            }
            // sound_op_set_loop_flags
            0xA8 => self.channels[channel].effect_loops |= operand(pc),
            // sound_op_stop
            0xAC => {
                self.op_stop(channel, pc);
                return Step::Wait;
            }
            // sound_op_call
            0xB0 => {
                let sequence = operand(pc);
                self.channels[channel].push_long(*pc);
                *pc = self.sequence_address(sequence);
            }
            // sound_op_return
            0xB4 => *pc = self.channels[channel].pop_long(),
            // sound_op_add_transposition (one semitone = 4); 0 resets it.
            0xB8 => {
                let ch = &mut self.channels[channel];
                match operand(pc) {
                    0 => ch.transposition = 0,
                    shift => ch.transposition = ch.transposition.wrapping_add(shift as i8 as u16),
                }
            }
            // sound_op_set_transposition
            0xBC => self.channels[channel].transposition = operand(pc) as i8 as u16,
            // sound_op_for: push the loop start and count (or zeros).
            0xC0 => {
                let count = operand(pc);
                let ch = &mut self.channels[channel];
                if count != 0 {
                    ch.push_long(*pc);
                    ch.push_word(u16::from(count));
                } else {
                    ch.push_long(0);
                    ch.push_word(0);
                }
            }
            // sound_op_next. The body runs count + 1 times: the final
            // decrement still jumps back once before the frame is dropped.
            0xC4 => {
                let ch = &mut self.channels[channel];
                let start = ch.stack_long(2);
                if start == 0 {
                    ch.stack_pointer = (ch.stack_pointer + 6).min(STACK_SIZE);
                } else {
                    *pc = start;
                    let count = ch.stack_word(0).wrapping_sub(1);
                    ch.set_stack_word(0, count);
                    if count == 0 {
                        ch.set_stack_long(2, 0);
                    }
                }
            }
            // sound_op_set_envelope
            0xC8 => {
                let envelope = SOUND_ENVELOPES + u32::from(operand(pc)) * ENVELOPE_SIZE;
                self.channels[channel].envelope = envelope;
            }
            // sound_op_clear_envelope
            0xCC => self.channels[channel].envelope = 0,
            // sound_op_set_instrument
            0xD0 => {
                let address = SOUND_BASE
                    .wrapping_add(memory.u32(INSTRUMENT_TABLE_INDEX))
                    .wrapping_add(u32::from(operand(pc)) * INSTRUMENT_SIZE);
                let ch = &mut self.channels[channel];
                ch.one_shot = memory.u16(address) as i16;
                ch.loop_offset = memory.u16(address + 2);
                ch.sample_len = memory.u16(address + 4);
                ch.sample_start = SOUND_BASE.wrapping_add(memory.u32(address + 6));
                ch.pitch_table = pitch_table(memory.u32(address + 10) as u8);
            }
            // sound_op_jump
            0xD4 => {
                let sequence = operand(pc);
                *pc = self.sequence_address(sequence);
            }
            // Bytes the jump table has no entry for: the original would jump
            // into unrelated code. Stop the channel instead.
            _ => {
                self.op_stop(channel, pc);
                return Step::Wait;
            }
        }
        Step::Next
    }

    /// `sound_op_stop`: frees the channel and silences it.
    fn op_stop(&mut self, channel: usize, pc: &mut u32) {
        let ch = &mut self.channels[channel];
        ch.time_to_update = 0;
        self.priorities[ch.number] = 0;
        *pc = 0;
        ch.start_pointer = 0;
        ch.needs_start = false;
        self.paula.voices[channel].interrupt_enabled = false;
        self.paula.set_dma(channel, false);
    }

    /// The per-tick part of `sound_update_channels`: count down the note,
    /// step tremolo and vibrato, and work out the hardware values.
    fn apply_modulation(&mut self, channel: usize) {
        let ch = &mut self.channels[channel];
        ch.time_to_update = ch.time_to_update.wrapping_sub(1);

        // Tremolo is skipped when an envelope controls the volume.
        if ch.envelope == 0 {
            let mut applied = false;
            for i in 0..2 {
                if ch.tremolo_pause_count[i] != 0 {
                    ch.tremolo_pause_count[i] -= 1;
                    applied = true;
                    break;
                }
                if ch.tremolo_length_count[i] != 0 {
                    ch.tremolo_length_count[i] -= 1;
                    ch.vol_adj = ch.vol_adj.wrapping_add(ch.effect.tremolo_rate[i]);
                    ch.tremolo_pause_count[i] = ch.effect.tremolo_pause[i];
                    applied = true;
                    break;
                }
            }
            if !applied {
                ch.vol_adj = 0;
                if ch.effect_loops & 1 != 0 {
                    ch.tremolo_reset();
                }
            }
        }

        let mut applied = false;
        for i in 0..3 {
            if ch.vibrato_pause_count[i] != 0 {
                ch.vibrato_pause_count[i] -= 1;
                applied = true;
                break;
            }
            if ch.vibrato_length_count[i] != 0 {
                ch.vibrato_length_count[i] -= 1;
                ch.period_adj = ch.period_adj.wrapping_add(ch.effect.vibrato_rate[i]);
                ch.vibrato_pause_count[i] = ch.effect.vibrato_pause[i];
                applied = true;
                break;
            }
        }
        // Unlike tremolo, the period adjustment is not cleared at the end.
        if !applied && ch.effect_loops & 2 != 0 {
            ch.vibrato_reset();
        }

        if ch.envelope == 0 {
            ch.hw_vol = ch.vol_base.wrapping_add(ch.vol_adj) & 0x3F;
        }
        ch.hw_period = ch.period_base.wrapping_add(ch.period_adj);
    }

    /// `sound_envelope_channel`.
    fn envelope_channel(&mut self, channel: usize) {
        let ch = &mut self.channels[channel];
        if ch.command_pointer == 0 || ch.envelope == 0 {
            return;
        }
        if ch.envelope_pause != 0 {
            ch.envelope_pause -= 1;
            if ch.envelope_pause != 0 {
                return;
            }
        }
        let envelope = Envelope::read(&self.memory, ch.envelope);
        envelope.step(ch);
    }

    /// `sound_update_hardware_channel`: runs for every channel each tick.
    fn update_hardware_channel(&mut self, channel: usize) {
        let ch = &mut self.channels[channel];
        let voice = &mut self.paula.voices[channel];
        voice.registers.period = ch.hw_period;
        voice.registers.volume = ch.hw_vol;
        if !ch.needs_start {
            return;
        }
        ch.needs_start = false;
        voice.registers.location = ch.sample_start;
        voice.registers.length = ch.sample_len;
        ch.ints_remaining = 2;
        self.paula.set_dma(channel, true);
        let voice = &mut self.paula.voices[channel];
        if ch.one_shot < 0 {
            if ch.loop_offset == 0 {
                // Loops whole; no interrupt needed.
                voice.interrupt_enabled = false;
                return;
            }
            ch.ints_remaining = 1;
        }
        voice.interrupt_enabled = true;
    }
}

fn next_byte(memory: &Memory, pc: &mut u32) -> u8 {
    let value = memory.u8(*pc);
    *pc = pc.wrapping_add(1);
    value
}

/// `sound_init_pitch_table`.
fn pitch_table(base_octave: u8) -> u32 {
    PITCH_TABLES[usize::from(base_octave.min(2))]
}

/// One entry of `sound_envelopes`.
struct Envelope {
    attack_rate: u8,
    attack_pause: u8,
    decay_rate: u8,
    decay_pause: u8,
    sustain_level: u8,
    release_time: u8,
    release_rate: u8,
    release_pause: u8,
}

impl Envelope {
    fn read(memory: &Memory, address: u32) -> Self {
        let byte = |i: u32| memory.u8(address + i);
        Self {
            attack_rate: byte(0),
            attack_pause: byte(1),
            decay_rate: byte(2),
            decay_pause: byte(3),
            sustain_level: byte(4),
            release_time: byte(5),
            release_rate: byte(6),
            release_pause: byte(7),
        }
    }

    /// `sound_envelope_channel_aux`. Arithmetic is on the volume's low byte,
    /// as in the original.
    fn step(&self, ch: &mut Channel) {
        let with_low_byte = |word: u16, low: u8| (word & 0xFF00) | u16::from(low);
        if !ch.held {
            // Release.
            ch.envelope_stage = 0;
            if ch.hw_vol == 0 {
                return;
            }
            ch.envelope_pause = self.release_pause;
            let (low, borrow) = (ch.hw_vol as u8).overflowing_sub(self.release_rate);
            ch.hw_vol = if borrow {
                0
            } else {
                with_low_byte(ch.hw_vol, low)
            };
            return;
        }
        match ch.envelope_stage {
            0 => {
                // Attack.
                ch.envelope_pause = self.attack_pause;
                let mut volume =
                    with_low_byte(ch.hw_vol, (ch.hw_vol as u8).wrapping_add(self.attack_rate));
                if volume >= ch.vol_base {
                    ch.envelope_stage += 1;
                    volume = ch.vol_base;
                }
                ch.hw_vol = volume;
            }
            1 => {
                // Decay.
                ch.envelope_pause = self.decay_pause;
                let mut volume =
                    with_low_byte(ch.hw_vol, (ch.hw_vol as u8).wrapping_sub(self.decay_rate));
                if volume as u8 >= 0x40 {
                    volume = 0;
                }
                if volume as u8 >= self.sustain_level {
                    ch.hw_vol = volume;
                } else {
                    ch.envelope_stage += 1;
                    self.sustain(ch);
                }
            }
            _ => self.sustain(ch),
        }
    }

    fn sustain(&self, ch: &mut Channel) {
        ch.hw_vol = u16::from(self.sustain_level);
        // A signed byte comparison, so a release time of 0xFF means "never".
        if (ch.time_to_update as u8 as i8) <= self.release_time as i8 {
            ch.held = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: u32 = 48_000;

    fn real_driver() -> Option<Driver> {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/Resources/Amiga.dmp");
        let bytes = std::fs::read(path).ok()?;
        Some(Driver::new(Memory::new(bytes), RATE))
    }

    fn ticks(driver: &mut Driver, count: u32) -> Vec<f32> {
        let mut out = vec![0.0; (count * RATE / TICKS_PER_SECOND * 2) as usize];
        driver.render(&mut out);
        out
    }

    fn instrument_start(driver: &Driver, instrument: u32) -> u32 {
        let address =
            SOUND_BASE + driver.memory.u32(INSTRUMENT_TABLE_INDEX) + instrument * INSTRUMENT_SIZE;
        SOUND_BASE + driver.memory.u32(address + 6)
    }

    /// "Get ready" (sound 46) plays instrument 23 ("get") then instrument 32
    /// ("ready") on channel 1, matching the samples identified by ear.
    #[test]
    fn get_ready_plays_get_then_ready() {
        let Some(mut driver) = real_driver() else {
            return;
        };
        let (get, ready) = (instrument_start(&driver, 23), instrument_start(&driver, 32));
        driver.play(46, 0);
        let mut seen = Vec::new();
        for _ in 0..200 {
            ticks(&mut driver, 1);
            let start = driver.channels[1].sample_start;
            if seen.last() != Some(&start) {
                seen.push(start);
            }
        }
        let get_at = seen.iter().position(|&s| s == get).expect("get played");
        let ready_at = seen.iter().position(|&s| s == ready).expect("ready played");
        assert!(get_at < ready_at);
    }

    #[test]
    fn every_sound_plays_without_panicking() {
        let Some(mut driver) = real_driver() else {
            return;
        };
        for sound in 0..56 {
            driver.play(sound, 0);
            let out = ticks(&mut driver, 150);
            assert!(
                out.iter().all(|s| s.is_finite() && s.abs() <= 1.0),
                "sound {sound}"
            );
        }
    }

    #[test]
    fn goal_is_audible_then_finishes() {
        let Some(mut driver) = real_driver() else {
            return;
        };
        driver.play(32, 8);
        let loud = ticks(&mut driver, 100);
        assert!(
            loud.iter().any(|s| s.abs() > 0.05),
            "the goal sound makes noise"
        );
        let later = ticks(&mut driver, 1_000);
        let tail = &later[later.len() - 9_600..];
        assert!(tail.iter().all(|s| s.abs() < 1e-6), "and stops");
        assert!(driver.channels.iter().all(|c| c.command_pointer == 0));
    }

    #[test]
    fn busy_channels_block_lower_priorities() {
        let Some(mut driver) = real_driver() else {
            return;
        };
        driver.play(32, 8); // Uses channels 1 and 2.
        ticks(&mut driver, 1);
        let playing = driver.channels[1].command_pointer;
        driver.play(46, 3); // Also channels 1 and 2, lower priority.
        assert_eq!(
            driver.channels[1].start_pointer,
            driver.sequence_address(0x37)
        );
        assert_ne!(driver.channels[1].command_pointer, 0);
        assert_eq!(driver.channels[1].command_pointer, playing, "not restarted");
        driver.play(46, 0); // Priority 0 always plays.
        assert_eq!(
            driver.channels[1].start_pointer,
            driver.sequence_address(0x16)
        );
    }

    // -- Opcode semantics on hand-built data -------------------------------

    /// A memory image with one sound (ID 1) whose channel 0 runs `script`
    /// as sequence 1. Instrument 0 is a looping 4-byte sample.
    fn synthetic(script: &[u8]) -> Driver {
        let mut bytes = vec![0u8; 0x40000];
        let put_u32 = |bytes: &mut Vec<u8>, at: u32, v: u32| {
            bytes[at as usize..at as usize + 4].copy_from_slice(&v.to_be_bytes())
        };
        // Sound 1: channel 0 plays sequence 1.
        bytes[(SOUND_TABLE + 6 + 2) as usize] = 1;
        // Sequence table at sound_base + 0x100; sequence 1 at + 0x200.
        put_u32(&mut bytes, SOUND_BASE, 0x100);
        put_u32(&mut bytes, SOUND_BASE + 0x100 + 4, 0x200);
        let script_at = (SOUND_BASE + 0x200) as usize;
        bytes[script_at..script_at + script.len()].copy_from_slice(script);
        // Instrument table at sound_base + 0x400.
        put_u32(&mut bytes, INSTRUMENT_TABLE_INDEX, 0x400);
        let instrument = (SOUND_BASE + 0x400) as usize;
        bytes[instrument..instrument + 6].copy_from_slice(&[0xFF, 0xFF, 0, 0, 0, 2]);
        put_u32(&mut bytes, SOUND_BASE + 0x400 + 6, 0x800);
        put_u32(&mut bytes, SOUND_BASE + 0x400 + 10, 2); // Base octave.
                                                         // Pitch tables: period = 300 + index.
        for (t, &table) in PITCH_TABLES.iter().enumerate() {
            for i in 0..48u32 {
                let at = (table + i * 2) as usize;
                let value = 300 + i as u16 + t as u16 * 1000;
                bytes[at..at + 2].copy_from_slice(&value.to_be_bytes());
            }
        }
        Driver::new(Memory::new(bytes), RATE)
    }

    #[test]
    fn for_next_runs_the_body_count_plus_one_times() {
        // tempo 255 bpm (750 / 255 = 2 frames per beat), note length 1 beat,
        // for 2 { note 5 } next, stop.
        let mut driver = synthetic(&[0x94, 0xFF, 0x8C, 0x01, 0xC0, 0x02, 0x05, 0xC4, 0xAC]);
        driver.play(1, 0);
        // Each note lasts 2 ticks, starting on ticks 1, 3, 5; the stop is
        // reached on tick 7. Running the body only twice would stop on tick 5.
        for tick in 1..=6 {
            driver.update_channels();
            assert_ne!(
                driver.channels[0].command_pointer, 0,
                "still running at tick {tick}"
            );
        }
        driver.update_channels();
        assert_eq!(driver.channels[0].command_pointer, 0, "stopped at tick 7");
    }

    #[test]
    fn notes_use_the_pitch_table_with_transposition() {
        // instrument 0, transpose +1 semitone (4 quarter steps), note 2, stop
        let mut driver = synthetic(&[0x94, 0xFF, 0x8C, 0x01, 0xD0, 0x00, 0xBC, 0x04, 0x02, 0xAC]);
        driver.play(1, 0);
        driver.update_channels();
        let channel = &driver.channels[0];
        // Base octave 2 picks the third table: 300 + 2000 + (2 * 4 + 4).
        assert_eq!(channel.period_base, 2312);
        assert_eq!(channel.sample_start, SOUND_BASE + 0x800);
    }

    #[test]
    fn call_and_return_resume_after_the_call() {
        // Sequence 1: call 2, note, stop. Sequence 2: note, return.
        let mut driver = synthetic(&[0x94, 0xFF, 0x8C, 0x01, 0xB0, 0x02, 0x05, 0xAC]);
        let mut bytes: Vec<u8> = (0..0x40000).map(|a| driver.memory.u8(a)).collect();
        let put = |bytes: &mut Vec<u8>, at: u32, data: &[u8]| {
            bytes[at as usize..at as usize + data.len()].copy_from_slice(data)
        };
        put(&mut bytes, SOUND_BASE + 0x100 + 8, &0x300u32.to_be_bytes());
        put(&mut bytes, SOUND_BASE + 0x300, &[0x07, 0xB4]);
        driver = Driver::new(Memory::new(bytes), RATE);
        driver.play(1, 0);
        driver.update_channels();
        assert_eq!(
            driver.channels[0].pitch, 7,
            "the called sequence plays first"
        );
        driver.update_channels();
        driver.update_channels();
        assert_eq!(driver.channels[0].pitch, 5, "then the caller continues");
    }
}

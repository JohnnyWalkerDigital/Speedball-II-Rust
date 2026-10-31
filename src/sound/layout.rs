//! Where the sound driver's tables are in memory, and how their records
//! are laid out. The driver reads them; [`super::data`] reads and writes
//! them.

/// The sound table: per sound, a priority, the no-play flag and the
/// sequence each of the four channels plays.
pub const SOUND_TABLE: u32 = 0x13958;
pub const SOUND_ENTRY_SIZE: u32 = 6;
/// Entries in the sound table.
pub const SOUND_COUNT: u8 = 56;

/// A sound table entry's fields.
pub mod sound_entry {
    pub const FLAGS: u32 = 1;
    /// Four bytes, one per channel.
    pub const SEQUENCES: u32 = 2;
    /// The no-play flag, in [`FLAGS`].
    pub const SILENT: u8 = 0x80;
}

/// The effect table: signed bytes per effect.
pub const SOUND_EFFECTS: u32 = 0x13B5A;
pub const EFFECT_SIZE: u32 = 15;
/// Entries in the effect table.
pub const EFFECTS: usize = 23;

/// An effect's fields: two tremolo stages and three vibrato stages, each
/// field holding one byte per stage.
pub mod effect {
    pub const TREMOLO_LENGTH: u32 = 0;
    pub const VIBRATO_LENGTH: u32 = 2;
    pub const TREMOLO_RATE: u32 = 5;
    pub const VIBRATO_RATE: u32 = 7;
    pub const TREMOLO_PAUSE: u32 = 10;
    pub const VIBRATO_PAUSE: u32 = 12;
}

/// The envelope table: 8 bytes per envelope.
pub const SOUND_ENVELOPES: u32 = 0x13CB4;
pub const ENVELOPE_SIZE: u32 = 8;

/// The three pitch tables: note periods per octave, in quarter semitones.
pub const PITCH_TABLES: [u32; 3] = [0x14A9A, 0x14AFA, 0x14B5A];
/// Words in each pitch table.
pub const PITCHES: usize = 48;

/// The default instrument; also played as a tiny looping sample.
pub const DEFAULT_INSTRUMENT: u32 = 0x14E5A;
/// The zero sample: 8 silent words.
pub const ZERO_SAMPLE: u32 = 0x14E68;

/// The start of the sound bank. Its first long is the offset of the
/// sequence table.
pub const SOUND_BASE: u32 = 0x1BC3E;
/// Holds the offset of the instrument table from [`SOUND_BASE`].
pub const INSTRUMENT_TABLE_INDEX: u32 = 0x1BC42;
pub const INSTRUMENT_SIZE: u32 = 14;

/// An instrument's fields (the default instrument has the same layout).
pub mod instrument {
    /// A word, negative for a looping sample.
    pub const ONE_SHOT: u32 = 0;
    /// A word: where the loop starts, in bytes.
    pub const LOOP_START: u32 = 2;
    /// A word: the sample's length in words.
    pub const LENGTH: u32 = 4;
    /// A long: the sample's offset from `SOUND_BASE`.
    pub const SAMPLE: u32 = 6;
    /// A long: the octave its notes are pitched from.
    pub const OCTAVE: u32 = 10;
}

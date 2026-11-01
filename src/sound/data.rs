//! What the sound driver plays, read out of a memory image and laid back
//! into one.
//!
//! The driver's own tables ([`SoundTables`]: which sequences each sound
//! starts, the tremolo and vibrato effects, the note pitches) are the same
//! in every image. Each image also holds a bank of sound data
//! ([`SoundData`]) from `SOUND_BASE`: a header, the sequences, the table of
//! sequences, the samples and the table of instruments, one after the
//! other. The match and the menus share their sequences and instruments
//! but load different samples; the intro has its own bank.
//!
//! [`SoundData::lay_out`] writes the bank back in that order, over a copy
//! of the original image. Unedited, every byte lands where it was. Edited,
//! what follows moves along, as when the original data was built; reads
//! past the end of a sample or a table still find what the original
//! memory holds there.

use serde::{Deserialize, Serialize};

use super::layout::{
    effect, instrument, sound_entry, EFFECT_SIZE, INSTRUMENT_SIZE, INSTRUMENT_TABLE_INDEX,
    PITCH_TABLES, SOUND_BASE, SOUND_COUNT, SOUND_EFFECTS, SOUND_ENTRY_SIZE, SOUND_TABLE,
};
pub use super::layout::{EFFECTS, PITCHES};
use super::sequences::Sequence;
use crate::data::bytes::BigEndian;

/// A layout constant as an index into an image.
const fn idx(value: u32) -> usize {
    value as usize
}

/// The address a pointer at `at` points to: pointers are offsets from the
/// start of the sound bank.
fn pointer(image: &[u8], at: usize) -> usize {
    idx(SOUND_BASE) + image.u32_at(at) as usize
}

/// The pointer to `address`.
fn offset_of(address: usize) -> u32 {
    (address - idx(SOUND_BASE)) as u32
}

/// The driver's tables, shared by every bank.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SoundTables {
    /// By sound number: the sequence each channel plays, 0 for none.
    pub sounds: Vec<[u8; 4]>,
    /// Sounds marked never to play.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub silent: Vec<u8>,
    pub effects: Vec<Effect>,
    /// Note periods (lower is higher), from each octave an instrument can
    /// start on. A note past the end reads on into the next.
    pub pitch_tables: Vec<Vec<u16>>,
}

/// Tremolo (volume) and vibrato (pitch) slides: each stage changes the
/// value by `rate` every `pause` frames, `length` times.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Effect {
    pub tremolo: Tremolo,
    pub vibrato: Vibrato,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tremolo {
    pub length: [i8; 2],
    pub rate: [i8; 2],
    pub pause: [i8; 2],
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vibrato {
    pub length: [i8; 3],
    pub rate: [i8; 3],
    pub pause: [i8; 3],
}

impl SoundTables {
    pub fn read(image: &[u8]) -> Self {
        let sounds = (0..usize::from(SOUND_COUNT))
            .map(|i| idx(SOUND_TABLE) + i * idx(SOUND_ENTRY_SIZE))
            .collect::<Vec<_>>();
        let effects = (0..EFFECTS)
            .map(|i| {
                let record = idx(SOUND_EFFECTS) + i * idx(EFFECT_SIZE);
                let field = |offset: u32| record + idx(offset);
                Effect {
                    tremolo: Tremolo {
                        length: signed_bytes(image, field(effect::TREMOLO_LENGTH)),
                        rate: signed_bytes(image, field(effect::TREMOLO_RATE)),
                        pause: signed_bytes(image, field(effect::TREMOLO_PAUSE)),
                    },
                    vibrato: Vibrato {
                        length: signed_bytes(image, field(effect::VIBRATO_LENGTH)),
                        rate: signed_bytes(image, field(effect::VIBRATO_RATE)),
                        pause: signed_bytes(image, field(effect::VIBRATO_PAUSE)),
                    },
                }
            })
            .collect();
        Self {
            sounds: sounds
                .iter()
                .map(|&entry| [0, 1, 2, 3].map(|c| image[entry + idx(sound_entry::SEQUENCES) + c]))
                .collect(),
            silent: (0..SOUND_COUNT)
                .filter(|&i| {
                    image[sounds[usize::from(i)] + idx(sound_entry::FLAGS)] & sound_entry::SILENT
                        != 0
                })
                .collect(),
            effects,
            pitch_tables: PITCH_TABLES
                .iter()
                .map(|&table| {
                    (0..PITCHES)
                        .map(|i| image.u16_at(idx(table) + i * 2))
                        .collect()
                })
                .collect(),
        }
    }

    pub fn check(&self) -> Result<(), String> {
        let mut problems = Vec::new();
        if self.sounds.len() != usize::from(SOUND_COUNT) {
            problems.push(format!(
                "sounds: {} sounds, not {SOUND_COUNT}",
                self.sounds.len()
            ));
        }
        if let Some(&n) = self.silent.iter().find(|&&n| n >= SOUND_COUNT) {
            problems.push(format!("silent: there is no sound {n}"));
        }
        if self.effects.len() != EFFECTS {
            problems.push(format!(
                "effects: {} effects, not {EFFECTS}",
                self.effects.len()
            ));
        }
        if self.pitch_tables.len() != PITCH_TABLES.len()
            || self.pitch_tables.iter().any(|t| t.len() != PITCHES)
        {
            problems.push(format!(
                "pitch_tables: must be {} tables of {PITCHES}",
                PITCH_TABLES.len()
            ));
        }
        crate::data::report(problems)
    }

    /// Writes the tables in place. A sound's first byte (a priority the
    /// driver never reads) is left as it was.
    fn write(&self, image: &mut [u8]) {
        for (i, channels) in self.sounds.iter().enumerate() {
            let entry = idx(SOUND_TABLE) + i * idx(SOUND_ENTRY_SIZE);
            let flags = entry + idx(sound_entry::FLAGS);
            let silent = self.silent.contains(&(i as u8));
            image[flags] = (image[flags] & !sound_entry::SILENT)
                | if silent { sound_entry::SILENT } else { 0 };
            let sequences = entry + idx(sound_entry::SEQUENCES);
            image[sequences..sequences + 4].copy_from_slice(channels);
        }
        for (i, effect) in self.effects.iter().enumerate() {
            let record = idx(SOUND_EFFECTS) + i * idx(EFFECT_SIZE);
            let (t, v) = (&effect.tremolo, &effect.vibrato);
            for (offset, values) in [
                (effect::TREMOLO_LENGTH, t.length.as_slice()),
                (effect::VIBRATO_LENGTH, &v.length),
                (effect::TREMOLO_RATE, &t.rate),
                (effect::VIBRATO_RATE, &v.rate),
                (effect::TREMOLO_PAUSE, &t.pause),
                (effect::VIBRATO_PAUSE, &v.pause),
            ] {
                for (k, &value) in values.iter().enumerate() {
                    image[record + idx(offset) + k] = value as u8;
                }
            }
        }
        for (&start, table) in PITCH_TABLES.iter().zip(&self.pitch_tables) {
            for (i, period) in table.iter().enumerate() {
                let word = idx(start) + i * 2;
                image[word..word + 2].copy_from_slice(&period.to_be_bytes());
            }
        }
    }
}

fn signed_bytes<const N: usize>(image: &[u8], at: usize) -> [i8; N] {
    std::array::from_fn(|k| image.i8_at(at + k))
}

/// One bank: sequences and samples in memory order, and the instruments
/// that play the samples.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SoundData {
    pub sequences: Vec<Sequence>,
    pub instruments: Vec<Instrument>,
    pub samples: Vec<Sample>,
}

/// An instrument: where its sample starts, how long it plays and from
/// which octave its notes are pitched.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Instrument {
    pub sample: String,
    /// Whether the sample repeats (from `loop_start`) for as long as the
    /// note lasts, rather than playing once.
    pub looping: bool,
    /// In bytes from the start.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub loop_start: u16,
    /// In words (two bytes), if not the whole sample: it may run on into
    /// the samples after.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub length: Option<u16>,
    pub octave: u32,
}

fn is_zero(value: &u16) -> bool {
    *value == 0
}

/// Signed 8-bit PCM.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sample {
    pub name: String,
    pub data: Vec<u8>,
}

/// Every bank the game plays from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SoundContent {
    pub tables: SoundTables,
    pub game: SoundData,
    pub menus: Option<SoundData>,
    pub intro: Option<SoundData>,
}

/// The table of sequences.
struct SequenceTable {
    at: usize,
    /// Entries, including the empty one for sequence 0.
    count: usize,
    /// Each sequence's number and address.
    starts: Vec<(u8, usize)>,
}

/// Reads the sequence table. Entry 0, which no sound plays, is empty; the
/// table ends at the next empty entry (the samples start with silence).
fn sequence_table(image: &[u8]) -> SequenceTable {
    let base = idx(SOUND_BASE);
    let table = pointer(image, base);
    let mut count = 0;
    let mut starts = Vec::new();
    for number in 0..=255u8 {
        let at = table + usize::from(number) * 4;
        if at + 4 > image.len() {
            break;
        }
        let offset = image.u32_at(at) as usize;
        if number > 0 && (offset == 0 || base + offset >= table) {
            break;
        }
        count += 1;
        if offset != 0 {
            starts.push((number, base + offset));
        }
    }
    SequenceTable {
        at: table,
        count,
        starts,
    }
}

/// An instrument as the image holds it.
struct InstrumentRecord {
    one_shot: i16,
    loop_start: u16,
    length: u16,
    sample_at: usize,
    octave: u32,
}

impl InstrumentRecord {
    fn read(image: &[u8], at: usize) -> Self {
        let field = |offset: u32| at + idx(offset);
        Self {
            one_shot: image.i16_at(field(instrument::ONE_SHOT)),
            loop_start: image.u16_at(field(instrument::LOOP_START)),
            length: image.u16_at(field(instrument::LENGTH)),
            sample_at: pointer(image, field(instrument::SAMPLE)),
            octave: image.u32_at(field(instrument::OCTAVE)),
        }
    }
}

impl SoundData {
    /// Reads the bank in `image`. Its instruments are those the sequences
    /// use.
    pub fn read(image: &[u8]) -> Result<Self, String> {
        let SequenceTable {
            at: table,
            count,
            mut starts,
        } = sequence_table(image);
        let table_end = table + count * 4;
        starts.sort_by_key(|&(_, address)| address);
        let mut sequences = Vec::new();
        for (i, &(number, address)) in starts.iter().enumerate() {
            let end = starts.get(i + 1).map_or(table, |&(_, a)| a);
            if end == address {
                return Err(format!("sequences share the address {address:#X}"));
            }
            sequences.push(Sequence {
                number,
                bytes: image[address..end].to_vec(),
            });
        }
        let instrument_count = sequences
            .iter()
            .flat_map(|s| s.references().instruments)
            .max()
            .map_or(0, |n| usize::from(n) + 1);
        let instruments_at = pointer(image, idx(INSTRUMENT_TABLE_INDEX));
        let records: Vec<_> = (0..instrument_count)
            .map(|i| InstrumentRecord::read(image, instruments_at + i * idx(INSTRUMENT_SIZE)))
            .collect();
        let mut sample_starts: Vec<usize> = records.iter().map(|r| r.sample_at).collect();
        sample_starts.sort_unstable();
        sample_starts.dedup();
        if sample_starts.first().is_some_and(|&s| s != table_end)
            || sample_starts.last().is_some_and(|&s| s >= instruments_at)
        {
            return Err("the samples are not between the two tables".into());
        }
        let samples: Vec<Sample> = sample_starts
            .iter()
            .enumerate()
            .map(|(i, &start)| {
                let end = sample_starts.get(i + 1).copied().unwrap_or(instruments_at);
                Sample {
                    name: format!("sample_{i:02}"),
                    data: image[start..end].to_vec(),
                }
            })
            .collect();
        let instruments = records
            .iter()
            .map(|record| {
                let i = sample_starts.binary_search(&record.sample_at).unwrap();
                let whole = samples[i].data.len() / 2;
                Instrument {
                    sample: samples[i].name.clone(),
                    looping: record.one_shot < 0,
                    loop_start: record.loop_start,
                    length: (usize::from(record.length) != whole).then_some(record.length),
                    octave: record.octave,
                }
            })
            .collect();
        Ok(Self {
            sequences,
            instruments,
            samples,
        })
    }

    /// What is wrong with the bank, if anything: references to sequences,
    /// instruments, effects or samples that are not there.
    pub fn check(&self) -> Result<(), String> {
        let mut problems = Vec::new();
        let has_sequence = |n: u8| self.sequences.iter().any(|s| s.number == n);
        for sequence in &self.sequences {
            let references = sequence.references();
            let at = format!("sequence {}", sequence.number);
            for n in references.sequences {
                if !has_sequence(n) {
                    problems.push(format!("{at}: there is no sequence {n}"));
                }
            }
            for n in references.instruments {
                if usize::from(n) >= self.instruments.len() {
                    problems.push(format!("{at}: there is no instrument {n}"));
                }
            }
            for n in references.effects {
                if usize::from(n) >= EFFECTS {
                    problems.push(format!("{at}: there is no effect {n}"));
                }
            }
        }
        for (i, sample) in self.samples.iter().enumerate() {
            if self.samples[..i].iter().any(|s| s.name == sample.name) {
                problems.push(format!("sample {} comes twice", sample.name));
            }
        }
        for (i, instrument) in self.instruments.iter().enumerate() {
            match self.samples.iter().find(|s| s.name == instrument.sample) {
                None => problems.push(format!(
                    "instrument {i}: there is no sample {}",
                    instrument.sample
                )),
                Some(sample) if instrument.length.is_none() && sample.data.len() / 2 > 0xFFFF => {
                    problems.push(format!(
                        "instrument {i}: sample {} is over 131070 bytes",
                        sample.name
                    ))
                }
                Some(_) => {}
            }
        }
        crate::data::report(problems)
    }

    /// Whether every sound's sequences are in this bank.
    pub fn check_sounds(&self, tables: &SoundTables) -> Result<(), String> {
        let missing: Vec<String> = tables
            .sounds
            .iter()
            .enumerate()
            .flat_map(|(i, channels)| channels.iter().map(move |&n| (i, n)))
            .filter(|&(_, n)| n != 0 && !self.sequences.iter().any(|s| s.number == n))
            .map(|(i, n)| format!("sound {i}: there is no sequence {n}"))
            .collect();
        crate::data::report(missing)
    }

    /// `base` with `tables` and this bank written over it. The sequences
    /// start where `base`'s do.
    pub fn lay_out(&self, tables: &SoundTables, base: &[u8]) -> Result<Vec<u8>, String> {
        self.check()?;
        let mut image = base.to_vec();
        tables.write(&mut image);
        let mut at = sequence_table(base)
            .starts
            .iter()
            .map(|&(_, address)| address)
            .min()
            .ok_or("the original has no sequences")?;

        let mut addresses = [None; 256];
        for sequence in &self.sequences {
            addresses[usize::from(sequence.number)] = Some(at);
            put(&mut image, at, &sequence.bytes);
            at += sequence.bytes.len();
        }
        let table = at;
        let count = self
            .sequences
            .iter()
            .map(|s| usize::from(s.number) + 1)
            .max()
            .unwrap_or(0);
        for address in &addresses[..count] {
            let offset = address.map_or(0, offset_of);
            put(&mut image, at, &offset.to_be_bytes());
            at += 4;
        }

        let mut sample_at = Vec::new();
        for sample in &self.samples {
            sample_at.push(at);
            put(&mut image, at, &sample.data);
            at += sample.data.len();
        }
        let instruments = at;
        for instrument in &self.instruments {
            let i = self
                .samples
                .iter()
                .position(|s| s.name == instrument.sample)
                .expect("checked");
            let length = instrument
                .length
                .unwrap_or((self.samples[i].data.len() / 2) as u16);
            let one_shot: i16 = if instrument.looping { -1 } else { 1 };
            let mut record = [0; idx(INSTRUMENT_SIZE)];
            let sample = offset_of(sample_at[i]);
            for (offset, bytes) in [
                (instrument::ONE_SHOT, &one_shot.to_be_bytes()[..]),
                (instrument::LOOP_START, &instrument.loop_start.to_be_bytes()),
                (instrument::LENGTH, &length.to_be_bytes()),
                (instrument::SAMPLE, &sample.to_be_bytes()),
                (instrument::OCTAVE, &instrument.octave.to_be_bytes()),
            ] {
                let field = idx(offset);
                record[field..field + bytes.len()].copy_from_slice(bytes);
            }
            put(&mut image, at, &record);
            at += record.len();
        }

        put(&mut image, idx(SOUND_BASE), &offset_of(table).to_be_bytes());
        put(
            &mut image,
            idx(INSTRUMENT_TABLE_INDEX),
            &offset_of(instruments).to_be_bytes(),
        );
        Ok(image)
    }
}

/// Writes `bytes` at `at`, growing the image if they run past its end.
fn put(image: &mut Vec<u8>, at: usize, bytes: &[u8]) {
    if image.len() < at + bytes.len() {
        image.resize(at + bytes.len(), 0);
    }
    image[at..at + bytes.len()].copy_from_slice(bytes);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::data::amiga::{menu_ram, Overlays};
    use crate::data::files::read_amiga_files;
    use crate::sound::driver::Driver;
    use crate::sound::memory::Memory;

    /// The match's, the menus' and the intro's memory, as the originals
    /// hold them.
    fn images() -> Option<Vec<(&'static str, Vec<u8>)>> {
        let config = Config::default();
        let amiga = read_amiga_files(&config.data_dir).ok()?;
        let overlays = Overlays::new(&amiga.disk)?;
        let menus = menu_ram(&amiga.match_memory, &overlays);
        Some(vec![
            ("match", amiga.match_memory),
            ("menus", menus),
            ("intro", amiga.intro_memory),
        ])
    }

    #[test]
    fn every_bank_lays_out_exactly_as_it_was() {
        let Some(images) = images() else {
            eprintln!("skipping: no originals");
            return;
        };
        let tables = SoundTables::read(&images[0].1);
        tables.check().unwrap();
        for (name, image) in &images {
            let data = SoundData::read(image).unwrap();
            data.check().unwrap();
            assert!(!data.sequences.is_empty() && !data.samples.is_empty());
            assert_eq!(SoundTables::read(image).sounds, tables.sounds, "{name}");
            let laid_out = data.lay_out(&tables, image).unwrap();
            assert!(laid_out == *image, "{name} differs");
        }
        let game = SoundData::read(&images[0].1).unwrap();
        game.check_sounds(&tables).unwrap();
        let menus = SoundData::read(&images[1].1).unwrap();
        assert_eq!(game.sequences, menus.sequences);
        assert_eq!(game.instruments, menus.instruments);
        assert_ne!(game.samples, menus.samples);
    }

    #[test]
    fn longer_data_moves_what_follows() {
        let Some(images) = images() else {
            return;
        };
        let image = &images[0].1;
        let tables = SoundTables::read(image);
        let mut data = SoundData::read(image).unwrap();
        data.sequences[0].bytes.push(0xAC);
        data.samples[3].data.extend_from_slice(&[1, 2]);
        let laid_out = data.lay_out(&tables, image).unwrap();
        let back = SoundData::read(&laid_out).unwrap();
        assert_eq!(back.sequences, data.sequences);
        assert_eq!(back.samples, data.samples);
        assert_eq!(back.instruments, data.instruments);
    }

    /// What sound `id` sounds like for its first two seconds.
    fn render(image: Vec<u8>, id: u8) -> Vec<f32> {
        let mut driver = Driver::new(Memory::new(image), 48_000);
        driver.play(id, 0);
        let mut out = vec![0.0; 2 * 48_000 * 2];
        driver.render(&mut out);
        out
    }

    #[test]
    fn edits_change_what_plays() {
        let Some(images) = images() else {
            return;
        };
        let image = &images[0].1;
        let tables = SoundTables::read(image);
        let data = SoundData::read(image).unwrap();
        // Sound 3, the throw, plays sequence 5: one note of instrument 19.
        assert_eq!(tables.sounds[3], [5, 0, 0, 0]);
        let original = render(image.clone(), 3);
        assert!(
            original.iter().any(|s| s.abs() > 0.01),
            "the throw is audible"
        );
        assert_eq!(render(data.lay_out(&tables, image).unwrap(), 3), original);

        let mut lower = data.clone();
        let sequence = lower.sequences.iter_mut().find(|s| s.number == 5).unwrap();
        let note = sequence.bytes.iter().position(|&b| b == 53).unwrap();
        sequence.bytes[note] = 41;
        assert_ne!(render(lower.lay_out(&tables, image).unwrap(), 3), original);

        let mut quieter = data.clone();
        let sample = &quieter.instruments[19].sample;
        let sample = quieter
            .samples
            .iter_mut()
            .find(|s| &s.name == sample)
            .unwrap();
        for byte in &mut sample.data {
            *byte = ((*byte as i8) / 2) as u8;
        }
        assert_ne!(
            render(quieter.lay_out(&tables, image).unwrap(), 3),
            original
        );
    }

    #[test]
    fn mistakes_are_found() {
        let Some(images) = images() else {
            return;
        };
        let mut data = SoundData::read(&images[0].1).unwrap();
        data.instruments[0].sample = "nothing".into();
        data.sequences[0].bytes.extend_from_slice(&[0xD4, 250]);
        let problems = data.check().unwrap_err();
        assert!(problems.contains("there is no sequence 250"), "{problems}");
        assert!(
            problems.contains("instrument 0: there is no sample nothing"),
            "{problems}"
        );
    }
}

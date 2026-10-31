//! Sound in a pack: the driver's tables in JSON, each bank's sequences as
//! text, its instruments in JSON and its samples as WAV files.
//!
//! The menus play the match's sequences and instruments with some samples
//! of their own, so `sound/menus/` holds only those samples.

use serde::{Deserialize, Serialize};

use super::pack::{Pack, PackWriter};
use super::ContentError;
use crate::sound::data::{Instrument, Sample, SoundContent, SoundData, SoundTables};
use crate::sound::{sequences, wav};

const TABLES: &str = "sound/sounds";
const GAME: &str = "sound/game";
const MENUS: &str = "sound/menus";
const INTRO: &str = "sound/intro";
const SEQUENCES: &str = "sequences.txt";
const INSTRUMENTS: &str = "instruments";

/// `instruments.json`: the samples in the order they are laid out, and the
/// instruments by number.
#[derive(Serialize, Deserialize)]
struct Instruments {
    samples: Vec<String>,
    instruments: Vec<Instrument>,
}

pub fn write(w: &mut PackWriter, sound: &SoundContent) -> Result<(), ContentError> {
    w.json(TABLES, &sound.tables)?;
    write_bank(w, GAME, &sound.game)?;
    if let Some(menus) = &sound.menus {
        if (&menus.sequences, &menus.instruments)
            != (&sound.game.sequences, &sound.game.instruments)
        {
            return Err(ContentError::Unreadable(
                "the menus' sound differs from the match's in more than samples".into(),
            ));
        }
        for sample in &menus.samples {
            if !sound.game.samples.contains(sample) {
                write_sample(w, MENUS, sample)?;
            }
        }
    }
    if let Some(intro) = &sound.intro {
        write_bank(w, INTRO, intro)?;
    }
    Ok(())
}

fn write_bank(w: &mut PackWriter, dir: &str, data: &SoundData) -> Result<(), ContentError> {
    w.file(
        &format!("{dir}/{SEQUENCES}"),
        sequences::to_text(&data.sequences).as_bytes(),
    )?;
    w.json(
        &format!("{dir}/{INSTRUMENTS}"),
        &Instruments {
            samples: data.samples.iter().map(|s| s.name.clone()).collect(),
            instruments: data.instruments.clone(),
        },
    )?;
    for sample in &data.samples {
        write_sample(w, dir, sample)?;
    }
    Ok(())
}

fn sample_file(dir: &str, name: &str) -> String {
    format!("{dir}/samples/{name}.wav")
}

fn write_sample(w: &mut PackWriter, dir: &str, sample: &Sample) -> Result<(), ContentError> {
    w.file(
        &sample_file(dir, &sample.name),
        &wav::mono_8(&sample.data, wav::SAMPLE_RATE),
    )
}

/// The pack's sound, checked. A pack without sound is
/// [`ContentError::Unavailable`], and plays silent.
pub fn read(pack: &Pack) -> Result<SoundContent, ContentError> {
    if !pack.file_path(&format!("{TABLES}.json")).exists() {
        return Err(ContentError::Unavailable("sound in the pack"));
    }
    let tables = pack.checked_json(TABLES, SoundTables::check)?;
    let game = read_bank(pack, GAME)?;
    game.check_sounds(&tables)
        .map_err(|e| ContentError::invalid(&pack.json_path(TABLES), e))?;
    let mut menus = game.clone();
    let has_menus = pack.has_dir(super::graphics::MENUS);
    for sample in &mut menus.samples {
        let file = sample_file(MENUS, &sample.name);
        if pack.file_path(&file).exists() {
            sample.data = read_sample(pack, &file)?;
        }
    }
    let intro = if pack.file_path(&format!("{INTRO}/{SEQUENCES}")).exists() {
        Some(read_bank(pack, INTRO)?)
    } else {
        None
    };
    Ok(SoundContent {
        tables,
        game,
        menus: has_menus.then_some(menus),
        intro,
    })
}

fn read_bank(pack: &Pack, dir: &str) -> Result<SoundData, ContentError> {
    let sequences_file = format!("{dir}/{SEQUENCES}");
    let text = pack.read_file(&sequences_file)?;
    let text = String::from_utf8(text).map_err(|_| {
        ContentError::invalid(&pack.file_path(&sequences_file), "is not UTF-8".into())
    })?;
    let sequences = sequences::parse(&text)
        .map_err(|e| ContentError::invalid(&pack.file_path(&sequences_file), e))?;
    let name = format!("{dir}/{INSTRUMENTS}");
    let file: Instruments = pack
        .json(&name)?
        .ok_or_else(|| ContentError::Missing(pack.json_path(&name).display().to_string()))?;
    let samples = file
        .samples
        .iter()
        .map(|name| {
            Ok(Sample {
                name: name.clone(),
                data: read_sample(pack, &sample_file(dir, name))?,
            })
        })
        .collect::<Result<_, ContentError>>()?;
    let data = SoundData {
        sequences,
        instruments: file.instruments,
        samples,
    };
    data.check()
        .map_err(|e| ContentError::invalid(&pack.file_path(dir), e))?;
    Ok(data)
}

fn read_sample(pack: &Pack, file: &str) -> Result<Vec<u8>, ContentError> {
    let bytes = pack.read_file(file)?;
    wav::read_mono(&bytes).map_err(|e| ContentError::invalid(&pack.file_path(file), e))
}

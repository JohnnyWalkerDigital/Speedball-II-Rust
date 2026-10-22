//! Locating and loading the original disk images and memory dumps.

use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::config::Platform;

const ATARI_DISK: &str =
    "Speedball 2 - Brutal Deluxe (1990)(Bitmap Brothers)[!] - Track 00 filled.st";
const AMIGA_DISK: &str = "Speedball 2 - Brutal Deluxe (1990)(ImageWorks)[cr DC].adf";
const AMIGA_MATCH_DUMP: &str = "Amiga.dmp";
const AMIGA_INTRO_DUMP: &str = "Intro - Amiga.dmp";

const ATARI_DISK_SIZE: usize = 819_200;
const AMIGA_DISK_SIZE: usize = 901_120;
const AMIGA_DUMP_SIZE: usize = 524_288;

/// Raw bytes of every file the selected platform needs.
///
/// The match logic always runs off the Atari ST disk (that is where the
/// original port found its lookup tables); the Amiga files are used for
/// graphics, sound and the menus.
pub struct GameFiles {
    pub atari_disk: Vec<u8>,
    pub amiga: Option<AmigaFiles>,
}

pub struct AmigaFiles {
    pub disk: Vec<u8>,
    /// RAM dump taken during a match (palettes, fonts, pitch map).
    pub match_dump: Vec<u8>,
}

#[derive(Debug)]
pub enum LoadError {
    Io {
        path: PathBuf,
        source: io::Error,
    },
    WrongSize {
        path: PathBuf,
        expected: usize,
        actual: usize,
    },
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LoadError::Io { path, source } => {
                write!(f, "cannot read {}: {source}", path.display())
            }
            LoadError::WrongSize {
                path,
                expected,
                actual,
            } => write!(
                f,
                "{} is {actual} bytes, expected {expected}; is it the right image?",
                path.display()
            ),
        }
    }
}

impl std::error::Error for LoadError {}

impl GameFiles {
    pub fn load(data_dir: &Path, platform: Platform) -> Result<Self, LoadError> {
        let atari_disk = read_exact_size(&data_dir.join(ATARI_DISK), ATARI_DISK_SIZE)?;
        let amiga = match platform {
            Platform::Atari => None,
            Platform::Amiga => Some(read_amiga_files(data_dir)?),
        };
        Ok(Self { atari_disk, amiga })
    }
}

/// The Amiga disk and match dump.
pub fn read_amiga_files(data_dir: &Path) -> Result<AmigaFiles, LoadError> {
    Ok(AmigaFiles {
        disk: read_exact_size(&data_dir.join(AMIGA_DISK), AMIGA_DISK_SIZE)?,
        match_dump: read_amiga_match_dump(data_dir)?,
    })
}

/// The Amiga match RAM dump on its own; sound needs it on either platform.
pub fn read_amiga_match_dump(data_dir: &Path) -> Result<Vec<u8>, LoadError> {
    read_exact_size(&data_dir.join(AMIGA_MATCH_DUMP), AMIGA_DUMP_SIZE)
}

/// The Amiga RAM dump taken during the intro: its pictures, font, text and
/// music.
pub fn read_amiga_intro_dump(data_dir: &Path) -> Result<Vec<u8>, LoadError> {
    read_exact_size(&data_dir.join(AMIGA_INTRO_DUMP), AMIGA_DUMP_SIZE)
}

fn read_exact_size(path: &Path, expected: usize) -> Result<Vec<u8>, LoadError> {
    let bytes = fs::read(path).map_err(|source| LoadError::Io {
        path: path.to_owned(),
        source,
    })?;
    if bytes.len() != expected {
        return Err(LoadError::WrongSize {
            path: path.to_owned(),
            expected,
            actual: bytes.len(),
        });
    }
    Ok(bytes)
}

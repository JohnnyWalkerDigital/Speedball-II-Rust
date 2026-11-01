//! Locating and loading the original disk images and memory dumps.

use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::config::Platform;

/// The disk images are found by extension and size, whatever they are
/// called.
const ATARI_DISK: DiskImage = DiskImage {
    what: "Atari ST disk image",
    extension: "st",
    size: 819_200,
};
const AMIGA_DISK: DiskImage = DiskImage {
    what: "Amiga disk image",
    extension: "adf",
    size: 901_120,
};
const AMIGA_MATCH_DUMP: &str = "Amiga.dmp";
const AMIGA_INTRO_DUMP: &str = "Intro - Amiga.dmp";

const AMIGA_DUMP_SIZE: usize = 524_288;

/// Raw bytes of every file the selected platform needs.
pub struct GameFiles {
    /// The Amiga's memory during the intro: the match's data, and the
    /// intro's pictures, text and music.
    pub intro_dump: Vec<u8>,
    /// The Atari ST disk, for the ST's graphics.
    pub atari_disk: Option<Vec<u8>>,
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
    /// No file, or more than one, could be the disk image.
    NoDiskImage {
        dir: PathBuf,
        image: DiskImage,
        candidates: Vec<PathBuf>,
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
            LoadError::NoDiskImage {
                dir,
                image,
                candidates,
            } => {
                let wanted = format!(
                    "{} (a .{} file of {} bytes)",
                    image.what, image.extension, image.size
                );
                if candidates.is_empty() {
                    write!(f, "{} has no {wanted}", dir.display())
                } else {
                    let names: Vec<_> =
                        candidates.iter().map(|p| p.display().to_string()).collect();
                    write!(
                        f,
                        "more than one file could be the {wanted}: {}",
                        names.join(", ")
                    )
                }
            }
        }
    }
}

impl std::error::Error for LoadError {}

impl GameFiles {
    /// The intro dump, and the files for `platform`'s graphics.
    pub fn load(data_dir: &Path, platform: Platform) -> Result<Self, LoadError> {
        Ok(Self {
            intro_dump: read_amiga_intro_dump(data_dir)?,
            atari_disk: match platform {
                Platform::Atari => Some(read_atari_disk(data_dir)?),
                Platform::Amiga => None,
            },
            amiga: match platform {
                Platform::Atari => None,
                Platform::Amiga => Some(read_amiga_files(data_dir)?),
            },
        })
    }
}

/// The Atari ST disk on its own.
pub fn read_atari_disk(data_dir: &Path) -> Result<Vec<u8>, LoadError> {
    read_disk_image(data_dir, ATARI_DISK)
}

/// The Amiga disk and match dump.
pub fn read_amiga_files(data_dir: &Path) -> Result<AmigaFiles, LoadError> {
    Ok(AmigaFiles {
        disk: read_disk_image(data_dir, AMIGA_DISK)?,
        match_dump: read_amiga_match_dump(data_dir)?,
    })
}

/// The Amiga match RAM dump on its own; sound needs it on either platform.
pub fn read_amiga_match_dump(data_dir: &Path) -> Result<Vec<u8>, LoadError> {
    read_exact_size(&data_dir.join(AMIGA_MATCH_DUMP), AMIGA_DUMP_SIZE)
}

/// The Amiga RAM dump taken during the intro, before any match.
pub fn read_amiga_intro_dump(data_dir: &Path) -> Result<Vec<u8>, LoadError> {
    read_exact_size(&data_dir.join(AMIGA_INTRO_DUMP), AMIGA_DUMP_SIZE)
}

/// A kind of disk image the game needs.
#[derive(Clone, Copy, Debug)]
pub struct DiskImage {
    what: &'static str,
    extension: &'static str,
    size: usize,
}

/// The one file in `dir` that could be `image`: its extension (in any
/// case) and size.
fn read_disk_image(dir: &Path, image: DiskImage) -> Result<Vec<u8>, LoadError> {
    let entries = fs::read_dir(dir).map_err(|source| LoadError::Io {
        path: dir.to_owned(),
        source,
    })?;
    let mut candidates: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|e| e.eq_ignore_ascii_case(image.extension))
                && fs::metadata(path).is_ok_and(|m| m.len() == image.size as u64)
        })
        .collect();
    candidates.sort();
    match candidates.as_slice() {
        [path] => read_exact_size(path, image.size),
        _ => Err(LoadError::NoDiskImage {
            dir: dir.to_owned(),
            image,
            candidates,
        }),
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disk_images_are_found_by_extension_and_size() {
        let dir = std::env::temp_dir().join(format!("sb2-images-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let image = DiskImage {
            what: "test image",
            extension: "adf",
            size: 4,
        };
        assert!(matches!(
            read_disk_image(&dir, image),
            Err(LoadError::NoDiskImage { .. })
        ));
        fs::write(dir.join("too short.adf"), [1, 2]).unwrap();
        fs::write(dir.join("notes.txt"), [1, 2, 3, 4]).unwrap();
        fs::write(dir.join("Any Name.ADF"), [1, 2, 3, 4]).unwrap();
        assert_eq!(read_disk_image(&dir, image).unwrap(), [1, 2, 3, 4]);
        fs::write(dir.join("another.adf"), [5, 6, 7, 8]).unwrap();
        let error = read_disk_image(&dir, image).unwrap_err().to_string();
        assert!(error.starts_with("more than one file"), "{error}");
        fs::remove_dir_all(dir).unwrap();
    }
}

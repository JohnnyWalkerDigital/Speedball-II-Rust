//! Locating and loading the original disk images.

use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use super::amiga::{intro_memory, match_memory, Overlays};
use super::ipf;
use crate::config::Platform;

/// The disk images are found by extension (and the ST's by size),
/// whatever they are called.
const ATARI_DISK: DiskImage = DiskImage {
    what: "Atari ST disk image",
    extension: "st",
    size: 819_200,
};
const AMIGA_DISK_EXTENSION: &str = "ipf";

/// The original files, loaded.
pub struct GameFiles {
    pub amiga: AmigaFiles,
    /// The Atari ST disk, for the ST's graphics.
    pub atari_disk: Option<Vec<u8>>,
}

/// The Amiga disk, and the Amiga's memory loaded from it.
pub struct AmigaFiles {
    /// The disk's sectors, in order.
    pub disk: Vec<u8>,
    /// Memory as the intro starts: the match's data, and the intro's
    /// pictures, text and music.
    pub intro_memory: Vec<u8>,
    /// Memory in a match: its palette, fonts, pitch map and sound.
    pub match_memory: Vec<u8>,
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
    /// No IPF in the folder is a release of the game the port reads, for
    /// these reasons.
    NoAmigaDisk {
        dir: PathBuf,
        rejected: Vec<(PathBuf, String)>,
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
            LoadError::NoAmigaDisk { dir, rejected } => {
                if rejected.is_empty() {
                    return write!(
                        f,
                        "{} has no Amiga disk image (a .{AMIGA_DISK_EXTENSION} file)",
                        dir.display()
                    );
                }
                write!(f, "no Amiga disk image in {} can be used:", dir.display())?;
                for (path, reason) in rejected {
                    write!(f, "\n  {}: {reason}", path.display())?;
                }
                Ok(())
            }
        }
    }
}

impl std::error::Error for LoadError {}

impl GameFiles {
    /// The Amiga disk, and the ST's for its graphics.
    pub fn load(data_dir: &Path, platform: Platform) -> Result<Self, LoadError> {
        Ok(Self {
            amiga: read_amiga_files(data_dir)?,
            atari_disk: match platform {
                Platform::Atari => Some(read_atari_disk(data_dir)?),
                Platform::Amiga => None,
            },
        })
    }
}

/// The Atari ST disk on its own.
pub fn read_atari_disk(data_dir: &Path) -> Result<Vec<u8>, LoadError> {
    read_disk_image(data_dir, ATARI_DISK)
}

/// The first IPF in `data_dir` that is a release the port reads, with the
/// Amiga's memory loaded from it.
pub fn read_amiga_files(data_dir: &Path) -> Result<AmigaFiles, LoadError> {
    let mut rejected = Vec::new();
    for path in files_with_extension(data_dir, AMIGA_DISK_EXTENSION)? {
        let ipf = fs::read(&path).map_err(|source| LoadError::Io {
            path: path.clone(),
            source,
        })?;
        match amiga_files(&ipf) {
            Ok(files) => return Ok(files),
            Err(reason) => rejected.push((path, reason)),
        }
    }
    Err(LoadError::NoAmigaDisk {
        dir: data_dir.to_owned(),
        rejected,
    })
}

fn amiga_files(ipf: &[u8]) -> Result<AmigaFiles, String> {
    let disk = ipf::decode(ipf).map_err(|e| e.to_string())?;
    let other_build = || {
        format!(
            "SPS release {} is a build of the game this port does not support yet",
            disk.release
        )
    };
    let overlays = Overlays::new(&disk.sectors).ok_or_else(other_build)?;
    let intro_memory = intro_memory(&disk.sectors).map_err(|_| other_build())?;
    let match_memory = match_memory(&intro_memory, &overlays);
    Ok(AmigaFiles {
        disk: disk.sectors,
        intro_memory,
        match_memory,
    })
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
    let candidates: Vec<PathBuf> = files_with_extension(dir, image.extension)?
        .into_iter()
        .filter(|path| fs::metadata(path).is_ok_and(|m| m.len() == image.size as u64))
        .collect();
    match candidates.as_slice() {
        [path] => read_exact_size(path, image.size),
        _ => Err(LoadError::NoDiskImage {
            dir: dir.to_owned(),
            image,
            candidates,
        }),
    }
}

/// The files in `dir` with `extension`, in any case, sorted.
fn files_with_extension(dir: &Path, extension: &str) -> Result<Vec<PathBuf>, LoadError> {
    let entries = fs::read_dir(dir).map_err(|source| LoadError::Io {
        path: dir.to_owned(),
        source,
    })?;
    let mut paths: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|e| e.eq_ignore_ascii_case(extension))
        })
        .collect();
    paths.sort();
    Ok(paths)
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

    #[test]
    fn unusable_ipfs_are_reported_one_by_one() {
        let dir = std::env::temp_dir().join(format!("sb2-ipfs-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let error = read_amiga_files(&dir).err().unwrap().to_string();
        assert!(
            error.ends_with("has no Amiga disk image (a .ipf file)"),
            "{error}"
        );
        fs::write(dir.join("not really.ipf"), b"FORM").unwrap();
        let error = read_amiga_files(&dir).err().unwrap().to_string();
        assert!(
            error.ends_with("not really.ipf: not an IPF image"),
            "{error}"
        );
        fs::remove_dir_all(dir).unwrap();
    }
}

//! Where the game's graphics and words come from: the original disks and
//! memory dumps ([`Originals`]), or a [`Pack`] of ordinary image and text
//! files extracted from them, which can be edited.
//!
//! The game asks a [`ContentSource`], so it does not matter which.

mod graphics;
mod json;
mod originals;
mod pack;
mod png_files;
mod sound;
mod tables;
mod text;

use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

use crate::config::Platform;
use crate::data::animations::AnimationTable;
use crate::data::lookups::Lookups;
use crate::game::roster::Teams;
use crate::game::start::StartTable;
use crate::graphics::ilbm::IlbmError;
use crate::graphics::Assets;
use crate::menu::assets::MenuAssets;
use crate::menu::flow::MenuContent;
use crate::menu::intro::{IntroGraphics, IntroText};
use crate::menu::manager::data::ManagerTables;
use crate::menu::strings::MenuText;
use crate::menu::tables::MenuTables;
use crate::sound::data::SoundContent;

pub use originals::{OriginalFiles, Originals};
pub use pack::Pack;

/// Supplies decoded graphics and text.
///
/// Parts a source may not have (the Amiga match graphics, the menus, the
/// sound) are [`ContentError::Unavailable`] when absent, and the intro is
/// `Ok(None)`, its graphics and text together. Any other error means a part
/// is there but can't be used.
pub trait ContentSource {
    /// Everything a match draws, in one platform's style.
    fn match_graphics(&self, platform: Platform) -> Result<Assets, ContentError>;
    /// The menus' pictures, fonts and manager screens.
    fn menu_graphics(&self) -> Result<MenuAssets, ContentError>;
    /// The intro's pictures and font, if there is an intro.
    fn intro_graphics(&self) -> Result<Option<IntroGraphics>, ContentError>;
    /// Every string the menus show.
    fn menu_text(&self) -> Result<MenuText, ContentError>;
    /// The intro's text, if there is an intro.
    fn intro_text(&self) -> Result<Option<IntroText>, ContentError>;
    /// The match's lookup tables.
    fn lookups(&self) -> Result<Lookups, ContentError>;
    /// The match's animation scripts.
    fn animations(&self) -> Result<AnimationTable, ContentError>;
    /// What every match starts from, naming scripts in [`Self::animations`].
    fn match_start(&self) -> Result<StartTable, ContentError>;
    /// The league teams' names and skills, and player two's.
    fn teams(&self) -> Result<Teams, ContentError>;
    /// The squads, market, prices and results behind the menus' games.
    fn menu_tables(&self) -> Result<MenuTables, ContentError>;
    /// The manager screens' layout: keypads, gym buttons, labels.
    fn manager_tables(&self) -> Result<ManagerTables, ContentError>;
    /// The sound driver's tables, sequences, instruments and samples.
    fn sound(&self) -> Result<SoundContent, ContentError>;
}

#[derive(Debug)]
pub enum ContentError {
    Io {
        path: PathBuf,
        source: io::Error,
    },
    Png {
        path: PathBuf,
        message: String,
    },
    Manifest(String),
    /// A file that does not say what the game needs.
    Invalid {
        path: PathBuf,
        message: String,
    },
    Missing(String),
    /// Data that is there but can't be made sense of.
    Unreadable(String),
    Picture(IlbmError),
    /// The originals needed are not there (the Amiga files).
    Unavailable(&'static str),
}

impl ContentError {
    pub(crate) fn io(path: &Path, source: io::Error) -> Self {
        ContentError::Io {
            path: path.to_owned(),
            source,
        }
    }

    pub(crate) fn invalid(path: &Path, message: String) -> Self {
        ContentError::Invalid {
            path: path.to_owned(),
            message,
        }
    }

    pub(crate) fn png(path: &Path, message: String) -> Self {
        ContentError::Png {
            path: path.to_owned(),
            message,
        }
    }
}

impl fmt::Display for ContentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ContentError::Io { path, source } => write!(f, "{}: {source}", path.display()),
            ContentError::Png { path, message } => write!(f, "{}: {message}", path.display()),
            ContentError::Manifest(message) => write!(f, "{message}"),
            ContentError::Invalid { path, message } => write!(f, "{}: {message}", path.display()),
            ContentError::Missing(what) => write!(f, "missing {what}"),
            ContentError::Unreadable(message) => write!(f, "{message}"),
            ContentError::Picture(error) => write!(f, "{error}"),
            ContentError::Unavailable(what) => write!(f, "needs {what}"),
        }
    }
}

impl std::error::Error for ContentError {}

impl From<IlbmError> for ContentError {
    fn from(error: IlbmError) -> Self {
        ContentError::Picture(error)
    }
}

/// Everything the menus show and play with, from `source`.
pub fn menu_content(source: &dyn ContentSource) -> Result<MenuContent, ContentError> {
    Ok(MenuContent {
        assets: source.menu_graphics()?,
        text: source.menu_text()?,
        tables: source.menu_tables()?,
        manager: source.manager_tables()?,
    })
}

/// What [`extract`] wrote.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Extracted {
    pub platforms: Vec<Platform>,
    pub menus: bool,
    pub intro: bool,
    pub sound: bool,
}

/// Writes everything `source` has into a pack at `dir`. Parts whose
/// originals are missing are left out.
pub fn extract(source: &dyn ContentSource, dir: &Path) -> Result<Extracted, ContentError> {
    let mut writer = pack::PackWriter::new(dir);
    let mut extracted = Extracted::default();
    writer.json(tables::LOOKUPS, &source.lookups()?)?;
    writer.json(tables::TEAMS, &source.teams()?)?;
    writer.json(tables::ANIMATIONS, &source.animations()?)?;
    writer.json(tables::START, &source.match_start()?)?;
    for platform in [Platform::Amiga, Platform::Atari] {
        if let Some(assets) = optional(source.match_graphics(platform))? {
            graphics::write_match(&mut writer, &assets)?;
            extracted.platforms.push(platform);
        }
    }
    if let Some(menus) = optional(source.menu_graphics())? {
        graphics::write_menus(&mut writer, &menus)?;
        writer.json(text::MENUS, &source.menu_text()?)?;
        writer.json(tables::SQUADS, &source.menu_tables()?)?;
        writer.json(tables::MANAGER, &source.manager_tables()?)?;
        extracted.menus = true;
    }
    if let Some(intro) = source.intro_graphics()? {
        graphics::write_intro(&mut writer, &intro)?;
        if let Some(intro) = source.intro_text()? {
            writer.json(text::INTRO, &intro)?;
        }
        extracted.intro = true;
    }
    if let Some(content) = optional(source.sound())? {
        sound::write(&mut writer, &content)?;
        extracted.sound = true;
    }
    writer.finish()?;
    Ok(extracted)
}

/// A part the source may not have, as `None` when it doesn't.
fn optional<T>(part: Result<T, ContentError>) -> Result<Option<T>, ContentError> {
    match part {
        Ok(part) => Ok(Some(part)),
        Err(ContentError::Unavailable(_)) => Ok(None),
        Err(error) => Err(error),
    }
}

/// Where a pack keeps the intro's pictures: the intro is in a pack if this
/// is.
const INTRO: &str = "intro";

impl ContentSource for Pack {
    fn match_graphics(&self, platform: Platform) -> Result<Assets, ContentError> {
        graphics::read_match(self, platform)
    }

    fn menu_graphics(&self) -> Result<MenuAssets, ContentError> {
        graphics::read_menus(self)
    }

    fn intro_graphics(&self) -> Result<Option<IntroGraphics>, ContentError> {
        if !self.has_dir(INTRO) {
            return Ok(None);
        }
        graphics::read_intro(self).map(Some)
    }

    fn menu_text(&self) -> Result<MenuText, ContentError> {
        self.checked_json(text::MENUS, MenuText::check)
    }

    fn intro_text(&self) -> Result<Option<IntroText>, ContentError> {
        if !self.has_dir(INTRO) {
            return Ok(None);
        }
        self.json(text::INTRO).map(Some)
    }

    fn lookups(&self) -> Result<Lookups, ContentError> {
        self.checked_json(tables::LOOKUPS, Lookups::check)
    }

    fn teams(&self) -> Result<Teams, ContentError> {
        self.checked_json(tables::TEAMS, Teams::check)
    }

    fn animations(&self) -> Result<AnimationTable, ContentError> {
        tables::read_animations(self)
    }

    fn match_start(&self) -> Result<StartTable, ContentError> {
        tables::read_start(self)
    }

    fn menu_tables(&self) -> Result<MenuTables, ContentError> {
        self.checked_json(tables::SQUADS, MenuTables::check)
    }

    fn manager_tables(&self) -> Result<ManagerTables, ContentError> {
        self.checked_json(tables::MANAGER, ManagerTables::check)
    }

    fn sound(&self) -> Result<SoundContent, ContentError> {
        sound::read(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::data::files::{read_atari_disk, GameFiles};

    #[test]
    fn a_pack_loads_back_exactly_what_was_extracted() {
        let config = Config::default();
        let Ok(files) = GameFiles::load(&config.data_dir, Platform::Amiga) else {
            eprintln!("skipping: no originals");
            return;
        };
        let mut files = files;
        files.atari_disk = read_atari_disk(&config.data_dir).ok();
        let files = OriginalFiles::new(files, true, None).expect("the originals load");
        let originals = files.originals();
        let dir = std::env::temp_dir().join(format!("sb2-pack-{}", std::process::id()));
        let extracted = extract(&originals, &dir).expect("the pack is written");
        assert_eq!(extracted.platforms, [Platform::Amiga, Platform::Atari]);
        assert!(extracted.menus);
        assert!(extracted.sound);

        let pack = Pack::open(&dir).expect("the pack opens");
        for platform in [Platform::Amiga, Platform::Atari] {
            assert!(
                pack.match_graphics(platform).unwrap()
                    == originals.match_graphics(platform).unwrap(),
                "{platform:?} match graphics differ"
            );
        }
        assert!(pack.menu_graphics().unwrap() == originals.menu_graphics().unwrap());
        assert!(pack.intro_graphics().unwrap() == originals.intro_graphics().unwrap());
        assert_eq!(pack.menu_text().unwrap(), originals.menu_text().unwrap());
        assert_eq!(pack.intro_text().unwrap(), originals.intro_text().unwrap());
        assert_eq!(pack.lookups().unwrap(), originals.lookups().unwrap());
        assert_eq!(pack.teams().unwrap(), originals.teams().unwrap());
        assert_eq!(pack.animations().unwrap(), originals.animations().unwrap());
        assert_eq!(
            pack.match_start().unwrap(),
            originals.match_start().unwrap()
        );
        assert_eq!(
            pack.menu_tables().unwrap(),
            originals.menu_tables().unwrap()
        );
        assert_eq!(
            pack.manager_tables().unwrap(),
            originals.manager_tables().unwrap()
        );
        assert_eq!(pack.sound().unwrap(), originals.sound().unwrap());
        assert!(pack.intro_graphics().unwrap().is_some());
        std::fs::remove_dir_all(dir).unwrap();
    }
}

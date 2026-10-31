//! Graphics and text decoded from the original disk images and memory
//! dumps.

use super::{ContentError, ContentSource};
use crate::config::Platform;
use crate::data::amiga::{menu_ram, Overlays};
use crate::data::animations::AnimationTable;
use crate::data::files::AmigaFiles;
use crate::data::lookups::Lookups;
use crate::data::AtariRom;
use crate::game::roster::Teams;
use crate::game::start::StartTable;
use crate::graphics::Assets;
use crate::menu::assets::MenuAssets;
use crate::menu::intro::{IntroGraphics, IntroText};
use crate::menu::manager::data::ManagerTables;
use crate::menu::ram::MenuRam;
use crate::menu::strings::MenuText;
use crate::menu::tables::MenuTables;
use crate::sound::data::{SoundContent, SoundData, SoundTables};

/// The original files, loaded, with the menus' memory rebuilt from the
/// Amiga ones. Which files are loaded is the caller's choice.
pub struct OriginalFiles {
    pub rom: AtariRom,
    pub amiga: Option<AmigaFiles>,
    /// The menus' memory, if they are wanted (and there are Amiga files).
    pub menu_ram: Option<MenuRam>,
    /// The Amiga match dump for sound, when the rest of the Amiga files
    /// were not loaded.
    pub sound_dump: Option<Vec<u8>>,
    pub intro_dump: Option<Vec<u8>>,
}

impl OriginalFiles {
    pub fn new(
        rom: AtariRom,
        amiga: Option<AmigaFiles>,
        with_menus: bool,
        sound_dump: Option<Vec<u8>>,
        intro_dump: Option<Vec<u8>>,
    ) -> Result<Self, ContentError> {
        let menu_ram = match &amiga {
            Some(files) if with_menus => {
                let overlays = Self::overlays(files)?;
                Some(MenuRam::new(menu_ram(&files.match_dump, &overlays)))
            }
            _ => None,
        };
        Ok(Self {
            rom,
            amiga,
            menu_ram,
            sound_dump,
            intro_dump,
        })
    }

    fn overlays(files: &AmigaFiles) -> Result<Overlays<'_>, ContentError> {
        Overlays::new(&files.disk).ok_or(ContentError::Unreadable(
            "the Amiga disk has no overlay directory".into(),
        ))
    }

    /// The Amiga match dump, which holds the sound.
    pub fn match_dump(&self) -> Option<&[u8]> {
        match &self.amiga {
            Some(amiga) => Some(&amiga.match_dump),
            None => self.sound_dump.as_deref(),
        }
    }

    /// The files as a [`ContentSource`].
    pub fn originals(&self) -> Originals<'_> {
        Originals {
            rom: &self.rom,
            amiga: self
                .amiga
                .as_ref()
                .zip(self.menu_ram.as_ref())
                .map(|(files, menu_ram)| AmigaOriginals {
                    files,
                    overlays: Self::overlays(files).expect("checked when loaded"),
                    menu_ram,
                }),
            match_dump: self.match_dump(),
            intro_dump: self.intro_dump.as_deref(),
        }
    }
}

/// The original files, as a [`ContentSource`].
pub struct Originals<'a> {
    pub rom: &'a AtariRom,
    pub amiga: Option<AmigaOriginals<'a>>,
    /// The Amiga match dump, which holds the sound.
    pub match_dump: Option<&'a [u8]>,
    pub intro_dump: Option<&'a [u8]>,
}

/// The Amiga disk and dump, and the menus' memory rebuilt from them.
pub struct AmigaOriginals<'a> {
    pub files: &'a AmigaFiles,
    pub overlays: Overlays<'a>,
    pub menu_ram: &'a MenuRam,
}

const AMIGA_FILES: &str = "the Amiga disk and dump";
const AMIGA_DUMP: &str = "the Amiga dump";

impl Originals<'_> {
    fn amiga(&self) -> Result<&AmigaOriginals<'_>, ContentError> {
        self.amiga
            .as_ref()
            .ok_or(ContentError::Unavailable(AMIGA_FILES))
    }
}

impl ContentSource for Originals<'_> {
    fn match_graphics(&self, platform: Platform) -> Result<Assets, ContentError> {
        match platform {
            Platform::Atari => Ok(Assets::load_atari(self.rom)),
            Platform::Amiga => {
                let amiga = self.amiga()?;
                Ok(Assets::load_amiga(amiga.files))
            }
        }
    }

    fn menu_graphics(&self) -> Result<MenuAssets, ContentError> {
        let amiga = self.amiga()?;
        Ok(MenuAssets::load(amiga.menu_ram, &amiga.overlays)?)
    }

    fn intro_graphics(&self) -> Result<Option<IntroGraphics>, ContentError> {
        Ok(self.intro_dump.map(IntroGraphics::load).transpose()?)
    }

    fn menu_text(&self) -> Result<MenuText, ContentError> {
        Ok(MenuText::read(self.amiga()?.menu_ram))
    }

    fn intro_text(&self) -> Result<Option<IntroText>, ContentError> {
        Ok(self.intro_dump.map(IntroText::read))
    }

    fn lookups(&self) -> Result<Lookups, ContentError> {
        Ok(Lookups::read(self.rom))
    }

    fn teams(&self) -> Result<Teams, ContentError> {
        Ok(Teams::read(self.rom))
    }

    fn animations(&self) -> Result<AnimationTable, ContentError> {
        Ok(AnimationTable::read(self.rom))
    }

    fn match_start(&self) -> Result<StartTable, ContentError> {
        Ok(self.rom.start().table(self.rom.animations()))
    }

    fn menu_tables(&self) -> Result<MenuTables, ContentError> {
        Ok(MenuTables::read(self.amiga()?.menu_ram))
    }

    fn manager_tables(&self) -> Result<ManagerTables, ContentError> {
        Ok(ManagerTables::read(self.amiga()?.menu_ram))
    }

    fn sound(&self) -> Result<SoundContent, ContentError> {
        let dump = self
            .match_dump
            .ok_or(ContentError::Unavailable(AMIGA_DUMP))?;
        let read = |image: &[u8]| {
            SoundData::read(image)
                .map_err(|e| ContentError::Unreadable(format!("the sound data can't be read: {e}")))
        };
        Ok(SoundContent {
            tables: SoundTables::read(dump),
            game: read(dump)?,
            menus: self
                .amiga
                .as_ref()
                .map(|amiga| read(amiga.menu_ram.bytes()))
                .transpose()?,
            intro: self.intro_dump.map(read).transpose()?,
        })
    }
}

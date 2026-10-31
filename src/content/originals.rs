//! Graphics and text decoded from the original disk images and memory
//! dumps.

use super::{ContentError, ContentSource};
use crate::config::Platform;
use crate::data::amiga::Overlays;
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

/// The original files, as loaded.
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
    pub overlays: &'a Overlays<'a>,
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
            Platform::Atari => Ok(Assets::load(self.rom, None, Platform::Atari)),
            Platform::Amiga => {
                let amiga = self.amiga()?;
                Ok(Assets::load(self.rom, Some(amiga.files), Platform::Amiga))
            }
        }
    }

    fn menu_graphics(&self) -> Result<MenuAssets, ContentError> {
        let amiga = self.amiga()?;
        Ok(MenuAssets::load(amiga.menu_ram, amiga.overlays)?)
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
            SoundData::read(image).map_err(|e| ContentError::Missing(format!("sound data ({e})")))
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

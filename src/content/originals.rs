//! Graphics and text decoded from the original disk images and the
//! memory loaded from them.

use super::{ContentError, ContentSource};
use crate::config::Platform;
use crate::data::amiga::{menu_ram, Overlays};
use crate::data::animations::AnimationTable;
use crate::data::files::{AmigaFiles, GameFiles};
use crate::data::lookups::Lookups;
use crate::data::Rom;
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
/// Amiga's if the menus are wanted.
pub struct OriginalFiles {
    /// The match's data, from the intro's memory.
    pub rom: Rom,
    /// The Atari ST disk, for the ST's graphics.
    pub atari_disk: Option<Vec<u8>>,
    pub amiga: AmigaFiles,
    pub menu_ram: Option<MenuRam>,
}

impl OriginalFiles {
    pub fn new(files: GameFiles, with_menus: bool) -> Self {
        let GameFiles { amiga, atari_disk } = files;
        let menu_ram = with_menus
            .then(|| MenuRam::new(menu_ram(&amiga.match_memory, &Self::overlays(&amiga))));
        Self {
            rom: Rom::new(&amiga.intro_memory),
            atari_disk,
            amiga,
            menu_ram,
        }
    }

    fn overlays(files: &AmigaFiles) -> Overlays<'_> {
        Overlays::new(&files.disk).expect("checked when loaded")
    }

    /// The files as a [`ContentSource`].
    pub fn originals(&self) -> Originals<'_> {
        Originals {
            rom: &self.rom,
            atari_disk: self.atari_disk.as_deref(),
            amiga: &self.amiga,
            overlays: Self::overlays(&self.amiga),
            menu_ram: self.menu_ram.as_ref(),
        }
    }
}

/// The original files, as a [`ContentSource`].
pub struct Originals<'a> {
    pub rom: &'a Rom,
    pub atari_disk: Option<&'a [u8]>,
    pub amiga: &'a AmigaFiles,
    pub overlays: Overlays<'a>,
    pub menu_ram: Option<&'a MenuRam>,
}

const ATARI_DISK: &str = "the Atari ST disk";
const MENU_RAM: &str = "the menus' memory";

impl Originals<'_> {
    fn menu_ram(&self) -> Result<&MenuRam, ContentError> {
        self.menu_ram.ok_or(ContentError::Unavailable(MENU_RAM))
    }
}

impl ContentSource for Originals<'_> {
    fn match_graphics(&self, platform: Platform) -> Result<Assets, ContentError> {
        match platform {
            Platform::Atari => {
                let disk = self
                    .atari_disk
                    .ok_or(ContentError::Unavailable(ATARI_DISK))?;
                Ok(Assets::load_atari(disk))
            }
            Platform::Amiga => Ok(Assets::load_amiga(self.amiga)),
        }
    }

    fn menu_graphics(&self) -> Result<MenuAssets, ContentError> {
        Ok(MenuAssets::load(self.menu_ram()?, &self.overlays)?)
    }

    fn intro_graphics(&self) -> Result<Option<IntroGraphics>, ContentError> {
        Ok(Some(IntroGraphics::load(&self.amiga.intro_memory)?))
    }

    fn menu_text(&self) -> Result<MenuText, ContentError> {
        Ok(MenuText::read(self.menu_ram()?))
    }

    fn intro_text(&self) -> Result<Option<IntroText>, ContentError> {
        Ok(Some(IntroText::read(&self.amiga.intro_memory)))
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
        Ok(MenuTables::read(self.menu_ram()?))
    }

    fn manager_tables(&self) -> Result<ManagerTables, ContentError> {
        Ok(ManagerTables::read(self.menu_ram()?))
    }

    fn sound(&self) -> Result<SoundContent, ContentError> {
        let memory = &self.amiga.match_memory;
        let read = |image: &[u8]| {
            SoundData::read(image)
                .map_err(|e| ContentError::Unreadable(format!("the sound data can't be read: {e}")))
        };
        Ok(SoundContent {
            tables: SoundTables::read(memory),
            game: read(memory)?,
            menus: self.menu_ram.map(|ram| read(ram.bytes())).transpose()?,
            intro: Some(read(&self.amiga.intro_memory)?),
        })
    }
}

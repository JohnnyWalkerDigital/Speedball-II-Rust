//! Graphics decoded from the original disk images and memory dumps.

use super::{ContentError, GraphicsSource};
use crate::config::Platform;
use crate::data::amiga::Overlays;
use crate::data::files::AmigaFiles;
use crate::data::AtariRom;
use crate::graphics::Assets;
use crate::menu::assets::MenuAssets;
use crate::menu::intro::IntroGraphics;
use crate::menu::ram::MenuRam;

/// The original files, as loaded.
pub struct Originals<'a> {
    pub rom: &'a AtariRom,
    pub amiga: Option<AmigaOriginals<'a>>,
    pub intro_dump: Option<&'a [u8]>,
}

/// The Amiga disk and dump, and the menus' memory rebuilt from them.
pub struct AmigaOriginals<'a> {
    pub files: &'a AmigaFiles,
    pub overlays: &'a Overlays<'a>,
    pub menu_ram: &'a MenuRam,
}

const AMIGA_FILES: &str = "the Amiga disk and dump";

impl GraphicsSource for Originals<'_> {
    fn match_graphics(&self, platform: Platform) -> Result<Assets, ContentError> {
        match platform {
            Platform::Atari => Ok(Assets::load(self.rom, None, Platform::Atari)),
            Platform::Amiga => {
                let amiga = self
                    .amiga
                    .as_ref()
                    .ok_or(ContentError::Unavailable(AMIGA_FILES))?;
                Ok(Assets::load(self.rom, Some(amiga.files), Platform::Amiga))
            }
        }
    }

    fn menu_graphics(&self) -> Result<MenuAssets, ContentError> {
        let amiga = self
            .amiga
            .as_ref()
            .ok_or(ContentError::Unavailable(AMIGA_FILES))?;
        Ok(MenuAssets::load(amiga.menu_ram, amiga.overlays)?)
    }

    fn intro_graphics(&self) -> Result<Option<IntroGraphics>, ContentError> {
        Ok(self.intro_dump.map(IntroGraphics::load).transpose()?)
    }
}

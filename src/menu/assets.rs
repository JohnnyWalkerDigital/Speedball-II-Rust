//! The menus' pictures and fonts.

use super::ram::{addr, MenuRam};
use crate::data::amiga::{overlay, Overlays};
use crate::graphics::ilbm::{self, IlbmError, Picture};
use crate::graphics::indexed::IndexedImage;
use crate::graphics::planar::{decode_indexed_sprites, Layout, SpriteFormat};

pub struct MenuAssets {
    /// The steel "SB II" backdrop behind every menu (`splash_backdrop`).
    pub backdrop: Picture,
    pub victory: Picture,
    pub defeat: Picture,
    pub knockout_win: Picture,
    pub league_win: Picture,
    pub promotion: Picture,
    pub cup_win: Picture,
    pub big_font: Vec<IndexedImage>,
    pub title_top_font: Vec<IndexedImage>,
    pub title_bottom_font: Vec<IndexedImage>,
    pub white_font: Vec<IndexedImage>,
    /// 8x8 letters, digits and table borders for the panel screens.
    pub orange_font: Vec<IndexedImage>,
}

/// A font for `draw_string`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SmallFont {
    TitleTop,
    TitleBottom,
    White,
    Orange,
}

impl MenuAssets {
    pub fn load(ram: &MenuRam, overlays: &Overlays) -> Result<Self, IlbmError> {
        let font = |address, length, size| {
            decode_indexed_sprites(
                ram.slice(address, length),
                SpriteFormat {
                    width: size,
                    height: size,
                    planes: 5,
                    layout: Layout::AmigaPlanar,
                },
            )
        };
        Ok(Self {
            backdrop: ilbm::decode(&ram.bytes()[overlay::MENU_ADDRESS..])?,
            victory: ilbm::decode(overlays.get(overlay::VICTORY))?,
            defeat: ilbm::decode(overlays.get(overlay::DEFEAT))?,
            knockout_win: ilbm::decode(overlays.get(overlay::KNOCKOUT_WIN))?,
            league_win: ilbm::decode(overlays.get(overlay::LEAGUE_WIN))?,
            promotion: ilbm::decode(overlays.get(overlay::PROMOTION))?,
            cup_win: ilbm::decode(overlays.get(overlay::CUP_WIN))?,
            big_font: font(addr::SPRITES_MENU_FONT, addr::MENU_FONT_LENGTH, 16),
            title_top_font: font(addr::SPRITES_FONTS_TITLES_TOP, addr::TITLES_TOP_LENGTH, 8),
            title_bottom_font: font(
                addr::SPRITES_FONTS_TITLES_BOTTOM,
                addr::TITLES_BOTTOM_LENGTH,
                8,
            ),
            white_font: font(addr::SPRITES_FONTS_WHITE, addr::WHITE_LENGTH, 8),
            orange_font: font(addr::SPRITES_FONT_ORANGE, addr::ORANGE_LENGTH, 8),
        })
    }

    pub fn small_font(&self, font: SmallFont) -> &[IndexedImage] {
        match font {
            SmallFont::TitleTop => &self.title_top_font,
            SmallFont::TitleBottom => &self.title_bottom_font,
            SmallFont::White => &self.white_font,
            SmallFont::Orange => &self.orange_font,
        }
    }
}

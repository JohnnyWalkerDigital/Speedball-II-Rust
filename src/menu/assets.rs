//! The menus' pictures and fonts.

use super::ram::{addr, MenuRam};
use crate::data::amiga::{overlay, Overlays};
use crate::data::bytes::BigEndian;
use crate::graphics::ilbm::{self, IlbmError, Picture};
use crate::graphics::indexed::{ColorWords, IndexedImage};
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
    pub manager: ManagerGraphics,
}

/// The manager, transfer and gym screens' graphics.
pub struct ManagerGraphics {
    /// `palette_management_a`.
    pub palette: ColorWords,
    /// 16x16 tiles, and the 20x13 maps of them behind each screen.
    pub tiles: Vec<IndexedImage>,
    pub manager_map: Vec<u8>,
    pub gym_map: Vec<u8>,
    /// 16x16 pieces of the 48x48 faces, six faces to a row of 18.
    pub faces: Vec<IndexedImage>,
    /// 16x16 pieces of the group and team logos, five to a row of 15.
    pub logos: Vec<IndexedImage>,
    /// The gym's upgrade lights (unlit, then lit).
    pub lights: Vec<IndexedImage>,
    /// 32x32 keypad buttons.
    pub buttons: Vec<IndexedImage>,
    /// 32x32 pieces of the armour model.
    pub armour: Vec<IndexedImage>,
    /// Small green letters and digits (`draw_string_pixel`).
    pub green_font: Vec<IndexedImage>,
    /// The screen titles' big letters.
    pub title_font: Vec<IndexedImage>,
    /// The cash readout's digits (they follow the title letters).
    pub cash_font: Vec<IndexedImage>,
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
            manager: ManagerGraphics::load(ram),
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

impl ManagerGraphics {
    fn load(ram: &MenuRam) -> Self {
        let sheet = |address: usize, count: usize, size: usize, planes: usize| {
            let format = SpriteFormat {
                width: size,
                height: size,
                planes,
                layout: Layout::AmigaPlanar,
            };
            decode_indexed_sprites(
                ram.slice(address, count * format.bytes_per_sprite()),
                format,
            )
        };
        let bytes = ram.bytes();
        Self {
            palette: std::array::from_fn(|i| bytes.u16_at(addr::PALETTE_MANAGEMENT_A + i * 2)),
            tiles: sheet(addr::SPRITES_MGMT_BACKGROUND, 173, 16, 5),
            manager_map: ram.slice(addr::MANAGER_TRANSFER_TILE_MAP, 260).to_vec(),
            gym_map: ram.slice(addr::GYM_TILE_MAP, 260).to_vec(),
            faces: sheet(addr::SPRITES_PLAYER_FACES, 270, 16, 4),
            logos: sheet(addr::SPRITES_GROUP_LOGO, 45, 16, 5),
            lights: sheet(addr::SPRITES_MGMT_LIGHTS, 16, 16, 5),
            buttons: sheet(addr::SPRITES_MGMT_BUTTONS, 32, 32, 5),
            armour: sheet(addr::SPRITES_MGMT_ARMOUR, 37, 32, 5),
            green_font: sheet(addr::SPRITES_FONTS_SMALL_GREEN, 36, 8, 5),
            title_font: sheet(addr::SPRITES_FONTS_MGR_XFER_GYM, 28, 8, 5),
            cash_font: sheet(addr::SPRITES_FONTS_CASH, 36, 8, 5),
        }
    }
}

//! Which file in a pack holds each of the game's images.

use super::pack::{Pack, PackWriter, COLUMNS};
use super::{ContentError, GraphicsSource};
use crate::config::Platform;
use crate::graphics::Assets;
use crate::menu::assets::{ManagerGraphics, MenuAssets};
use crate::menu::intro::IntroGraphics;

fn match_dir(platform: Platform) -> &'static str {
    match platform {
        Platform::Amiga => "match/amiga",
        Platform::Atari => "match/atari",
    }
}

pub fn write_match(w: &mut PackWriter, assets: &Assets) -> Result<(), ContentError> {
    let dir = match_dir(assets.platform);
    let name = |file: &str| format!("{dir}/{file}");
    w.image(&name("pitch"), &assets.pitch)?;
    w.sheet(&name("pitch_tiles"), &assets.pitch_tiles)?;
    w.sheet(&name("panels"), &assets.panels)?;
    w.sheet(&name("panel_font"), &assets.panel_font)?;
    w.sheet(&name("digits"), &assets.digits)?;
    w.image(&name("status_bar"), &assets.status_bar)?;
    w.sheet(&name("entities"), &assets.entities)?;
    w.sheet(&name("players"), &assets.players)?;
    w.sheet(&name("players_blue"), &assets.players_blue)?;
    w.palette(dir, &assets.palette);
    Ok(())
}

fn read_match(pack: &Pack, platform: Platform) -> Result<Assets, ContentError> {
    let dir = match_dir(platform);
    let name = |file: &str| format!("{dir}/{file}");
    Ok(Assets {
        platform,
        pitch: pack.image(&name("pitch"))?,
        pitch_tiles: pack.sheet(&name("pitch_tiles"))?,
        panels: pack.sheet(&name("panels"))?,
        panel_font: pack.sheet(&name("panel_font"))?,
        digits: pack.sheet(&name("digits"))?,
        status_bar: pack.image(&name("status_bar"))?,
        entities: pack.sheet(&name("entities"))?,
        players: pack.sheet(&name("players"))?,
        players_blue: pack.sheet(&name("players_blue"))?,
        palette: pack.palette(dir)?,
    })
}

const MENU_PICTURES: [&str; 7] = [
    "backdrop",
    "victory",
    "defeat",
    "knockout_win",
    "league_win",
    "promotion",
    "cup_win",
];
const MENU_FONTS: [&str; 5] = ["big", "title_top", "title_bottom", "white", "orange"];
/// The manager screens' sheets, and their sprites to a row: the faces
/// and logos are 48x48 pictures cut into 16x16 pieces, stored a row of
/// pictures at a time, so they are kept whole.
const MANAGER_SHEETS: [(&str, usize); 9] = [
    ("tiles", COLUMNS),
    ("faces", 18),
    ("logos", 15),
    ("lights", COLUMNS),
    ("buttons", COLUMNS),
    ("armour", COLUMNS),
    ("green_font", COLUMNS),
    ("title_font", COLUMNS),
    ("cash_font", COLUMNS),
];

pub fn write_menus(w: &mut PackWriter, menus: &MenuAssets) -> Result<(), ContentError> {
    let pictures = [
        &menus.backdrop,
        &menus.victory,
        &menus.defeat,
        &menus.knockout_win,
        &menus.league_win,
        &menus.promotion,
        &menus.cup_win,
    ];
    for (name, picture) in MENU_PICTURES.iter().zip(pictures) {
        w.picture(&format!("menus/pictures/{name}"), picture)?;
    }
    let fonts = [
        &menus.big_font,
        &menus.title_top_font,
        &menus.title_bottom_font,
        &menus.white_font,
        &menus.orange_font,
    ];
    // The fonts draw on the backdrop, in its colours.
    for (name, font) in MENU_FONTS.iter().zip(fonts) {
        let name = format!("menus/fonts/{name}");
        w.indexed_sheet(&name, font, COLUMNS, &menus.backdrop.palette)?;
    }
    let manager = &menus.manager;
    let sheets = [
        &manager.tiles,
        &manager.faces,
        &manager.logos,
        &manager.lights,
        &manager.buttons,
        &manager.armour,
        &manager.green_font,
        &manager.title_font,
        &manager.cash_font,
    ];
    for ((name, columns), sheet) in MANAGER_SHEETS.iter().zip(sheets) {
        let name = format!("menus/manager/{name}");
        w.indexed_sheet(&name, sheet, *columns, &manager.palette)?;
    }
    w.color_words("menus/manager", &manager.palette);
    Ok(())
}

fn read_menus(pack: &Pack) -> Result<MenuAssets, ContentError> {
    let picture = |name: &str| pack.picture(&format!("menus/pictures/{name}"));
    let font = |name: &str| pack.indexed_sheet(&format!("menus/fonts/{name}"));
    let sheet = |name: &str| pack.indexed_sheet(&format!("menus/manager/{name}"));
    Ok(MenuAssets {
        backdrop: picture("backdrop")?,
        victory: picture("victory")?,
        defeat: picture("defeat")?,
        knockout_win: picture("knockout_win")?,
        league_win: picture("league_win")?,
        promotion: picture("promotion")?,
        cup_win: picture("cup_win")?,
        big_font: font("big")?,
        title_top_font: font("title_top")?,
        title_bottom_font: font("title_bottom")?,
        white_font: font("white")?,
        orange_font: font("orange")?,
        manager: ManagerGraphics {
            palette: pack.color_words("menus/manager")?,
            tiles: sheet("tiles")?,
            faces: sheet("faces")?,
            logos: sheet("logos")?,
            lights: sheet("lights")?,
            buttons: sheet("buttons")?,
            armour: sheet("armour")?,
            green_font: sheet("green_font")?,
            title_font: sheet("title_font")?,
            cash_font: sheet("cash_font")?,
        },
    })
}

pub fn write_intro(w: &mut PackWriter, intro: &IntroGraphics) -> Result<(), ContentError> {
    w.picture("intro/title", &intro.title)?;
    w.picture("intro/backdrop", &intro.backdrop)?;
    // The text is typed on the arena backdrop.
    w.indexed_sheet("intro/font", &intro.font, COLUMNS, &intro.backdrop.palette)
}

fn read_intro(pack: &Pack) -> Result<IntroGraphics, ContentError> {
    Ok(IntroGraphics {
        title: pack.picture("intro/title")?,
        backdrop: pack.picture("intro/backdrop")?,
        font: pack.indexed_sheet("intro/font")?,
    })
}

impl GraphicsSource for Pack {
    fn match_graphics(&self, platform: Platform) -> Result<Assets, ContentError> {
        read_match(self, platform)
    }

    fn menu_graphics(&self) -> Result<MenuAssets, ContentError> {
        read_menus(self)
    }

    fn intro_graphics(&self) -> Result<Option<IntroGraphics>, ContentError> {
        if !self.has("intro/title") {
            return Ok(None);
        }
        read_intro(self).map(Some)
    }
}

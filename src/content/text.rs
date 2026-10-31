//! The menus' and the intro's words, as JSON files in a pack.

use super::pack::{Pack, PackWriter};
use super::ContentError;
use crate::menu::intro::IntroText;
use crate::menu::strings::MenuText;

const MENUS: &str = "text/menus";
const INTRO: &str = "text/intro";

pub fn write_menus(w: &mut PackWriter, text: &MenuText) -> Result<(), ContentError> {
    w.json(MENUS, text)
}

pub fn write_intro(w: &mut PackWriter, text: &IntroText) -> Result<(), ContentError> {
    w.json(INTRO, text)
}

/// The menus' text, checked to be usable.
pub fn read_menus(pack: &Pack) -> Result<MenuText, ContentError> {
    let path = pack.json_path(MENUS);
    let text: MenuText = pack
        .json(MENUS)?
        .ok_or_else(|| ContentError::Missing(path.display().to_string()))?;
    text.check()
        .map_err(|problems| ContentError::invalid(&path, problems))?;
    Ok(text)
}

pub fn read_intro(pack: &Pack) -> Result<Option<IntroText>, ContentError> {
    pack.json(INTRO)
}

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
    pack.checked_json(MENUS, MenuText::check)
}

pub fn read_intro(pack: &Pack) -> Result<IntroText, ContentError> {
    pack.checked_json(INTRO, |_| Ok(()))
}

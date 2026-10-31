//! The game's tables, as JSON files in a pack.

use super::pack::Pack;
use super::ContentError;
use crate::data::animations::AnimationTable;
use crate::game::start::StartTable;

pub const LOOKUPS: &str = "tables/match";
pub const TEAMS: &str = "tables/teams";
pub const ANIMATIONS: &str = "tables/animations";
pub const START: &str = "tables/start";
pub const SQUADS: &str = "tables/squads";
pub const MANAGER: &str = "tables/manager_screens";

/// The scripts, checked to lay out.
pub fn read_animations(pack: &Pack) -> Result<AnimationTable, ContentError> {
    pack.checked_json(ANIMATIONS, |table: &AnimationTable| {
        table.clone().lay_out().map(|_| ())
    })
}

/// The starting records, checked against the pack's scripts.
pub fn read_start(pack: &Pack) -> Result<StartTable, ContentError> {
    let animations = read_animations(pack)?
        .lay_out()
        .map_err(|e| ContentError::invalid(&pack.json_path(ANIMATIONS), e))?;
    pack.checked_json(START, |table: &StartTable| {
        table.resolve(&animations).map(|_| ())
    })
}

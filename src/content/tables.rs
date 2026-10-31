//! The game's tables, as JSON files in a pack.

use super::pack::{Pack, PackWriter};
use super::ContentError;
use crate::data::animations::AnimationTable;
use crate::data::lookups::Lookups;
use crate::game::roster::Teams;
use crate::game::start::StartTable;
use crate::menu::manager::data::ManagerTables;
use crate::menu::tables::MenuTables;

const LOOKUPS: &str = "tables/match";
const TEAMS: &str = "tables/teams";
const ANIMATIONS: &str = "tables/animations";
const START: &str = "tables/start";
const MENUS: &str = "tables/squads";
const MANAGER: &str = "tables/manager_screens";

pub fn write_lookups(w: &mut PackWriter, lookups: &Lookups) -> Result<(), ContentError> {
    w.json(LOOKUPS, lookups)
}

pub fn read_lookups(pack: &Pack) -> Result<Lookups, ContentError> {
    pack.checked_json(LOOKUPS, Lookups::check)
}

pub fn write_teams(w: &mut PackWriter, teams: &Teams) -> Result<(), ContentError> {
    w.json(TEAMS, teams)
}

pub fn read_teams(pack: &Pack) -> Result<Teams, ContentError> {
    pack.checked_json(TEAMS, Teams::check)
}

pub fn write_animations(w: &mut PackWriter, table: &AnimationTable) -> Result<(), ContentError> {
    w.json(ANIMATIONS, table)
}

/// The scripts, checked to lay out.
pub fn read_animations(pack: &Pack) -> Result<AnimationTable, ContentError> {
    pack.checked_json(ANIMATIONS, |table: &AnimationTable| {
        table.clone().lay_out().map(|_| ())
    })
}

pub fn write_start(w: &mut PackWriter, table: &StartTable) -> Result<(), ContentError> {
    w.json(START, table)
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

pub fn write_menus(w: &mut PackWriter, tables: &MenuTables) -> Result<(), ContentError> {
    w.json(MENUS, tables)
}

pub fn read_menus(pack: &Pack) -> Result<MenuTables, ContentError> {
    pack.checked_json(MENUS, MenuTables::check)
}

pub fn write_manager(w: &mut PackWriter, tables: &ManagerTables) -> Result<(), ContentError> {
    w.json(MANAGER, tables)
}

pub fn read_manager(pack: &Pack) -> Result<ManagerTables, ContentError> {
    pack.checked_json(MANAGER, ManagerTables::check)
}

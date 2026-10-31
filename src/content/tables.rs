//! The game's tables, as JSON files in a pack.

use super::pack::{Pack, PackWriter};
use super::ContentError;
use crate::data::lookups::Lookups;
use crate::game::roster::Teams;
use crate::menu::manager::data::ManagerTables;
use crate::menu::tables::MenuTables;

const LOOKUPS: &str = "tables/match";
const TEAMS: &str = "tables/teams";
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

//! The numbers behind the menus' games: the squads Brutal Deluxe and
//! player two start with, the transfer market, the gym's prices and the
//! results of matches the player is not in.
//!
//! Read from the original's memory ([`MenuTables::read`]) or an asset pack.

use serde::{Deserialize, Serialize};

use super::ram::{addr, MenuRam};
use crate::game::league::ResultsTable;
use crate::game::squad::{MarketPlayer, Squad, Upgrade, MARKET_SIZE, SQUAD_SIZE};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MenuTables {
    /// Brutal Deluxe's squad as the game starts, in formation order:
    /// defence (with the goalkeeper), midfield, attack, substitutes. Skills
    /// are aggression, attack, defence, speed, throwing, power, stamina
    /// and intelligence; each game sets them to its own starting value.
    pub squad: Squad,
    /// Player two's.
    pub squad_two: Squad,
    /// The names and faces both squads get back at the main menu.
    pub base_identities: [Identity; SQUAD_SIZE],
    /// Who can be bought, in the order the transfer screen shows them.
    pub market: Vec<MarketPlayer>,
    /// The gym's eight upgrades: the skill each raises (0 aggression to 7
    /// intelligence) and its price.
    pub upgrades: [Upgrade; 8],
    /// Four tables of results for matches without Brutal Deluxe, one
    /// picked at random for each: by home team, then away team.
    pub other_results: Vec<ResultsTable>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Identity {
    pub name: String,
    pub face: u16,
}

impl MenuTables {
    pub fn read(ram: &MenuRam) -> Self {
        let results = ram.slice(addr::SCORES_TABLE, addr::SCORES_TABLE_LENGTH);
        Self {
            squad: ram.squad(addr::TEAM_INFO_TABLE),
            squad_two: ram.squad(addr::TEAM_INFO_TABLE_OTHER),
            base_identities: ram
                .base_identities()
                .map(|(name, face)| Identity { name, face }),
            market: ram.market(),
            upgrades: ram.upgrades(),
            other_results: results
                .chunks_exact(16 * 16 * 2)
                .map(|table| {
                    std::array::from_fn(|team1| {
                        std::array::from_fn(|team2| {
                            let at = (team1 * 16 + team2) * 2;
                            [table[at], table[at + 1]]
                        })
                    })
                })
                .collect(),
        }
    }

    /// Finds what the menus could not use.
    pub fn check(&self) -> Result<(), String> {
        let mut problems = Vec::new();
        if self.market.len() != MARKET_SIZE {
            problems.push(format!("market must have {MARKET_SIZE} players"));
        }
        if self.other_results.len() != 4 {
            problems.push("other_results must have 4 tables".to_string());
        }
        if self.upgrades.iter().any(|u| u.skill >= 8) {
            problems.push("each upgrade's skill must be 0 to 7".to_string());
        }
        if problems.is_empty() {
            Ok(())
        } else {
            Err(problems.join("\n"))
        }
    }

    /// The names and faces as `restore_identities` takes them.
    pub fn identities(&self) -> Vec<(String, u16)> {
        self.base_identities
            .iter()
            .map(|i| (i.name.clone(), i.face))
            .collect()
    }
}

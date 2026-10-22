//! The manager's side of the game: a squad of twelve, the transfer market,
//! and the gym (`buy_for_player`, `keypad_transfer_fn_buy`,
//! `keypad_manager_fn_substitute`, `update_market`, `get_player_value`,
//! `get_max_boost`).

use super::rng::Rng;

/// Players in a squad: nine on the pitch, then three substitutes.
pub const SQUAD_SIZE: usize = 12;
/// Players on the transfer market.
pub const MARKET_SIZE: usize = 18;
/// Face numbers above this are market players (bought or for sale).
pub const LAST_SQUAD_FACE: u16 = 0x45;

/// The eight skills in the order the stats records hold them.
pub type SkillBytes = [u8; 8];

/// One player record (`player_1_defense_1` and friends, and their stats).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SquadPlayer {
    pub name: String,
    /// Which face to draw, and whether they came from the market.
    pub face: u16,
    /// Stats byte 2: 0 goalkeeper, 1 defence, 2 midfield, 3 and up attack.
    /// For market players, the row they can play in (1 to 3).
    pub position: u8,
    pub skills: SkillBytes,
}

impl SquadPlayer {
    pub fn is_from_market(&self) -> bool {
        self.face > LAST_SQUAD_FACE
    }
}

/// The squad, in formation order: three each of defence (with the
/// goalkeeper), midfield and attack, then three substitutes.
pub type Squad = [SquadPlayer; SQUAD_SIZE];

/// A player on the market (`players_market`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MarketPlayer {
    pub player: SquadPlayer,
    /// The management round at which they next come on or off the market.
    pub change_time: i16,
    pub buyable: bool,
}

/// The competition being played, for its effect on prices and limits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Competition {
    Knockout,
    League,
    Cup,
    TwoPlayers,
}

/// What prices and limits depend on.
#[derive(Clone, Copy, Debug)]
pub struct Context {
    pub competition: Competition,
    /// `misc_flags` bit 0: "TEAM MANAGER".
    pub manager_mode: bool,
    /// Brutal Deluxe are in division 1 (placing 8 or better).
    pub in_division_1: bool,
}

/// The eight gym upgrades: which skill each improves and its price per
/// player (`upgrades_table`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Upgrade {
    pub skill: usize,
    pub price: u16,
}

/// Why a purchase did not happen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// "NOT ENOUGH CASH".
    NotEnoughCash,
    /// "MAXIMUM BOOST ALREADY".
    MaximumBoost,
}

/// `get_player_value`: ten credits' worth of each upgrade for every ten
/// points above 100, and a premium for market players.
pub fn player_value(player: &SquadPlayer, upgrades: &[Upgrade], context: Context) -> u16 {
    let base = upgrades.iter().fold(0u16, |value, upgrade| {
        let above = u16::from(player.skills[upgrade.skill]).wrapping_sub(100);
        value.wrapping_add((above / 10).wrapping_mul(upgrade.price))
    });
    if !player.is_from_market() {
        return base;
    }
    // Half again in the cup or for a team manager, a quarter otherwise.
    let premium = if context.competition == Competition::Cup || context.manager_mode {
        base >> 1
    } else {
        base >> 2
    };
    base.wrapping_add(premium)
}

/// `get_max_boost`: the gym only takes a skill to 30 above the weakest
/// group's average (rounded down to ten), with a ceiling depending on the
/// game.
pub fn max_boost(squad: &Squad, context: Context) -> u8 {
    let weakest = group_averages(squad)
        .iter()
        .map(average_8)
        .min()
        .unwrap_or(0);
    let mut max = weakest / 10 * 10 + 30;
    let ceiling = if context.manager_mode && context.in_division_1 {
        max += 10;
        230
    } else if context.manager_mode {
        max += 10;
        ceiling_for(context.competition)
    } else {
        ceiling_for(context.competition)
    };
    max.min(ceiling) as u8
}

fn ceiling_for(competition: Competition) -> u16 {
    match competition {
        Competition::Knockout => 240,
        _ => 220,
    }
}

/// `generate_team_stats`: per group, each skill averaged over its three
/// players (rounded down).
pub fn group_averages(squad: &Squad) -> [SkillBytes; 4] {
    std::array::from_fn(|group| {
        std::array::from_fn(|skill| {
            let sum: u16 = squad[group * 3..group * 3 + 3]
                .iter()
                .map(|p| u16::from(p.skills[skill]))
                .sum();
            (sum / 3) as u8
        })
    })
}

/// `generate_team_total_stats`: each skill averaged over the four groups.
pub fn team_average(squad: &Squad) -> SkillBytes {
    let groups = group_averages(squad);
    std::array::from_fn(|skill| {
        let sum: u16 = groups.iter().map(|g| u16::from(g[skill])).sum();
        (sum >> 2) as u8
    })
}

/// `average_8`: the mean of eight skills, rounded down.
pub fn average_8(skills: &SkillBytes) -> u16 {
    skills.iter().map(|&s| u16::from(s)).sum::<u16>() >> 3
}

/// The money and squad the gym and market work on.
pub struct Manager<'a> {
    pub squad: &'a mut Squad,
    pub cash: &'a mut u16,
    pub market: &'a [MarketPlayer],
    pub context: Context,
}

impl Manager<'_> {
    /// `buy_for_player`: ten more in one skill, if it is under the limit.
    /// A player bought from the market can only get back to the skills he
    /// had there.
    pub fn buy_for_player(&mut self, slot: usize, upgrade: Upgrade) -> Result<(), Refusal> {
        if upgrade.price as i16 > *self.cash as i16 {
            return Err(Refusal::NotEnoughCash);
        }
        let player = &self.squad[slot];
        let limit = if player.is_from_market() {
            market_player(self.market, player.face).map_or(0, |m| m.player.skills[upgrade.skill])
        } else {
            max_boost(self.squad, self.context)
        };
        let skill = &mut self.squad[slot].skills[upgrade.skill];
        if limit <= *skill {
            return Err(Refusal::MaximumBoost);
        }
        *skill = skill.wrapping_add(10);
        *self.cash = self.cash.wrapping_sub(upgrade.price);
        Ok(())
    }

    /// `buy_for_group`: the same for the three players of a group, if the
    /// cash covers all three. Players at their limit are skipped quietly.
    pub fn buy_for_group(&mut self, group: usize, upgrade: Upgrade) -> Result<(), Refusal> {
        self.buy_for_several(group * 3..group * 3 + 3, upgrade)
    }

    /// `buy_for_team`: the same for all twelve.
    pub fn buy_for_team(&mut self, upgrade: Upgrade) -> Result<(), Refusal> {
        self.buy_for_several(0..SQUAD_SIZE, upgrade)
    }

    fn buy_for_several(
        &mut self,
        slots: std::ops::Range<usize>,
        upgrade: Upgrade,
    ) -> Result<(), Refusal> {
        let total = upgrade.price.wrapping_mul(slots.len() as u16);
        if total as i16 > *self.cash as i16 {
            return Err(Refusal::NotEnoughCash);
        }
        for slot in slots {
            // A refusal for one player does not stop the rest.
            let _ = self.buy_for_player(slot, upgrade);
        }
        Ok(())
    }
}

/// `find_market_player`: the market record a bought player came from.
pub fn market_player(market: &[MarketPlayer], face: u16) -> Option<&MarketPlayer> {
    market.iter().find(|m| m.player.face == face)
}

/// `keypad_manager_fn_substitute`: swap two squad places. Players from the
/// market only play in their own position's row, or on the bench; the
/// squad's own players go anywhere. Places keep their positions; the
/// players' names, faces and skills move. Returns whether they swapped.
pub fn substitute(squad: &mut Squad, market: &[MarketPlayer], a: usize, b: usize) -> bool {
    let both_own = !squad[a].is_from_market() && !squad[b].is_from_market();
    let allowed = if both_own || a / 3 == b / 3 {
        true
    } else {
        // One of them must be on the bench...
        let (bench, other_row) = if a / 3 == 3 {
            (a, b / 3)
        } else if b / 3 == 3 {
            (b, a / 3)
        } else {
            return false;
        };
        // ...and a market player there can only go to his own row.
        let player = &squad[bench];
        if !player.is_from_market() {
            true
        } else {
            market_player(market, player.face)
                .is_some_and(|m| usize::from(m.player.position).wrapping_sub(1) == other_row)
        }
    };
    if allowed {
        let (first, second) = (squad[a].clone(), squad[b].clone());
        for (slot, from) in [(a, second), (b, first)] {
            let target = &mut squad[slot];
            target.name = from.name;
            target.face = from.face;
            target.skills = from.skills;
        }
    }
    allowed
}

/// The transfer market and the management rounds that drive it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Market {
    pub players: Vec<MarketPlayer>,
    /// `num_management_rounds`: manager screens visited so far.
    pub rounds: u16,
    /// `num_buyable_players`.
    pub buyable_count: u16,
}

impl Market {
    pub fn new(players: Vec<MarketPlayer>) -> Self {
        Self {
            players,
            rounds: 0,
            buyable_count: 0,
        }
    }

    /// `init_market`: nobody for sale, each player due to change in one
    /// to eight rounds.
    pub fn init(&mut self, rng: &mut Rng) {
        self.rounds = 0;
        self.buyable_count = 0;
        for market in &mut self.players {
            market.change_time = (rng.next() & 7) as i16 + 1;
            market.buyable = false;
        }
    }

    /// `update_market`: players due this round come on or go off the
    /// market. At most three are for sale; which ones depends on their
    /// value and the division.
    pub fn update(&mut self, rng: &mut Rng, upgrades: &[Upgrade], context: Context) {
        for market in &mut self.players {
            if market.change_time != self.rounds as i16 {
                continue;
            }
            if market.buyable {
                market.change_time += (rng.next() & 7) as i16 + 1;
                market.buyable = false;
                self.buyable_count -= 1;
                continue;
            }
            market.change_time += (rng.next() & 3) as i16 + 1;
            if self.buyable_count == 3 {
                continue;
            }
            let wanted = context.competition == Competition::Cup || {
                let value = player_value(&market.player, upgrades, context);
                if context.in_division_1 {
                    value > 1250 && (!context.manager_mode || value > 1500)
                } else {
                    value <= 1500 && (context.manager_mode || value <= 1250)
                }
            };
            if wanted {
                market.buyable = true;
                self.buyable_count += 1;
            }
        }
    }

    /// `keypad_transfer_fn_buy`: buy market player `index` into squad place
    /// `slot`, paying the difference in value. He must play in that row,
    /// unless he goes on the bench. Returns whether the deal went through.
    pub fn buy(
        &mut self,
        index: usize,
        slot: usize,
        squad: &mut Squad,
        cash: &mut u16,
        upgrades: &[Upgrade],
        context: Context,
    ) -> bool {
        if self.buyable_count == 0 {
            return false;
        }
        let market = &self.players[index];
        let cost = player_value(&market.player, upgrades, context).wrapping_sub(player_value(
            &squad[slot],
            upgrades,
            context,
        ));
        if cost as i16 > *cash as i16 {
            return false;
        }
        let row = slot / 3 + 1;
        if row != 4 && usize::from(market.player.position) != row {
            return false;
        }
        *cash = cash.wrapping_sub(cost);
        let bought = market.player.clone();
        let market = &mut self.players[index];
        market.buyable = false;
        market.change_time = -1;
        self.buyable_count -= 1;
        let target = &mut squad[slot];
        target.name = bought.name;
        target.face = bought.face;
        target.skills = bought.skills;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const UPGRADES: [Upgrade; 8] = [
        Upgrade { skill: 0, price: 5 },
        Upgrade {
            skill: 1,
            price: 10,
        },
        Upgrade {
            skill: 2,
            price: 10,
        },
        Upgrade {
            skill: 3,
            price: 15,
        },
        Upgrade {
            skill: 4,
            price: 10,
        },
        Upgrade {
            skill: 5,
            price: 10,
        },
        Upgrade {
            skill: 6,
            price: 15,
        },
        Upgrade {
            skill: 7,
            price: 15,
        },
    ];

    fn player(face: u16, position: u8, skill: u8) -> SquadPlayer {
        SquadPlayer {
            name: format!("P{face}"),
            face,
            position,
            skills: [skill; 8],
        }
    }

    fn squad(skill: u8) -> Squad {
        std::array::from_fn(|i| player(i as u16 * 3, (i / 3).min(3) as u8, skill))
    }

    const KNOCKOUT: Context = Context {
        competition: Competition::Knockout,
        manager_mode: false,
        in_division_1: false,
    };

    #[test]
    fn value_counts_tens_above_a_hundred() {
        // 50 above 100 in each skill: five steps of every price (90).
        assert_eq!(player_value(&player(0, 1, 150), &UPGRADES, KNOCKOUT), 450);
        // Market players cost a quarter more, or half in the cup.
        assert_eq!(player_value(&player(108, 1, 150), &UPGRADES, KNOCKOUT), 562);
        let cup = Context {
            competition: Competition::Cup,
            ..KNOCKOUT
        };
        assert_eq!(player_value(&player(108, 1, 150), &UPGRADES, cup), 675);
    }

    #[test]
    fn the_gym_stops_thirty_above_the_weakest_group() {
        let mut squad = squad(100);
        assert_eq!(max_boost(&squad, KNOCKOUT), 130);
        squad[0].skills = [200; 8];
        // The defence average rises but midfield is still the weakest.
        assert_eq!(max_boost(&squad, KNOCKOUT), 130);
        let squad = self::squad(250);
        assert_eq!(max_boost(&squad, KNOCKOUT), 240);
        let league = Context {
            competition: Competition::League,
            ..KNOCKOUT
        };
        assert_eq!(max_boost(&squad, league), 220);
    }

    #[test]
    fn buying_raises_a_skill_until_the_limit() {
        let mut squad = squad(100);
        let mut cash = 1000;
        let mut manager = Manager {
            squad: &mut squad,
            cash: &mut cash,
            market: &[],
            context: KNOCKOUT,
        };
        let boots = UPGRADES[3];
        for _ in 0..3 {
            assert_eq!(manager.buy_for_player(4, boots), Ok(()));
        }
        assert_eq!(manager.buy_for_player(4, boots), Err(Refusal::MaximumBoost));
        assert_eq!(squad[4].skills[3], 130);
        assert_eq!(cash, 1000 - 3 * 15);
    }

    #[test]
    fn groups_need_cash_for_all_three() {
        let mut squad = squad(100);
        let mut cash = 20;
        let mut manager = Manager {
            squad: &mut squad,
            cash: &mut cash,
            market: &[],
            context: KNOCKOUT,
        };
        assert_eq!(
            manager.buy_for_group(1, UPGRADES[1]),
            Err(Refusal::NotEnoughCash)
        );
        assert_eq!(manager.buy_for_group(1, UPGRADES[0]), Ok(()));
        assert_eq!(cash, 5);
        assert!(squad[3..6].iter().all(|p| p.skills[0] == 110));
    }

    #[test]
    fn market_players_keep_to_their_row_or_the_bench() {
        let mut squad = squad(100);
        let market = vec![MarketPlayer {
            player: player(108, 2, 180),
            change_time: -1,
            buyable: false,
        }];
        squad[9] = market[0].player.clone();
        // From the bench into defence: not his row.
        assert!(!substitute(&mut squad, &market, 9, 1));
        // Into midfield: his row.
        assert!(substitute(&mut squad, &market, 9, 4));
        assert_eq!(squad[4].face, 108);
        // Own players swap anywhere.
        assert!(substitute(&mut squad, &market, 0, 7));
    }

    #[test]
    fn transfers_cost_the_difference_in_value() {
        let mut squad = squad(100);
        let mut market = Market::new(vec![MarketPlayer {
            player: player(108, 3, 120),
            change_time: 0,
            buyable: true,
        }]);
        market.buyable_count = 1;
        let mut cash = 1000;
        // A forward cannot go into defence.
        assert!(!market.buy(0, 1, &mut squad, &mut cash, &UPGRADES, KNOCKOUT));
        assert!(market.buy(0, 7, &mut squad, &mut cash, &UPGRADES, KNOCKOUT));
        // Worth 2 steps of every price (180), plus a quarter.
        assert_eq!(cash, 1000 - 225);
        assert_eq!(squad[7].face, 108);
        assert_eq!(market.buyable_count, 0);
        assert_eq!(market.players[0].change_time, -1);
    }
}

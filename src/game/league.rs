//! The league: two divisions of eight, each playing every other team in
//! the division twice a season (`game_league` and its helpers).
//!
//! Division 1 is index 0 and division 2 index 1, as the original stores
//! them. League placings (1 to 16) live in the teams' records.

use super::career::{compare_scores, Career, TeamId, LEAGUE_TEAMS};
use super::rng::Rng;

pub const DIVISION_SIZE: usize = 8;
pub const ROUNDS: usize = 7;
pub const MATCHES_PER_ROUND: usize = 4;

/// `match_permutations`: who plays whom each round, by position in the
/// division's fixture order (1-based, as stored).
const MATCH_PERMUTATIONS: [[u8; 8]; ROUNDS] = [
    [1, 2, 3, 4, 5, 6, 7, 8],
    [1, 4, 2, 3, 5, 8, 6, 7],
    [1, 8, 2, 7, 3, 6, 4, 5],
    [1, 3, 2, 4, 5, 7, 6, 8],
    [1, 5, 2, 6, 3, 7, 4, 8],
    [1, 7, 2, 8, 3, 5, 4, 6],
    [1, 6, 2, 5, 3, 8, 4, 7],
];

pub const BRUTAL_DELUXE: TeamId = TeamId::League(0);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct League {
    /// `division_1_b`, `division_2_b`: each division in fixture order.
    pub fixtures: [[TeamId; DIVISION_SIZE]; 2],
    /// `division_1`, `division_2`: each division in table order.
    pub tables: [[TeamId; DIVISION_SIZE]; 2],
    /// `results_div_1`, `results_div_2`: this round's scores.
    pub results: [[(i32, i32); MATCHES_PER_ROUND]; 2],
}

impl League {
    /// `league_new_game`: the league teams in their stored order, Brutal
    /// Deluxe bottom of division 2 and Super Nashwan top of division 1.
    pub fn new_game(career: &mut Career) -> Self {
        for i in 0..LEAGUE_TEAMS {
            career.record_mut(TeamId::League(i)).league_placing = (LEAGUE_TEAMS - i) as u8;
        }
        let division = |first: usize| std::array::from_fn(|i| TeamId::League(first + i));
        let fixtures = [division(DIVISION_SIZE), division(0)];
        Self {
            fixtures,
            tables: fixtures,
            results: [[(0, 0); MATCHES_PER_ROUND]; 2],
        }
    }

    /// Which division a placing is in.
    pub fn division_of(placing: u8) -> usize {
        usize::from(placing > DIVISION_SIZE as u8)
    }

    /// `shuffle_league` then `put_bd_first`, for both divisions: a new
    /// fixture order each half-season.
    pub fn shuffle_fixtures(&mut self, rng: &mut Rng) {
        for division in &mut self.fixtures {
            shuffle(division, rng, 1);
            put_first(division, BRUTAL_DELUXE);
        }
    }

    /// `set_league_teams`: the teams in a division's match this round.
    pub fn fixture(&self, division: usize, round: usize, game: usize) -> (TeamId, TeamId) {
        let pair = &MATCH_PERMUTATIONS[round][game * 2..game * 2 + 2];
        let team = |n: u8| self.fixtures[division][usize::from(n) - 1];
        (team(pair[0]), team(pair[1]))
    }

    /// `update_league_placings`: sort each division by league points, then
    /// goal difference, and number the placings 1 to 16 down the tables.
    pub fn update_placings(&mut self, career: &mut Career) {
        self.tables = self.fixtures;
        for table in &mut self.tables {
            while bubble_pass(table, career) {}
        }
        for (placing, &team) in self.tables.iter().flatten().enumerate() {
            career.record_mut(team).league_placing = placing as u8 + 1;
        }
    }

    /// `league_promote_relegate`: `up` (from division 2) and `down` (from
    /// division 1) swap places and divisions.
    pub fn promote_relegate(&mut self, down: TeamId, up: TeamId, career: &mut Career) {
        let (down_placing, up_placing) = (
            career.record(down).league_placing,
            career.record(up).league_placing,
        );
        for (team, placing) in [(down, up_placing), (up, down_placing)] {
            let record = career.record_mut(team);
            record.old_league_placing = record.league_placing;
            record.league_placing = placing;
        }
        replace(&mut self.fixtures[0], down, up);
        replace(&mut self.fixtures[1], up, down);
    }
}

/// One pass of `team_bubble_pass`: higher league points first, and on
/// equal points the better goal difference. Returns whether it swapped.
fn bubble_pass(table: &mut [TeamId; DIVISION_SIZE], career: &Career) -> bool {
    let mut swapped = false;
    for i in 0..DIVISION_SIZE - 1 {
        let (a, b) = (career.record(table[i]), career.record(table[i + 1]));
        let goal_difference =
            |r: &super::career::TeamRecord| r.points_for.wrapping_sub(r.points_against) as i16;
        let behind = match a.league_points.cmp(&b.league_points) {
            std::cmp::Ordering::Less => true,
            std::cmp::Ordering::Greater => false,
            std::cmp::Ordering::Equal => goal_difference(a) < goal_difference(b),
        };
        if behind {
            table.swap(i, i + 1);
            swapped = true;
        }
    }
    swapped
}

/// `shuffle_league` and friends: 32 swaps of random entries. `stride` is
/// the spacing of the entries swapped, for the cup's later rounds.
pub fn shuffle(teams: &mut [TeamId], rng: &mut Rng, stride: usize) {
    let mask = teams.len() / stride - 1;
    for _ in 0..32 {
        let random = rng.next();
        let a = (random as usize & 0xFFFF & mask) * stride;
        let b = ((random >> 16) as usize & mask) * stride;
        teams.swap(a, b);
    }
}

/// `put_bd_first`: if `team` is in the list, it swaps places with the
/// first entry.
pub fn put_first(teams: &mut [TeamId], team: TeamId) {
    if let Some(at) = teams.iter().position(|&t| t == team) {
        teams.swap(0, at);
    }
}

fn replace(teams: &mut [TeamId], from: TeamId, to: TeamId) {
    for slot in teams.iter_mut().filter(|t| **t == from) {
        *slot = to;
    }
}

/// `cup_league_match_not_bd`: matches without Brutal Deluxe are not
/// played but looked up in `scores_table`, four tables of results between
/// every pair of league teams, one picked at random.
pub struct ScoresTable {
    /// `[table][team1][team2]` = (score1, score2).
    scores: Vec<(u8, u8)>,
}

impl ScoresTable {
    pub fn new(bytes: &[u8]) -> Self {
        Self {
            scores: bytes
                .chunks_exact(2)
                .map(|pair| (pair[0], pair[1]))
                .collect(),
        }
    }

    pub fn result(&self, rng: &mut Rng, team1: TeamId, team2: TeamId) -> (i32, i32) {
        let table = (rng.next() & 3) as usize;
        let index = |team: TeamId| match team {
            TeamId::League(i) => i,
            TeamId::PlayerTwo => unreachable!("player 2 is never in a league"),
        };
        let (a, b) = self.scores[table * 256 + index(team1) * 16 + index(team2)];
        (i32::from(a), i32::from(b))
    }
}

/// Whether team 2 beat team 1, the way the cup and play-offs decide it.
pub fn team2_won(scores: (i32, i32)) -> bool {
    compare_scores(scores.1, scores.0).is_gt()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::career::TeamRecord;
    use crate::game::geometry::PerTeam;

    fn career() -> Career {
        Career::new([TeamRecord::default(); LEAGUE_TEAMS + 1])
    }

    #[test]
    fn a_new_league_starts_brutal_deluxe_at_the_bottom() {
        let mut career = career();
        let league = League::new_game(&mut career);
        assert_eq!(career.record(BRUTAL_DELUXE).league_placing, 16);
        assert_eq!(career.record(TeamId::League(15)).league_placing, 1);
        assert_eq!(league.fixtures[1][0], BRUTAL_DELUXE);
        assert_eq!(league.fixtures[0][7], TeamId::League(15));
    }

    #[test]
    fn every_team_meets_every_other_once_a_half_season() {
        let mut career = career();
        let league = League::new_game(&mut career);
        let mut met = std::collections::HashSet::new();
        for round in 0..ROUNDS {
            for game in 0..MATCHES_PER_ROUND {
                let (a, b) = league.fixture(1, round, game);
                assert!(met.insert((a.min_key(), b.min_key()).min((b.min_key(), a.min_key()))));
            }
        }
        assert_eq!(met.len(), 28);
    }

    #[test]
    fn tables_sort_by_points_then_goal_difference() {
        let mut career = career();
        let mut league = League::new_game(&mut career);
        let (a, b, c) = (TeamId::League(1), TeamId::League(2), TeamId::League(3));
        career.record_result(PerTeam::new(a, b), PerTeam::new(40, 10));
        career.record_result(PerTeam::new(c, BRUTAL_DELUXE), PerTeam::new(20, 10));
        league.update_placings(&mut career);
        assert_eq!(&league.tables[1][..2], [a, c]);
        assert_eq!(career.record(a).league_placing, 9);
        assert_eq!(career.record(c).league_placing, 10);
        // Division 1 is still in its stored order and placed 1 to 8.
        assert_eq!(career.record(league.tables[0][0]).league_placing, 1);
    }

    #[test]
    fn promotion_swaps_places_and_divisions() {
        let mut career = career();
        let mut league = League::new_game(&mut career);
        league.update_placings(&mut career);
        let (down, up) = (league.tables[0][7], league.tables[1][0]);
        league.promote_relegate(down, up, &mut career);
        assert_eq!(career.record(up).league_placing, 8);
        assert_eq!(career.record(up).old_league_placing, 9);
        assert_eq!(career.record(down).league_placing, 9);
        assert!(league.fixtures[0].contains(&up));
        assert!(league.fixtures[1].contains(&down));
    }

    #[test]
    fn shuffling_keeps_brutal_deluxe_first() {
        let mut career = career();
        let mut league = League::new_game(&mut career);
        league.shuffle_fixtures(&mut Rng::new(1, 2));
        assert_eq!(league.fixtures[1][0], BRUTAL_DELUXE);
        let mut sorted = league.fixtures[0];
        sorted.sort_by_key(|t| t.min_key());
        assert_eq!(sorted, std::array::from_fn(|i| TeamId::League(8 + i)));
    }

    impl TeamId {
        fn min_key(self) -> usize {
            match self {
                TeamId::League(i) => i,
                TeamId::PlayerTwo => 99,
            }
        }
    }
}

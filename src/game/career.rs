//! What carries over from match to match: money, the teams' records and the
//! last match's statistics (`cash_p1`, `match_cash_*`, the `team` records'
//! league fields, `our_team_for_stats`).

use std::cmp::Ordering;

use super::geometry::PerTeam;
use super::m68k::ext_divu_register;
use super::roster::Skills;
use super::statistics::TeamStatistics;

/// The 16 league teams, in the original's order.
pub const LEAGUE_TEAMS: usize = 16;

/// Which record a team keeps its results in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TeamId {
    League(usize),
    /// The second human in two-player games (`team_player_2`).
    PlayerTwo,
}

/// A team's results so far (`num_played` to `old_league_placing`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TeamRecord {
    pub played: u16,
    pub won: u16,
    pub drawn: u16,
    pub lost: u16,
    pub points_for: u16,
    pub points_against: u16,
    pub league_points: u16,
    pub league_placing: u8,
    pub old_league_placing: u8,
}

/// Prize money. Each match a team keeps what it picked up in coins, at
/// least `start` and at most its `cap`; whatever it fell short of the cap
/// is added to the next match's.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Money {
    /// `cash_p1`, `cash_p2`: money to spend in the gym.
    pub cash: PerTeam<u16>,
    /// `match_cash_max_p1`, `_p2`: the most each team can earn next match.
    pub cap: PerTeam<u16>,
    /// `match_cash_max_base`: the game mode's cap per match.
    pub cap_base: u16,
    /// `match_cash_start`: what a team earns even with no coins.
    pub start: u16,
}

/// The teams and statistics of the last match played to the end.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LastMatch {
    pub teams: PerTeam<TeamId>,
    pub statistics: PerTeam<TeamStatistics>,
}

#[derive(Clone, Debug)]
pub struct Career {
    pub money: Money,
    records: [TeamRecord; LEAGUE_TEAMS + 1],
    pub last_match: Option<LastMatch>,
    /// `player_1_player_stats_table`, `player_2_...`: each human squad's
    /// skills, kept between matches.
    pub squads: PerTeam<[Skills; SQUAD_SIZE]>,
}

/// Players in a squad: nine on the pitch and three substitutes.
pub const SQUAD_SIZE: usize = 12;

impl Career {
    /// The state at power-on: every variable zero, the records as stored.
    pub fn new(records: [TeamRecord; LEAGUE_TEAMS + 1]) -> Self {
        Self {
            money: Money::default(),
            records,
            last_match: None,
            squads: PerTeam::default(),
        }
    }

    pub fn record(&self, team: TeamId) -> &TeamRecord {
        &self.records[Self::slot(team)]
    }

    pub(crate) fn record_mut(&mut self, team: TeamId) -> &mut TeamRecord {
        &mut self.records[Self::slot(team)]
    }

    fn slot(team: TeamId) -> usize {
        match team {
            TeamId::League(index) => index,
            TeamId::PlayerTwo => LEAGUE_TEAMS,
        }
    }

    /// `main_menu`: both players start with 1100 and no statistics. The
    /// caps are left as the last game set them.
    pub fn start_main_menu(&mut self) {
        self.money.cash = PerTeam::new(1100, 1100);
        self.money.start = 0;
        self.last_match = None;
    }

    /// `init_player_stats`: every skill in a human squad set to `value`.
    pub fn init_squad(&mut self, side: super::geometry::Side, value: i32) {
        *self.squads.get_mut(side) = [Skills::uniform(value); SQUAD_SIZE];
    }

    /// A game mode's cap per match, for both teams.
    pub fn set_cash_cap(&mut self, base: u16) {
        self.money.cap_base = base;
        self.money.cap = PerTeam::new(base, base);
    }

    /// `clear_match_stats`: every team back to no games played.
    pub fn clear_records(&mut self) {
        for record in &mut self.records {
            *record = TeamRecord {
                league_placing: record.league_placing,
                old_league_placing: record.old_league_placing,
                ..TeamRecord::default()
            };
        }
    }

    /// The end of `play_match`: each team banks what it earned, within the
    /// limits, and its next cap grows by what it left on the table.
    pub fn settle_prize_money(&mut self, earned: PerTeam<i32>) {
        let money = &mut self.money;
        for (earned, cash, cap) in [
            (earned.team1, &mut money.cash.team1, &mut money.cap.team1),
            (earned.team2, &mut money.cash.team2, &mut money.cap.team2),
        ] {
            // Unsigned word comparisons, as `bhi` and `bls`.
            let prize = (earned as u16).max(money.start).min(*cap);
            *cash = cash.wrapping_add(prize);
            *cap = money.cap_base.wrapping_add(cap.wrapping_sub(prize));
        }
    }

    /// `update_match_stats`: a win is worth 10 league points and a draw 5,
    /// plus a point for every 10 scored.
    pub fn record_result(&mut self, teams: PerTeam<TeamId>, scores: PerTeam<i32>) {
        for team in [teams.team1, teams.team2] {
            let record = self.record_mut(team);
            record.old_league_placing = record.league_placing;
            record.played += 1;
        }
        let (winner, loser) = match compare_scores(scores.team1, scores.team2) {
            Ordering::Equal => {
                for team in [teams.team1, teams.team2] {
                    let record = self.record_mut(team);
                    record.drawn += 1;
                    record.league_points += 5;
                }
                (None, None)
            }
            Ordering::Greater => (Some(teams.team1), Some(teams.team2)),
            Ordering::Less => (Some(teams.team2), Some(teams.team1)),
        };
        if let (Some(winner), Some(loser)) = (winner, loser) {
            let record = self.record_mut(winner);
            record.won += 1;
            record.league_points += 10;
            self.record_mut(loser).lost += 1;
        }
        for (team, other, score) in [
            (teams.team1, teams.team2, scores.team1),
            (teams.team2, teams.team1, scores.team2),
        ] {
            // A tenth of the score.
            let points = ext_divu_register(score as i16, 10) as u16;
            let record = self.record_mut(team);
            record.league_points = record.league_points.wrapping_add(points);
            record.points_for = record.points_for.wrapping_add(points);
            let record = self.record_mut(other);
            record.points_against = record.points_against.wrapping_add(points);
        }
    }
}

/// How the original compares two scores: as unsigned 16-bit words. Scores
/// never go below zero in a match (a lit star costs no more than it
/// earned), so this only matters in principle.
pub fn compare_scores(a: i32, b: i32) -> Ordering {
    (a as u16).cmp(&(b as u16))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn career() -> Career {
        let mut career = Career::new([TeamRecord::default(); LEAGUE_TEAMS + 1]);
        career.start_main_menu();
        career
    }

    #[test]
    fn prize_money_is_capped_and_the_rest_carries_over() {
        let mut career = career();
        career.set_cash_cap(1100);
        career.settle_prize_money(PerTeam::new(400, 1500));
        assert_eq!(career.money.cash, PerTeam::new(1500, 2200));
        // Team 1 left 700 unearned; team 2 hit its cap.
        assert_eq!(career.money.cap, PerTeam::new(1800, 1100));
    }

    #[test]
    fn every_team_earns_at_least_the_starting_amount() {
        let mut career = career();
        career.set_cash_cap(650);
        career.money.start = 300;
        career.settle_prize_money(PerTeam::new(0, 100));
        assert_eq!(career.money.cash, PerTeam::new(1400, 1400));
        assert_eq!(career.money.cap, PerTeam::new(1000, 1000));
    }

    #[test]
    fn results_score_league_points() {
        let mut career = career();
        let teams = PerTeam::new(TeamId::League(0), TeamId::League(3));
        career.record_result(teams, PerTeam::new(57, 20));
        career.record_result(teams, PerTeam::new(10, 10));
        let winner = career.record(TeamId::League(0));
        assert_eq!((winner.played, winner.won, winner.drawn), (2, 1, 1));
        assert_eq!(winner.league_points, 10 + 5 + 5 + 1);
        assert_eq!((winner.points_for, winner.points_against), (6, 3));
        let loser = career.record(TeamId::League(3));
        assert_eq!((loser.lost, loser.league_points), (1, 2 + 5 + 1));
    }

    #[test]
    fn clearing_keeps_league_placings() {
        let mut career = career();
        career.records[2].league_placing = 4;
        career.record_result(
            PerTeam::new(TeamId::League(2), TeamId::League(5)),
            PerTeam::new(10, 0),
        );
        career.clear_records();
        let record = career.record(TeamId::League(2));
        assert_eq!(
            (record.played, record.won, record.league_placing),
            (0, 0, 4)
        );
        assert_eq!(record.old_league_placing, 4);
    }
}

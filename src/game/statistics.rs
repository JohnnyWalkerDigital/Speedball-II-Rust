//! The statistics shown after a match, gathered as `write_team_statistics`
//! adds up the players' counters.

use super::geometry::{PerTeam, Side};
use super::player::{MatchStats, PlayerId};
use super::squad::{Squad, SquadPlayer};
use super::state::Match;

/// One team's numbers for the statistics screen. Sums of the players'
/// byte counters, kept in 16 bits like the original's.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TeamStatistics {
    /// Goals, plus saves by the other team's goalkeeper.
    pub shots_at_goal: u16,
    pub goals_scored: u16,
    /// Saves by this team's goalkeeper.
    pub goals_saved: u16,
    /// Frames the ball was held, by anyone in the team.
    pub time_in_possession: u16,
    pub successful_tackles: u16,
    pub substitutions: u16,
    /// Frames the ball spent in the other team's half.
    pub time_in_opponents_half: u16,
    pub bonus_points: i16,
}

impl Match {
    pub fn statistics(&self) -> PerTeam<TeamStatistics> {
        PerTeam::new(
            self.team_statistics(Side::Team1),
            self.team_statistics(Side::Team2),
        )
    }

    /// A human squad after the match: injured players have been swapped to
    /// the bench, their skills rounded down, and that sticks (the original
    /// worked on the squad's own records).
    pub fn squad_after(&self, side: Side, before: &Squad) -> Squad {
        let persons = match side {
            Side::Team1 => &self.roster.persons_team1,
            Side::Team2 => &self.roster.persons_team2,
        };
        std::array::from_fn(|slot| {
            let person = &persons[slot];
            let skills = self.roster.attributes(person.attributes).skills.to_array();
            SquadPlayer {
                name: person.name.clone(),
                face: person.id as u16,
                position: before[slot].position,
                skills: skills.map(|s| s as u8),
            }
        })
    }

    /// What each team picked up from coins this match (`match_cash_p1`/`_p2`).
    pub fn credits_earned(&self) -> PerTeam<i32> {
        self.credits_earned
    }

    fn team_statistics(&self, side: Side) -> TeamStatistics {
        let team1 = side == Side::Team1;
        // Slot 0 of each team is its goalkeeper.
        let keeper = |team1: bool| &self.players[PlayerId::of(team1, 0).0].stats;
        let sum = |stat: fn(&MatchStats) -> u8| {
            PlayerId::team(team1)
                .map(|p| u16::from(stat(&self.players[p.0].stats)))
                .fold(0u16, u16::wrapping_add)
        };
        let goals_scored = sum(|s| s.goals_scored);
        TeamStatistics {
            shots_at_goal: goals_scored.wrapping_add(u16::from(keeper(!team1).goals_saved)),
            goals_scored,
            goals_saved: u16::from(keeper(team1).goals_saved),
            time_in_possession: sum(|s| s.time_in_possession),
            successful_tackles: sum(|s| s.successful_tackles),
            substitutions: sum(|s| s.substitutions),
            time_in_opponents_half: *self.time_in_opponents_half.get(side) as u16,
            bonus_points: *self.bonus_points.get(side) as i16,
        }
    }
}

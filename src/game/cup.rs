//! The cup: sixteen teams, knocked out over four rounds of two-legged ties.

use super::career::{TeamId, LEAGUE_TEAMS};
use super::league::{put_first, shuffle, BRUTAL_DELUXE};
use super::rng::Rng;

pub const ROUNDS: usize = 4;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cup {
    /// The draw. Each tie's winner takes the first team's slot.
    pub teams: [TeamId; LEAGUE_TEAMS],
    /// 0 for round one up to 3 for the final.
    pub round: usize,
}

impl Cup {
    /// The league teams in order, each half shuffled, with Brutal Deluxe
    /// drawn first.
    pub fn new(rng: &mut Rng) -> Self {
        let mut teams = std::array::from_fn(TeamId::League);
        let (first, second) = teams.split_at_mut(LEAGUE_TEAMS / 2);
        shuffle(first, rng, 1);
        put_first(first, BRUTAL_DELUXE);
        shuffle(second, rng, 1);
        Self { teams, round: 0 }
    }

    /// The redraw before rounds two and three, among the winners so far.
    pub fn redraw(&mut self, rng: &mut Rng) {
        match self.round {
            1 => {
                let (first, second) = self.teams.split_at_mut(LEAGUE_TEAMS / 2);
                shuffle(first, rng, 2);
                put_first(first, BRUTAL_DELUXE);
                shuffle(second, rng, 2);
            }
            2 => {
                shuffle(&mut self.teams, rng, 4);
                put_first(&mut self.teams, BRUTAL_DELUXE);
            }
            _ => {}
        }
    }

    /// The draw slots of each tie this round.
    pub fn ties(&self) -> impl Iterator<Item = (usize, usize)> {
        let gap = 1 << self.round;
        (0..LEAGUE_TEAMS / (2 * gap)).map(move |tie| (tie * 2 * gap, tie * 2 * gap + gap))
    }

    /// The second team won the tie: it moves into the first slot.
    pub fn second_team_won(&mut self, tie: (usize, usize)) {
        self.teams.swap(tie.0, tie.1);
    }

    pub fn is_finished(&self) -> bool {
        self.round == ROUNDS
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rounds_halve_the_field_down_to_a_final() {
        let mut cup = Cup::new(&mut Rng::new(3, 4));
        assert_eq!(cup.teams[0], BRUTAL_DELUXE);
        let sizes: Vec<_> = (0..ROUNDS)
            .map(|round| {
                cup.round = round;
                cup.ties().count()
            })
            .collect();
        assert_eq!(sizes, [8, 4, 2, 1]);
        cup.round = 3;
        assert_eq!(cup.ties().next(), Some((0, 8)));
    }

    #[test]
    fn redraws_only_move_the_winners() {
        let mut rng = Rng::new(5, 6);
        let mut cup = Cup::new(&mut rng);
        cup.round = 1;
        let winners: Vec<_> = cup.ties().flat_map(|(a, _)| [a]).collect();
        let before: Vec<_> = (0..LEAGUE_TEAMS).step_by(2).map(|i| cup.teams[i]).collect();
        let losers: Vec<_> = (1..LEAGUE_TEAMS).step_by(2).map(|i| cup.teams[i]).collect();
        cup.redraw(&mut rng);
        assert_eq!(winners, [0, 4, 8, 12]);
        let mut after: Vec<_> = (0..LEAGUE_TEAMS).step_by(2).map(|i| cup.teams[i]).collect();
        let mut sorted_before = before.clone();
        sorted_before.sort_by_key(|t| format!("{t:?}"));
        after.sort_by_key(|t| format!("{t:?}"));
        assert_eq!(after, sorted_before);
        let losers_after: Vec<_> = (1..LEAGUE_TEAMS).step_by(2).map(|i| cup.teams[i]).collect();
        assert_eq!(losers_after, losers);
        assert_eq!(cup.teams[0], BRUTAL_DELUXE);
    }
}

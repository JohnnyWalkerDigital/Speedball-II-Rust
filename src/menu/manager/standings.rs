//! The competition behind a visit to the manager screens: for the
//! "TABLE" key and for saving.

use crate::game::cup::Cup;
use crate::game::league::League;
use crate::game::save::Progress;
use crate::menu::saving::Snapshot;

/// The league or cup being played, and how far it has got.
pub enum Standings<'a> {
    None,
    League {
        league: &'a mut League,
        round: usize,
        game: usize,
        initial_dir: i32,
    },
    Cup {
        cup: &'a Cup,
        /// Which tie of the round, and its two places in the draw.
        tie_index: usize,
        tie: (usize, usize),
        aggregate: (i32, i32),
        initial_dir: i32,
    },
}

impl Standings<'_> {
    /// What a save records of the competition (`write_league_config`,
    /// `write_cup_config`, `write_teams`).
    pub(super) fn snapshot(&self) -> Option<Snapshot> {
        match self {
            Standings::None => None,
            Standings::League {
                league,
                round,
                game,
                initial_dir,
            } => Some(Snapshot {
                teams: league.saved_order(),
                initial_dir: *initial_dir,
                progress: Progress::League {
                    round: *round as u16,
                    game: *game as u16,
                },
            }),
            Standings::Cup {
                cup,
                tie_index,
                tie,
                aggregate,
                initial_dir,
            } => Some(Snapshot {
                teams: cup.teams.to_vec(),
                initial_dir: *initial_dir,
                progress: Progress::Cup {
                    round: cup.round as u16,
                    tie: *tie_index as u16,
                    ties: cup.ties().count() as u16,
                    // `cup_team_1` and `_2` are byte offsets into the draw.
                    slots: (tie.0 as u16 * 4, tie.1 as u16 * 4),
                    aggregate: (aggregate.0 as i16, aggregate.1 as i16),
                },
            }),
        }
    }
}

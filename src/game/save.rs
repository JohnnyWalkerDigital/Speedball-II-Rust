//! Saved games, byte for byte as the Amiga wrote them to a save disk.
//!
//! The structure is the same for both games apart from the magic and the
//! last few fields: 0x362 bytes for a league, 0x36C for a cup. Names and
//! teams are stored as offsets from fixed addresses in the original's
//! memory; turning them back into names is the caller's job.

use super::career::{TeamId, TeamRecord};

/// The space between teams' records in the original's memory, which saves
/// count teams by.
pub const TEAM_RECORD_SIZE: usize = 0x40;

/// A player as saved.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SavedPlayer {
    /// Offset of the name from the first of the original's name strings.
    pub name_offset: u32,
    pub face: u16,
    pub skills: [u8; 8],
}

/// A team's place and record.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SavedTeam {
    /// Offset of the team from Brutal Deluxe's record (0x40 per team).
    pub team_offset: u32,
    pub played: u16,
    pub won: u16,
    pub drawn: u16,
    pub lost: u16,
    pub points_for: u16,
    pub points_against: u16,
    pub league_points: u16,
    pub league_placing: u16,
    pub old_league_placing: u16,
}

impl SavedTeam {
    /// `team`'s record as saved.
    pub fn new(team: TeamId, record: &TeamRecord) -> Self {
        Self {
            team_offset: Self::offset_of(team),
            played: record.played,
            won: record.won,
            drawn: record.drawn,
            lost: record.lost,
            points_for: record.points_for,
            points_against: record.points_against,
            league_points: record.league_points,
            league_placing: u16::from(record.league_placing),
            old_league_placing: u16::from(record.old_league_placing),
        }
    }

    /// The record as it was saved.
    pub fn record(&self) -> TeamRecord {
        TeamRecord {
            played: self.played,
            won: self.won,
            drawn: self.drawn,
            lost: self.lost,
            points_for: self.points_for,
            points_against: self.points_against,
            league_points: self.league_points,
            league_placing: self.league_placing as u8,
            old_league_placing: self.old_league_placing as u8,
        }
    }

    /// The offset a league team is saved as. Player 2 is never saved.
    fn offset_of(team: TeamId) -> u32 {
        match team {
            TeamId::League(index) => (index * TEAM_RECORD_SIZE) as u32,
            TeamId::PlayerTwo => unreachable!("player 2 is never saved"),
        }
    }

    /// The league team this record is for.
    pub fn team(&self) -> TeamId {
        TeamId::League(self.team_offset as usize / TEAM_RECORD_SIZE)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SavedMarketPlayer {
    pub player: SavedPlayer,
    pub change_time: i16,
    pub buyable: u16,
}

/// Where the game had got to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Progress {
    /// The round, and the match within it.
    League { round: u16, game: u16 },
    /// The round, the tie, the number of ties, the tie's two slots (byte
    /// offsets into the draw) and the tie's aggregate so far.
    Cup {
        round: u16,
        tie: u16,
        ties: u16,
        slots: (u16, u16),
        aggregate: (i16, i16),
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SaveGame {
    pub cash: u16,
    pub caps: (u16, u16),
    pub initial_dir: u16,
    /// Bit 0 is "team manager".
    pub flags: u16,
    pub squad: Vec<SavedPlayer>,
    /// In the save's team order: division 2 then division 1 for a league,
    /// the draw for a cup.
    pub teams: Vec<SavedTeam>,
    pub management_rounds: u16,
    pub buyable_count: u16,
    pub market: Vec<SavedMarketPlayer>,
    pub progress: Progress,
}

pub const SQUAD: usize = 12;
pub const TEAMS: usize = 16;
pub const MARKET: usize = 18;
const SQUAD_AT: usize = 0xE;
const TEAMS_AT: usize = 0xB6;
const MARKET_AT: usize = 0x216;
const PROGRESS_AT: usize = 0x35E;
pub const LEAGUE_SIZE: usize = 0x362;
pub const CUP_SIZE: usize = 0x36C;
const LEAGUE_MAGIC: &[u8; 4] = b"leag";
const CUP_MAGIC: &[u8; 4] = b"cup ";

impl SaveGame {
    pub fn encode(&self) -> Vec<u8> {
        let size = match self.progress {
            Progress::League { .. } => LEAGUE_SIZE,
            Progress::Cup { .. } => CUP_SIZE,
        };
        let mut out = Writer(vec![0; size]);
        out.bytes(
            0,
            match self.progress {
                Progress::League { .. } => LEAGUE_MAGIC,
                Progress::Cup { .. } => CUP_MAGIC,
            },
        );
        for (i, value) in [
            self.cash,
            self.caps.0,
            self.caps.1,
            self.initial_dir,
            self.flags,
        ]
        .into_iter()
        .enumerate()
        {
            out.word(4 + i * 2, value);
        }
        let mut at = SQUAD_AT;
        for player in &self.squad {
            at = out.player(at, player);
        }
        let mut at = TEAMS_AT;
        for team in &self.teams {
            out.long(at, team.team_offset);
            for (i, value) in [
                team.played,
                team.won,
                team.drawn,
                team.lost,
                team.points_for,
                team.points_against,
                team.league_points,
                team.league_placing,
                team.old_league_placing,
            ]
            .into_iter()
            .enumerate()
            {
                out.word(at + 4 + i * 2, value);
            }
            at += 22;
        }
        out.word(MARKET_AT, self.management_rounds);
        out.word(MARKET_AT + 2, self.buyable_count);
        let mut at = MARKET_AT + 4;
        for market in &self.market {
            at = out.player(at, &market.player);
            out.word(at, market.change_time as u16);
            out.word(at + 2, market.buyable);
            at += 4;
        }
        match self.progress {
            Progress::League { round, game } => {
                out.word(PROGRESS_AT, round);
                out.word(PROGRESS_AT + 2, game);
            }
            Progress::Cup {
                round,
                tie,
                ties,
                slots,
                aggregate,
            } => {
                for (i, value) in [
                    round,
                    tie,
                    ties,
                    slots.0,
                    slots.1,
                    aggregate.0 as u16,
                    aggregate.1 as u16,
                ]
                .into_iter()
                .enumerate()
                {
                    out.word(PROGRESS_AT + i * 2, value);
                }
            }
        }
        out.0
    }

    /// Reads a save. `None` unless it starts with the right magic
    /// (the original treats a mismatch as a disk error).
    pub fn decode(bytes: &[u8], cup: bool) -> Option<Self> {
        let (magic, size) = if cup {
            (CUP_MAGIC, CUP_SIZE)
        } else {
            (LEAGUE_MAGIC, LEAGUE_SIZE)
        };
        if bytes.len() < size || &bytes[..4] != magic {
            return None;
        }
        let r = Reader(bytes);
        let squad = (0..SQUAD).map(|i| r.player(SQUAD_AT + i * 14)).collect();
        let teams = (0..TEAMS)
            .map(|i| {
                let at = TEAMS_AT + i * 22;
                let word = |n: usize| r.word(at + 4 + n * 2);
                SavedTeam {
                    team_offset: r.long(at),
                    played: word(0),
                    won: word(1),
                    drawn: word(2),
                    lost: word(3),
                    points_for: word(4),
                    points_against: word(5),
                    league_points: word(6),
                    league_placing: word(7),
                    old_league_placing: word(8),
                }
            })
            .collect();
        let market = (0..MARKET)
            .map(|i| {
                let at = MARKET_AT + 4 + i * 18;
                SavedMarketPlayer {
                    player: r.player(at),
                    change_time: r.word(at + 14) as i16,
                    buyable: r.word(at + 16),
                }
            })
            .collect();
        let progress = if cup {
            let word = |n: usize| r.word(PROGRESS_AT + n * 2);
            Progress::Cup {
                round: word(0),
                tie: word(1),
                ties: word(2),
                slots: (word(3), word(4)),
                aggregate: (word(5) as i16, word(6) as i16),
            }
        } else {
            Progress::League {
                round: r.word(PROGRESS_AT),
                game: r.word(PROGRESS_AT + 2),
            }
        };
        Some(Self {
            cash: r.word(4),
            caps: (r.word(6), r.word(8)),
            initial_dir: r.word(0xA),
            flags: r.word(0xC),
            squad,
            teams,
            management_rounds: r.word(MARKET_AT),
            buyable_count: r.word(MARKET_AT + 2),
            market,
            progress,
        })
    }
}

struct Writer(Vec<u8>);

impl Writer {
    fn bytes(&mut self, at: usize, bytes: &[u8]) {
        self.0[at..at + bytes.len()].copy_from_slice(bytes);
    }

    fn word(&mut self, at: usize, value: u16) {
        self.bytes(at, &value.to_be_bytes());
    }

    fn long(&mut self, at: usize, value: u32) {
        self.bytes(at, &value.to_be_bytes());
    }

    /// Writes a player and returns where the next field goes.
    fn player(&mut self, at: usize, player: &SavedPlayer) -> usize {
        self.long(at, player.name_offset);
        self.word(at + 4, player.face);
        self.bytes(at + 6, &player.skills);
        at + 14
    }
}

struct Reader<'a>(&'a [u8]);

impl Reader<'_> {
    fn word(&self, at: usize) -> u16 {
        u16::from_be_bytes([self.0[at], self.0[at + 1]])
    }

    fn long(&self, at: usize) -> u32 {
        u32::from_be_bytes(self.0[at..at + 4].try_into().expect("four bytes"))
    }

    fn player(&self, at: usize) -> SavedPlayer {
        SavedPlayer {
            name_offset: self.long(at),
            face: self.word(at + 4),
            skills: self.0[at + 6..at + 14].try_into().expect("eight skills"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(progress: Progress) -> SaveGame {
        let player = |i: u32| SavedPlayer {
            name_offset: i * 6,
            face: i as u16 * 3,
            skills: [100 + i as u8; 8],
        };
        SaveGame {
            cash: 1234,
            caps: (650, 700),
            initial_dir: 4,
            flags: 3,
            squad: (0..12).map(player).collect(),
            teams: (0..16)
                .map(|i| SavedTeam {
                    team_offset: i * 0x40,
                    played: i as u16,
                    won: 1,
                    drawn: 2,
                    lost: 3,
                    points_for: 40,
                    points_against: 50,
                    league_points: 60,
                    league_placing: i as u16 + 1,
                    old_league_placing: 16 - i as u16,
                })
                .collect(),
            management_rounds: 8,
            buyable_count: 2,
            market: (0..18)
                .map(|i| SavedMarketPlayer {
                    player: player(i + 20),
                    change_time: -1,
                    buyable: (i % 2) as u16,
                })
                .collect(),
            progress,
        }
    }

    #[test]
    fn a_league_save_round_trips_at_the_original_size() {
        let save = sample(Progress::League { round: 3, game: 1 });
        let bytes = save.encode();
        assert_eq!(bytes.len(), LEAGUE_SIZE);
        assert_eq!(&bytes[..4], b"leag");
        // Cash, then the first player's name offset and face.
        assert_eq!(&bytes[4..6], &1234u16.to_be_bytes());
        assert_eq!(&bytes[0xE..0x14], &[0, 0, 0, 0, 0, 0]);
        assert_eq!(SaveGame::decode(&bytes, false), Some(save));
        assert_eq!(SaveGame::decode(&bytes, true), None, "not a cup");
    }

    #[test]
    fn a_cup_save_round_trips() {
        let save = sample(Progress::Cup {
            round: 2,
            tie: 0,
            ties: 2,
            slots: (0, 16),
            aggregate: (30, 20),
        });
        let bytes = save.encode();
        assert_eq!(bytes.len(), CUP_SIZE);
        assert_eq!(SaveGame::decode(&bytes, true), Some(save));
    }
}

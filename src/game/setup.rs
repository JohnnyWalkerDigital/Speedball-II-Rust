//! Building the initial state for a match: what the menus set up before
//! `play_match`.

use super::career::BRUTAL_DELUXE_INDEX;
use super::geometry::PerTeam;
use super::player::{Player, PlayerId};
use super::rng::Rng;
use super::roster::{Attributes, Person, Roster, Skills, Team};
use super::squad;
use super::text::{TextId, TextPanel};
use crate::config::{Config, GameMode, TEAM_COUNT};
use crate::data::Rom;

/// Everything `Match::new` needs.
pub struct MatchSetup {
    pub kind: MatchKind,
    pub players: Vec<Player>,
    pub roster: Roster,
    pub team1: Team,
    pub team2: Team,
    pub coins_credits: i32,
    pub leg_duration: i32,
    pub min_credits: i32,
    pub max_credits: PerTeam<i32>,
    pub initial_dir_team1: i32,
    pub computer: PerTeam<bool>,
    pub rng: Rng,
}

/// How a game mode's matches differ from a whole match between two teams.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MatchKind {
    /// The leg the clock starts on: 0 for a whole match, 1 for the second
    /// half only, and past full time for a match that never ends.
    pub first_leg: i32,
    /// Whether team 2 is on the pitch.
    pub team2_on_pitch: bool,
    /// Whether the reverse-controls token can appear.
    pub reverse_token: bool,
    /// Whether coins keep appearing while team 2 is under its cap, as well
    /// as team 1.
    pub coins_for_team2: bool,
}

impl MatchKind {
    pub const KNOCKOUT: Self = Self {
        first_leg: 0,
        team2_on_pitch: true,
        reverse_token: false,
        coins_for_team2: false,
    };
    /// Brutal Deluxe alone, with no end.
    pub const PRACTICE: Self = Self {
        first_leg: 3,
        team2_on_pitch: false,
        ..Self::KNOCKOUT
    };
    pub const TWO_PLAYERS: Self = Self {
        reverse_token: true,
        coins_for_team2: true,
        ..Self::KNOCKOUT
    };
    /// The second half only.
    pub const DEMO: Self = Self {
        first_leg: 1,
        ..Self::KNOCKOUT
    };
}

/// What a game mode decides about a match.
pub struct MatchRules {
    pub kind: MatchKind,
    pub team1: Lineup,
    pub team2: Lineup,
    /// How long each half lasts, in seconds.
    pub leg_duration: i32,
    /// What each team has earned at kick-off.
    pub min_credits: i32,
    /// `coin_value`: what a coin is worth (doubled in the cup).
    pub coin_value: i32,
    /// No more coins appear once a team has earned this much.
    pub max_credits: PerTeam<i32>,
    /// Which teams the computer plays.
    pub computer: PerTeam<bool>,
    /// The initial playing direction: 0 has team 1 attacking up first.
    pub initial_dir_team1: i32,
}

/// A team and how its players' skills are set.
pub struct Lineup {
    pub team: Team,
    pub skills: SquadSkills,
}

pub enum SquadSkills {
    /// Every skill the same.
    Uniform(i32),
    /// A human manager's squad: each player's name, face and skills.
    Players(Box<squad::Squad>),
    /// The team's skill templates.
    FromTeam,
}

impl MatchRules {
    /// Team 1 is Brutal Deluxe on 100 in every skill, as `main_menu` sets
    /// them up for one player; team 2 plays itself.
    pub fn one_player(rom: &Rom, opponent: usize) -> Self {
        Self {
            kind: MatchKind::KNOCKOUT,
            team1: Lineup {
                team: league_team(rom, BRUTAL_DELUXE_INDEX),
                skills: SquadSkills::Uniform(squad::ONE_PLAYER_SKILL.into()),
            },
            team2: Lineup {
                team: league_team(rom, opponent),
                skills: SquadSkills::FromTeam,
            },
            leg_duration: HALF_LENGTH_SECONDS,
            min_credits: 0,
            coin_value: 100,
            max_credits: PerTeam::new(1100, 1100),
            computer: PerTeam::new(false, true),
            initial_dir_team1: 0,
        }
    }

    /// Practice: Brutal Deluxe alone on the pitch.
    pub fn practice(rom: &Rom) -> Self {
        Self {
            kind: MatchKind::PRACTICE,
            team1: Lineup {
                team: league_team(rom, BRUTAL_DELUXE_INDEX),
                skills: SquadSkills::Uniform(squad::ONE_PLAYER_SKILL.into()),
            },
            team2: Lineup {
                team: league_team(rom, 1),
                skills: SquadSkills::Uniform(squad::ONE_PLAYER_SKILL.into()),
            },
            leg_duration: HALF_LENGTH_SECONDS,
            min_credits: 0,
            coin_value: 100,
            max_credits: PerTeam::new(10_000, 10_000),
            computer: PerTeam::new(false, false),
            initial_dir_team1: 0,
        }
    }

    /// Two players: "PLAYER 1" against "PLAYER 2", all skills 170.
    pub fn two_players(rom: &Rom, initial_dir_team1: i32) -> Self {
        let texts = TextPanel::new();
        let mut team1 = league_team(rom, BRUTAL_DELUXE_INDEX);
        team1.full_name = texts.text(TextId::Player1Full).clone();
        team1.short_name = texts.text(TextId::Player1Short).clone();
        Self {
            kind: MatchKind::TWO_PLAYERS,
            team1: Lineup {
                team: team1,
                skills: SquadSkills::Uniform(squad::TWO_PLAYER_SKILL.into()),
            },
            team2: Lineup {
                team: rom.teams().player_two(),
                skills: SquadSkills::Uniform(squad::TWO_PLAYER_SKILL.into()),
            },
            leg_duration: HALF_LENGTH_SECONDS,
            min_credits: 0,
            coin_value: 100,
            max_credits: PerTeam::new(2000, 2000),
            computer: PerTeam::new(false, false),
            initial_dir_team1,
        }
    }

    /// The demo game: Fatal Justice against Turbo Hammers, 20 second halves.
    pub fn demo(rom: &Rom) -> Self {
        Self {
            kind: MatchKind::DEMO,
            team1: Lineup {
                team: league_team(rom, 14),
                skills: SquadSkills::FromTeam,
            },
            team2: Lineup {
                team: league_team(rom, 13),
                skills: SquadSkills::FromTeam,
            },
            leg_duration: 20,
            min_credits: 0,
            coin_value: 100,
            max_credits: PerTeam::new(0, 0),
            computer: PerTeam::new(true, true),
            initial_dir_team1: 0,
        }
    }
}

const HALF_LENGTH_SECONDS: i32 = 90;

/// One of the 16 league teams, in the original's order.
pub fn league_team(rom: &Rom, index: usize) -> Team {
    rom.teams().league(index)
}

impl MatchSetup {
    /// A match from the menus, seeded with `seed`.
    pub fn new(rom: &Rom, rules: MatchRules, seed: u64) -> Self {
        let mut seeds = SplitMix64(seed);
        let rng = Rng::new(seeds.next_u31(), seeds.next_u31());
        Self::assemble(rom, rules, rng)
    }

    /// A match straight from the command line (`--mode` and friends).
    pub fn from_config(rom: &Rom, config: &Config) -> Self {
        let mut seeds = SplitMix64(config.seed);
        let rng = Rng::new(seeds.next_u31(), seeds.next_u31());
        let rules = match config.mode.unwrap_or(GameMode::Knockout) {
            GameMode::Knockout => knockout(rom, &mut seeds, config),
            GameMode::Practice => practice(rom, config),
            GameMode::TwoPlayers => two_players(rom, config),
            GameMode::Demo => demo(rom, config),
        };
        Self::assemble(rom, rules, rng)
    }

    fn assemble(rom: &Rom, rules: MatchRules, rng: Rng) -> Self {
        let mut players = rom.start().players();
        let mut roster = rom.start().roster();
        set_skills(
            &rules.team1,
            &mut roster.attributes_team1,
            &mut roster.persons_team1,
        );
        set_skills(
            &rules.team2,
            &mut roster.attributes_team2,
            &mut roster.persons_team2,
        );
        if !rules.kind.team2_on_pitch {
            for id in PlayerId::team(false) {
                players[id.0].entity.flags.set_invisible(true);
            }
        }
        Self {
            kind: rules.kind,
            players,
            roster,
            team1: rules.team1.team,
            team2: rules.team2.team,
            coins_credits: rules.coin_value,
            leg_duration: rules.leg_duration,
            min_credits: rules.min_credits,
            max_credits: rules.max_credits,
            initial_dir_team1: rules.initial_dir_team1,
            computer: rules.computer,
            rng,
        }
    }
}

fn set_skills(lineup: &Lineup, attributes: &mut [Attributes], persons: &mut [Person]) {
    match &lineup.skills {
        &SquadSkills::Uniform(value) => {
            for member in attributes {
                member.skills = Skills::uniform(value);
            }
        }
        SquadSkills::Players(players) => {
            // Each squad place keeps its own attribute record.
            for ((member, person), player) in attributes.iter_mut().zip(persons).zip(players.iter())
            {
                member.skills = Skills::from_array(player.skills.map(i32::from));
                person.name = player.name.clone();
                person.id = i32::from(player.face);
            }
        }
        SquadSkills::FromTeam => lineup.team.apply_skills_to(attributes),
    }
}

// -- Command-line modes ---------------------------------------------------------

/// Two random teams, both on the computer until someone touches the
/// controls.
fn knockout(rom: &Rom, seeds: &mut SplitMix64, config: &Config) -> MatchRules {
    let (team1, team2) = random_distinct_teams(seeds, config);
    MatchRules {
        kind: MatchKind::KNOCKOUT,
        team1: Lineup {
            team: league_team(rom, team1),
            skills: SquadSkills::FromTeam,
        },
        team2: Lineup {
            team: league_team(rom, team2),
            skills: SquadSkills::FromTeam,
        },
        leg_duration: HALF_LENGTH_SECONDS,
        min_credits: 0,
        coin_value: 100,
        max_credits: PerTeam::new(1100, 1100),
        computer: PerTeam::new(true, true),
        initial_dir_team1: 0,
    }
}

fn practice(rom: &Rom, config: &Config) -> MatchRules {
    MatchRules {
        team1: Lineup {
            team: league_team(rom, config.team1.unwrap_or(0)),
            skills: SquadSkills::Uniform(squad::ONE_PLAYER_SKILL.into()),
        },
        team2: Lineup {
            team: league_team(rom, config.team2.unwrap_or(1)),
            skills: SquadSkills::Uniform(squad::ONE_PLAYER_SKILL.into()),
        },
        max_credits: PerTeam::new(0, 0),
        ..MatchRules::practice(rom)
    }
}

fn two_players(rom: &Rom, config: &Config) -> MatchRules {
    // Team 1 is always "PLAYER 1" on 170s, so `--team1` changes nothing.
    let mut rules = MatchRules::two_players(rom, 0);
    if let Some(team2) = config.team2 {
        rules.team2.team = league_team(rom, team2);
    }
    rules
}

fn demo(rom: &Rom, config: &Config) -> MatchRules {
    let mut rules = MatchRules::demo(rom);
    if let Some(team1) = config.team1 {
        rules.team1.team = league_team(rom, team1);
    }
    if let Some(team2) = config.team2 {
        rules.team2.team = league_team(rom, team2);
    }
    rules
}

fn random_distinct_teams(seeds: &mut SplitMix64, config: &Config) -> (usize, usize) {
    loop {
        let team1 = config.team1.unwrap_or_else(|| seeds.below(TEAM_COUNT));
        let team2 = config.team2.unwrap_or_else(|| seeds.below(TEAM_COUNT));
        if team1 != team2 {
            return (team1, team2);
        }
    }
}

/// Small seedable generator for match setup (not part of the game logic).
struct SplitMix64(u64);

impl SplitMix64 {
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// A non-negative 31-bit value.
    fn next_u31(&mut self) -> u32 {
        (self.next_u64() >> 33) as u32
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next_u64() % n as u64) as usize
    }
}

//! Building the initial state for a match: what the menus set up before
//! `play_match`.

use super::geometry::PerTeam;
use super::player::{Player, PlayerId};
use super::rng::Rng;
use super::roster::{Attributes, Roster, Skills, Team};
use super::text::{TextId, TextPanel};
use crate::config::{Config, GameMode, TEAM_COUNT};
use crate::data::{addr, AtariRom};

/// Everything `Match::new` needs (the original `MatchParam`).
pub struct MatchSetup {
    pub mode: GameMode,
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

/// What a game mode decides about a match.
pub struct MatchRules {
    pub mode: GameMode,
    pub team1: Squad,
    pub team2: Squad,
    /// `match_half_length_seconds`.
    pub leg_duration: i32,
    /// `match_cash_start`: what each team has earned at kick-off.
    pub min_credits: i32,
    /// `coin_value`: what a coin is worth (doubled in the cup).
    pub coin_value: i32,
    /// `match_cash_max_p1` / `_p2`: no more coins appear once a team has
    /// earned this much.
    pub max_credits: PerTeam<i32>,
    /// Which teams the computer plays (`misc_flags` bits 0 and 1).
    pub computer: PerTeam<bool>,
    /// `initial_playing_direction`: 0 has team 1 attacking up first.
    pub initial_dir_team1: i32,
}

/// A team and how its players' skills are set.
pub struct Squad {
    pub team: Team,
    pub skills: SquadSkills,
}

pub enum SquadSkills {
    /// Every skill the same (`init_player_stats`).
    Uniform(i32),
    /// Each player's own skills (`player_1_player_stats_table`), in squad
    /// order: three each of defence, midfield, attack and substitutes.
    Given(Box<[Skills; 12]>),
    /// The team's skill templates (`populate_opposition_player_stats`).
    FromTeam,
}

impl MatchRules {
    /// Team 1 is Brutal Deluxe on 100 in every skill, as `main_menu` sets
    /// them up for one player; team 2 plays itself.
    pub fn one_player(rom: &AtariRom, opponent: usize) -> Self {
        Self {
            mode: GameMode::Knockout,
            team1: Squad {
                team: league_team(rom, BRUTAL_DELUXE),
                skills: SquadSkills::Uniform(100),
            },
            team2: Squad {
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

    /// `game_practice`: Brutal Deluxe alone on the pitch.
    pub fn practice(rom: &AtariRom) -> Self {
        Self {
            mode: GameMode::Practice,
            team1: Squad {
                team: league_team(rom, BRUTAL_DELUXE),
                skills: SquadSkills::Uniform(100),
            },
            team2: Squad {
                team: league_team(rom, 1),
                skills: SquadSkills::Uniform(100),
            },
            leg_duration: HALF_LENGTH_SECONDS,
            min_credits: 0,
            coin_value: 100,
            max_credits: PerTeam::new(10_000, 10_000),
            computer: PerTeam::new(false, false),
            initial_dir_team1: 0,
        }
    }

    /// `game_two_player`: "PLAYER 1" against "PLAYER 2", all skills 170.
    pub fn two_players(rom: &AtariRom, initial_dir_team1: i32) -> Self {
        let texts = TextPanel::new(rom);
        let mut team1 = league_team(rom, BRUTAL_DELUXE);
        team1.full_name = texts.text(TextId::Player1Full).clone();
        team1.short_name = texts.text(TextId::Player1Short).clone();
        Self {
            mode: GameMode::TwoPlayers,
            team1: Squad {
                team: team1,
                skills: SquadSkills::Uniform(170),
            },
            team2: Squad {
                team: Team::read(rom, addr::TEAM_PLAYER2),
                skills: SquadSkills::Uniform(170),
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
    pub fn demo(rom: &AtariRom) -> Self {
        Self {
            mode: GameMode::Demo,
            team1: Squad {
                team: league_team(rom, 14),
                skills: SquadSkills::FromTeam,
            },
            team2: Squad {
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

/// The player's team in one-player games.
pub const BRUTAL_DELUXE: usize = 0;
const HALF_LENGTH_SECONDS: i32 = 90;

/// One of the 16 league teams, in the original's order.
pub fn league_team(rom: &AtariRom, index: usize) -> Team {
    Team::read(rom, addr::TEAMS + index as i32 * Team::SIZE)
}

impl MatchSetup {
    /// A match from the menus, seeded with `seed`.
    pub fn new(rom: &AtariRom, rules: MatchRules, seed: u64) -> Self {
        let mut seeds = SplitMix64(seed);
        let rng = Rng::new(seeds.next_u31(), seeds.next_u31());
        Self::assemble(rom, rules, rng)
    }

    /// A match straight from the command line (`--mode` and friends).
    pub fn from_config(rom: &AtariRom, config: &Config) -> Self {
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

    fn assemble(rom: &AtariRom, rules: MatchRules, rng: Rng) -> Self {
        let mut players = read_players(rom);
        let mut roster = Roster::read(rom);
        set_skills(&rules.team1, &mut roster.attributes_team1);
        set_skills(&rules.team2, &mut roster.attributes_team2);
        if rules.mode == GameMode::Practice {
            for id in PlayerId::team(false) {
                players[id.0].entity.flags.set_invisible(true);
            }
        }
        Self {
            mode: rules.mode,
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

fn set_skills(squad: &Squad, attributes: &mut [Attributes]) {
    match &squad.skills {
        &SquadSkills::Uniform(value) => {
            for member in attributes {
                member.skills = Skills::uniform(value);
            }
        }
        SquadSkills::Given(skills) => {
            for (member, skills) in attributes.iter_mut().zip(skills.iter()) {
                member.skills = *skills;
            }
        }
        SquadSkills::FromTeam => squad.team.apply_skills_to(attributes),
    }
}

fn read_players(rom: &AtariRom) -> Vec<Player> {
    let team = |base: i32| {
        (0..PlayerId::TEAM_SIZE).map(move |i| Player::read(rom, base + i as i32 * Player::SIZE, i))
    };
    team(addr::PLAYERS_TEAM1)
        .chain(team(addr::PLAYERS_TEAM2))
        .collect()
}

// -- Command-line modes ---------------------------------------------------------

/// Two random teams, both on the computer until someone touches the
/// controls (the C# port's work-in-progress setup).
fn knockout(rom: &AtariRom, seeds: &mut SplitMix64, config: &Config) -> MatchRules {
    let (team1, team2) = random_distinct_teams(seeds, config);
    MatchRules {
        mode: GameMode::Knockout,
        team1: Squad {
            team: league_team(rom, team1),
            skills: SquadSkills::FromTeam,
        },
        team2: Squad {
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

fn practice(rom: &AtariRom, config: &Config) -> MatchRules {
    MatchRules {
        team1: Squad {
            team: league_team(rom, config.team1.unwrap_or(0)),
            skills: SquadSkills::Uniform(100),
        },
        team2: Squad {
            team: league_team(rom, config.team2.unwrap_or(1)),
            skills: SquadSkills::Uniform(100),
        },
        max_credits: PerTeam::new(0, 0),
        ..MatchRules::practice(rom)
    }
}

fn two_players(rom: &AtariRom, config: &Config) -> MatchRules {
    // Team 1 is always "PLAYER 1" on 170s, so `--team1` changes nothing.
    let mut rules = MatchRules::two_players(rom, 0);
    if let Some(team2) = config.team2 {
        rules.team2.team = league_team(rom, team2);
    }
    rules
}

fn demo(rom: &AtariRom, config: &Config) -> MatchRules {
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

    /// Like .NET's `Random.Next()`: a non-negative 31-bit value.
    fn next_u31(&mut self) -> u32 {
        (self.next_u64() >> 33) as u32
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next_u64() % n as u64) as usize
    }
}

//! Building the initial state for a match (what the menus would normally do).

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

/// The parts of the setup that differ between game modes.
struct ModeRules {
    team1: Team,
    team2: Team,
    leg_duration: i32,
    max_credits: PerTeam<i32>,
    computer: PerTeam<bool>,
}

impl MatchSetup {
    pub fn new(rom: &AtariRom, config: &Config) -> Self {
        let mut seeds = SplitMix64(config.seed);
        let rng = Rng::new(seeds.next_u31(), seeds.next_u31());

        let teams: Vec<Team> = (0..TEAM_COUNT as i32)
            .map(|i| Team::read(rom, addr::TEAMS + i * Team::SIZE))
            .collect();
        let mut players = read_players(rom);
        let mut roster = Roster::read(rom);
        let rules = match config.mode {
            GameMode::Knockout => knockout(&teams, &mut roster, &mut seeds, config),
            GameMode::Practice => practice(&teams, &mut roster, &mut players, config),
            GameMode::TwoPlayers => two_players(rom, &teams, &mut roster, config),
            GameMode::Demo => demo(&teams, &mut roster, config),
        };

        Self {
            mode: config.mode,
            players,
            roster,
            team1: rules.team1,
            team2: rules.team2,
            coins_credits: 100,
            leg_duration: rules.leg_duration,
            min_credits: 0,
            max_credits: rules.max_credits,
            initial_dir_team1: 0,
            computer: rules.computer,
            rng,
        }
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

fn knockout(
    teams: &[Team],
    roster: &mut Roster,
    seeds: &mut SplitMix64,
    config: &Config,
) -> ModeRules {
    let (team1, team2) = random_distinct_teams(seeds, config);
    set_all_skills(&mut roster.attributes_team1, 100);
    set_all_skills(&mut roster.attributes_team2, 100);
    teams[team1].apply_skills_to(&mut roster.attributes_team1);
    teams[team2].apply_skills_to(&mut roster.attributes_team2);
    ModeRules {
        team1: teams[team1].clone(),
        team2: teams[team2].clone(),
        leg_duration: 90,
        max_credits: PerTeam::new(1100, 1100),
        // The original port leaves both teams to the computer until the
        // player touches the controls.
        computer: PerTeam::new(true, true),
    }
}

fn practice(
    teams: &[Team],
    roster: &mut Roster,
    players: &mut [Player],
    config: &Config,
) -> ModeRules {
    set_all_skills(&mut roster.attributes_team1, 100);
    set_all_skills(&mut roster.attributes_team2, 100);
    for id in PlayerId::team(false) {
        players[id.0].entity.flags.set_invisible(true);
    }
    ModeRules {
        team1: teams[config.team1.unwrap_or(0)].clone(),
        team2: teams[config.team2.unwrap_or(1)].clone(),
        leg_duration: 90,
        max_credits: PerTeam::new(0, 0),
        computer: PerTeam::new(false, false),
    }
}

fn two_players(rom: &AtariRom, teams: &[Team], roster: &mut Roster, config: &Config) -> ModeRules {
    set_all_skills(&mut roster.attributes_team1, 170);
    set_all_skills(&mut roster.attributes_team2, 170);
    let texts = TextPanel::new(rom);
    let mut team1 = teams[config.team1.unwrap_or(0)].clone();
    team1.full_name = texts.text(TextId::Player1Full).clone();
    team1.short_name = texts.text(TextId::Player1Short).clone();
    let team2 = match config.team2 {
        Some(i) => teams[i].clone(),
        None => Team::read(rom, addr::TEAM_PLAYER2),
    };
    ModeRules {
        team1,
        team2,
        leg_duration: 90,
        max_credits: PerTeam::new(2000, 2000),
        computer: PerTeam::new(false, false),
    }
}

fn demo(teams: &[Team], roster: &mut Roster, config: &Config) -> ModeRules {
    let team1 = &teams[config.team1.unwrap_or(14)];
    let team2 = &teams[config.team2.unwrap_or(13)];
    team1.apply_skills_to(&mut roster.attributes_team1);
    team2.apply_skills_to(&mut roster.attributes_team2);
    ModeRules {
        team1: team1.clone(),
        team2: team2.clone(),
        leg_duration: 20,
        max_credits: PerTeam::new(0, 0),
        computer: PerTeam::new(true, true),
    }
}

fn set_all_skills(attributes: &mut [Attributes], value: i32) {
    for member in attributes {
        member.skills = Skills::uniform(value);
    }
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

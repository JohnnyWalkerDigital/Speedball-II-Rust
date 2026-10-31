//! Every string the menus show, by what it is for: screen titles, list
//! menus, messages, the panel screens' lines and the teams' names.
//!
//! Read from the original's memory ([`MenuText::read`]) or an asset pack,
//! where they are written in the [`readable`](super::readable) form.

use serde::{Deserialize, Serialize};

use super::ram::{addr, MenuRam};
use super::readable;
use crate::game::career::{TeamId, LEAGUE_TEAMS};
use crate::game::cup::ROUNDS as CUP_ROUNDS;

/// A screen title: the text at the top and bottom of the backdrop.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Title {
    #[serde(with = "readable::menu")]
    pub top: Vec<u8>,
    #[serde(with = "readable::menu")]
    pub bottom: Vec<u8>,
}

/// A list menu (`shared_menu_function`'s menu structure).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MenuSpec {
    pub title: Title,
    #[serde(with = "readable::menu")]
    pub items: Vec<u8>,
    /// One pair for each item.
    pub arrows: Vec<Arrows>,
}

/// Where the two arrows either side of a menu item go, in text cells.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Arrows {
    #[serde(with = "readable::cell")]
    pub left: (u8, u8),
    #[serde(with = "readable::cell")]
    pub right: (u8, u8),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MenuText {
    pub titles: Titles,
    pub menus: Menus,
    pub messages: Messages,
    pub panels: Panels,
    pub teams: Teams,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Titles {
    /// "SPEEDBALL 2", "PUSH START": behind most messages.
    pub speedball_2: Title,
    pub two_player: Title,
    pub next_fixture: Title,
    pub division_1: Title,
    pub division_2: Title,
    pub results: Title,
    pub statistics: Title,
    pub fixture_list: Title,
    pub save_game: Title,
    pub load_game: Title,
    pub replay_goals: Title,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Menus {
    /// One player, two players, demo, replay goals.
    pub game_type: MenuSpec,
    /// Knockout, league, cup, practice.
    pub one_player: MenuSpec,
    /// One match, best of three, best of five.
    pub two_players: MenuSpec,
    /// New game, team manager, load game.
    pub league: MenuSpec,
    /// New game, load game.
    pub cup: MenuSpec,
    /// Retry, cancel.
    pub disk_error: MenuSpec,
}

/// Messages in the big letters. Numbers are written into some at fixed
/// places, which [`MenuText::check`] lists.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Messages {
    #[serde(with = "readable::menu")]
    pub game_over: Vec<u8>,
    #[serde(with = "readable::menu")]
    pub replays_over: Vec<u8>,
    /// "MATCH 0", numbered.
    #[serde(with = "readable::menu")]
    pub match_number: Vec<u8>,
    #[serde(with = "readable::menu")]
    pub second_leg: Vec<u8>,
    /// "PLAYER 0", before each player's turn in the gym.
    #[serde(with = "readable::menu")]
    pub player_turn: Vec<u8>,
    #[serde(with = "readable::menu")]
    pub player_wins: Vec<u8>,
    #[serde(with = "readable::menu")]
    pub score: Vec<u8>,
    #[serde(with = "readable::menu")]
    pub total: Vec<u8>,
    #[serde(with = "readable::menu")]
    pub blue_win: Vec<u8>,
    #[serde(with = "readable::menu")]
    pub red_win: Vec<u8>,
    #[serde(with = "readable::menu")]
    pub match_drawn: Vec<u8>,
    /// On the victory and defeat pictures, in small letters.
    #[serde(with = "readable::menu")]
    pub score_win: Vec<u8>,
    #[serde(with = "readable::menu")]
    pub total_score_win: Vec<u8>,
    #[serde(with = "readable::menu")]
    pub new_season: Vec<u8>,
    #[serde(with = "readable::menu")]
    pub relegated: Vec<u8>,
    #[serde(with = "readable::menu")]
    pub playoff: Vec<u8>,
    /// "ROUND ONE" to "GRAND FINAL".
    #[serde(with = "readable::menu_list")]
    pub cup_rounds: Vec<Vec<u8>>,
    #[serde(with = "readable::menu")]
    pub insert_game_save_disk: Vec<u8>,
    #[serde(with = "readable::menu")]
    pub insert_goal_save_disk: Vec<u8>,
    /// On the match panel: "GOAL 0", numbered, for a replay from disk.
    #[serde(with = "readable::menu")]
    pub replay_goal: Vec<u8>,
    /// On the match panel: "DISK ERROR", "FIRE RETRY", "ESC CANCEL".
    #[serde(with = "readable::menu")]
    pub disk_error: Vec<u8>,
}

/// The screens drawn on the steel panel, in small letters. Their lines'
/// rows are set by their place in the list.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Panels {
    /// "PLAY", between two teams.
    #[serde(with = "readable::panel")]
    pub play: Vec<u8>,
    pub next_fixture: Vec<Line<FixtureNumber>>,
    #[serde(with = "readable::panel_list")]
    pub league_table: Vec<Vec<u8>>,
    /// A team's row, filled in by `construct_league_placing_string`.
    #[serde(with = "readable::panel")]
    pub league_row: Vec<u8>,
    #[serde(with = "readable::panel_list")]
    pub results: Vec<Vec<u8>>,
    #[serde(with = "readable::panel")]
    pub result_row: Vec<u8>,
    pub statistics: Vec<Line<Statistic>>,
    #[serde(with = "readable::panel_list")]
    pub fixture_list: Vec<Vec<u8>>,
}

/// A line of a panel screen, and the numbers written into it, if any.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Line<N> {
    #[serde(with = "readable::panel")]
    pub text: Vec<u8>,
    #[serde(default = "Option::default", skip_serializing_if = "Option::is_none")]
    pub numbers: Option<N>,
}

/// The next-fixture lines that hold numbers, in the order
/// `generate_team_fixture_strings` fills them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FixtureNumber {
    PlayedWonDrawnLostPoints,
    LeaguePlacing,
    PointsFor,
    PointsAgainst,
    Defence,
    Midfield,
    Attack,
    Substitutes,
}

impl FixtureNumber {
    pub const ALL: [FixtureNumber; 8] = [
        FixtureNumber::PlayedWonDrawnLostPoints,
        FixtureNumber::LeaguePlacing,
        FixtureNumber::PointsFor,
        FixtureNumber::PointsAgainst,
        FixtureNumber::Defence,
        FixtureNumber::Midfield,
        FixtureNumber::Attack,
        FixtureNumber::Substitutes,
    ];

    fn address(self) -> usize {
        match self {
            FixtureNumber::PlayedWonDrawnLostPoints => addr::STR_PWDLP_NUMS,
            FixtureNumber::LeaguePlacing => addr::STR_LEAGUE_PLACING,
            FixtureNumber::PointsFor => addr::STR_POINTS_FOR,
            FixtureNumber::PointsAgainst => addr::STR_POINTS_AGAINST,
            FixtureNumber::Defence => addr::STR_DEFENCE,
            FixtureNumber::Midfield => addr::STR_MIDFIELD,
            FixtureNumber::Attack => addr::STR_ATTACK,
            FixtureNumber::Substitutes => addr::STR_SUBSTITUTES,
        }
    }
}

/// The match statistics, each on its own line.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Statistic {
    ShotsAtGoal,
    GoalsScored,
    GoalsSaved,
    TimeInPossession,
    SuccessfulTackles,
    Substitutions,
    TimeInOpponentsHalf,
    BonusPoints,
}

impl Statistic {
    const ALL: [Statistic; 8] = [
        Statistic::ShotsAtGoal,
        Statistic::GoalsScored,
        Statistic::GoalsSaved,
        Statistic::TimeInPossession,
        Statistic::SuccessfulTackles,
        Statistic::Substitutions,
        Statistic::TimeInOpponentsHalf,
        Statistic::BonusPoints,
    ];

    fn address(self) -> usize {
        match self {
            Statistic::ShotsAtGoal => addr::STR_SHOTS_AT_GOAL,
            Statistic::GoalsScored => addr::STR_GOALS_SCORED,
            Statistic::GoalsSaved => addr::STR_GOALS_SAVED,
            Statistic::TimeInPossession => addr::STR_TIME_IN_POSSESSION,
            Statistic::SuccessfulTackles => addr::STR_SUCCESSFUL_TACKLES,
            Statistic::Substitutions => addr::STR_SUBSTITUTIONS,
            Statistic::TimeInOpponentsHalf => addr::STR_TIME_IN_OPPONENTS_HALF,
            Statistic::BonusPoints => addr::STR_BONUS_POINTS,
        }
    }
}

/// A list of strings (`draw_string_list`), and what each line shows.
fn lines<N>(ram: &MenuRam, list: usize, numbers: impl Fn(usize) -> Option<N>) -> Vec<Line<N>> {
    ram.pointer_list(list)
        .into_iter()
        .map(|address| Line {
            text: ram.string(address),
            numbers: numbers(address),
        })
        .collect()
}

/// The teams' names as the menus show them.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Teams {
    /// The sixteen league teams, Brutal Deluxe first.
    #[serde(with = "readable::menu_list")]
    pub league: Vec<Vec<u8>>,
    #[serde(with = "readable::menu")]
    pub player_two: Vec<u8>,
}

impl MenuText {
    pub fn read(ram: &MenuRam) -> Self {
        let title = |address| ram.title(address);
        let string = |address| ram.string(address);
        Self {
            titles: Titles {
                speedball_2: title(addr::TITLE_SPEEDBALL_2_PUSH_START),
                two_player: title(addr::TITLE_TWO_PLAYER),
                next_fixture: title(addr::TITLE_NEXT_FIXTURE),
                division_1: title(addr::TITLE_DIVISION_1),
                division_2: title(addr::TITLE_DIVISION_2),
                results: title(addr::TITLE_RESULTS),
                statistics: title(addr::TITLE_STATISTICS),
                fixture_list: title(addr::TITLE_FIXTURE_LIST),
                save_game: title(addr::TITLE_SAVE_GAME),
                load_game: title(addr::TITLE_LOAD_GAME),
                replay_goals: title(addr::TITLE_REPLAY_GOALS),
            },
            menus: Menus {
                game_type: MenuSpec {
                    title: title(addr::TITLE_SELECT_GAME),
                    items: string(addr::STR_MENU_GAME_SELECT),
                    arrows: ram.arrows(addr::MENU_ARROWS_GAME_TYPE, 4),
                },
                one_player: ram.menu(addr::MENU_SELECT_MATCH),
                two_players: ram.menu(addr::MENU_SELECT_ONE_THREE_FIVE),
                league: ram.menu(addr::MENU_SELECT_GAME),
                cup: ram.menu(addr::MENU_SELECT_NEW_LOAD),
                disk_error: ram.menu(addr::MENU_SELECT_RETRY_CANCEL),
            },
            messages: Messages {
                game_over: string(addr::STR_GAME_OVER),
                replays_over: string(addr::STR_REPLAYS_OVER),
                match_number: string(addr::STR_MATCH),
                second_leg: string(addr::STR_SECOND_LEG),
                player_turn: string(addr::STR_PLAYER_0),
                player_wins: string(addr::STR_PLAYER_WINS),
                score: string(addr::STR_SCORE),
                total: string(addr::STR_TOTAL),
                blue_win: string(addr::STR_BLUE_WIN),
                red_win: string(addr::STR_RED_WIN),
                match_drawn: string(addr::STR_MATCH_DRAWN),
                score_win: string(addr::STR_SCORE_WIN),
                total_score_win: string(addr::STR_TOTAL_SCORE_WIN),
                new_season: string(addr::STR_NEW_SEASON),
                relegated: string(addr::STR_RELEGATE),
                playoff: string(addr::STR_PLAYOFF),
                cup_rounds: (0..CUP_ROUNDS)
                    .map(|round| ram.string_at(addr::STRINGS_CUP_ROUNDS, round))
                    .collect(),
                insert_game_save_disk: string(addr::STR_INSERT_GAME_SAVE_DISK),
                insert_goal_save_disk: string(addr::STR_INSERT_GOAL_SAVE_DISK),
                replay_goal: string(addr::STR_GOAL),
                disk_error: string(addr::STR_DISK_ERROR),
            },
            panels: Panels {
                play: string(addr::STR_PLAY),
                next_fixture: lines(ram, addr::STRINGS_NEXT_FIXTURE, |address| {
                    FixtureNumber::ALL
                        .into_iter()
                        .find(|n| n.address() == address)
                }),
                league_table: ram.string_list(addr::STRINGS_LEAGUE_PLACINGS),
                league_row: string(addr::STR_LEAGUE_TEAM_DETAILS),
                results: ram.string_list(addr::STRINGS_RESULTS),
                result_row: string(addr::STR_LEAGUE_MATCH_RESULT),
                statistics: lines(ram, addr::STRINGS_STATISTICS, |address| {
                    Statistic::ALL.into_iter().find(|s| s.address() == address)
                }),
                fixture_list: ram.string_list(addr::STRINGS_FIXTURE_LIST),
            },
            teams: Teams {
                league: (0..LEAGUE_TEAMS)
                    .map(|i| ram.team_name(TeamId::League(i)))
                    .collect(),
                player_two: ram.team_name(TeamId::PlayerTwo),
            },
        }
    }

    /// A team's name, its position to be set.
    pub fn team_name(&self, team: TeamId) -> Vec<u8> {
        match team {
            TeamId::League(index) => self.teams.league[index].clone(),
            TeamId::PlayerTwo => self.teams.player_two.clone(),
        }
    }

    /// Finds what would stop the menus using these strings: numbers are
    /// written into some at fixed places, and menus' choices are counted.
    pub fn check(&self) -> Result<(), String> {
        let mut problems = Vec::new();
        let mut long_enough = |what: &str, text: &[u8], length: usize| {
            if text.len() < length {
                problems.push(format!(
                    "{what} must be at least {length} characters long, counting its \
                     position as two: the game writes into it at fixed places"
                ));
            }
        };
        let m = &self.messages;
        // `flow.rs` writes the match number, the winner and the player.
        long_enough("messages.match_number", &m.match_number, 9);
        long_enough("messages.player_wins", &m.player_wins, 10);
        long_enough("messages.player_turn", &m.player_turn, 10);
        long_enough("messages.score", &m.score, 18);
        long_enough("messages.total", &m.total, 18);
        // These have the winners' column too.
        long_enough("messages.score_win", &m.score_win, 19);
        long_enough("messages.total_score_win", &m.total_score_win, 25);
        long_enough("messages.replay_goal", &m.replay_goal, 10);
        let p = &self.panels;
        long_enough("panels.league_row", &p.league_row, 0x32);
        long_enough("panels.result_row", &p.result_row, 0x32);
        for line in &p.next_fixture {
            if line.numbers.is_some() {
                long_enough("a numbered panels.next_fixture line", &line.text, 0x32);
            }
        }
        for line in &p.statistics {
            if line.numbers.is_some() {
                long_enough("a numbered panels.statistics line", &line.text, 0x2A);
            }
        }

        for number in FixtureNumber::ALL {
            if !p.next_fixture.iter().any(|l| l.numbers == Some(number)) {
                let name = serde_json::to_string(&number).expect("a name");
                problems.push(format!(
                    "panels.next_fixture has no line with \"numbers\": {name}"
                ));
            }
        }
        if m.cup_rounds.len() != CUP_ROUNDS {
            problems.push(format!("messages.cup_rounds must have {CUP_ROUNDS} rounds"));
        }
        if self.teams.league.len() != LEAGUE_TEAMS {
            problems.push(format!("teams.league must have {LEAGUE_TEAMS} teams"));
        }
        let menus = &self.menus;
        for (name, menu, choices) in [
            ("game_type", &menus.game_type, 4),
            ("one_player", &menus.one_player, 4),
            ("two_players", &menus.two_players, 3),
            ("league", &menus.league, 3),
            ("cup", &menus.cup, 2),
            ("disk_error", &menus.disk_error, 2),
        ] {
            if menu.arrows.len() != choices {
                problems.push(format!(
                    "menus.{name} must have {choices} pairs of arrows, one for each choice"
                ));
            }
        }
        if problems.is_empty() {
            Ok(())
        } else {
            Err(problems.join("\n"))
        }
    }
}

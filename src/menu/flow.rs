//! The front end: the main menu and the game modes, ported from
//! `main_menu`, `shared_menu_function`, `game_two_player`, `game_knockout`
//! and the result screens. The league and cup are in their own modules.
//!
//! Each function follows its original, waiting on [`Ui`] where the 68000
//! code waited for the next frame.

use std::rc::Rc;

use super::assets::{MenuAssets, SmallFont};
use super::host::{Button, Control};
use super::manager::data::ManagerTables;
use super::manager::{self, Cursors, Standings};
use super::panels::{self, Competition};
use super::play::{run_match, Controls, ReplaySaves};
use super::ram::{addr, MenuRam, MenuSpec, Title};
use super::text::write_digits;
use super::ui::{Flow, Page, Stop, Ui};
use crate::data::save_disk::SaveDisk;
use crate::data::AtariRom;
use crate::game::career::{compare_scores, Career, LastMatch, TeamId};
use crate::game::events::Sound;
use crate::game::geometry::{PerTeam, Side};
use crate::game::league::ScoresTable;
use crate::game::rng::Rng;
use crate::game::setup::{MatchRules, SquadSkills, BRUTAL_DELUXE};
use crate::game::squad::Market;
use crate::game::{Match, MatchSetup};
use crate::graphics::ilbm::Picture;
use crate::graphics::Canvas;
use crate::render::MatchRenderer;

/// After this many frames on the main menu the demo starts (30 seconds).
const DEMO_TIMEOUT_VBLANKS: u32 = 1500;
/// `wait_any_fire_pressed_timeout` gives up after the 8-bit frame timer
/// wraps (about 5 seconds).
const FIRE_TIMEOUT_VBLANKS: u32 = 255;
/// `game_knockout` plays the other 15 teams in turn.
const KNOCKOUT_ROUNDS: usize = 15;

const MENU_MOVE: Sound = Sound {
    id: 0x14,
    priority: 1,
};
const MENU_SELECT: Sound = Sound {
    id: 0x16,
    priority: 1,
};
const VICTORY_CHEER: Sound = Sound { id: 1, priority: 8 };
const VICTORY_AFTER: Sound = Sound {
    id: 30,
    priority: 0,
};
const WIN_FANFARE: Sound = Sound {
    id: 0x27,
    priority: 8,
};
const WIN_AFTER: Sound = Sound {
    id: 0x23,
    priority: 0,
};

/// Everything the front end works with.
pub struct Game {
    pub rom: Rc<AtariRom>,
    pub ram: MenuRam,
    pub assets: MenuAssets,
    pub renderer: MatchRenderer,
    /// Money and results, kept across games like the original's globals.
    pub career: Career,
    /// The menus' random numbers (`rand`), for draws and other results.
    pub rng: Rng,
    pub scores_table: ScoresTable,
    /// `coin_value`: doubled while the cup is on.
    pub coin_value: i32,
    /// `misc_flags` bit 0: the computer plays Brutal Deluxe's matches
    /// ("TEAM MANAGER" in the league).
    pub manager_mode: bool,
    /// `misc_flags` bit 1: a one-player game (set for the demo too).
    /// Goal replays can only be saved in one-player games outside manager
    /// mode; the flags outlast the game, so the same goes for the replay
    /// viewer.
    pub one_player: bool,
    /// `misc_flags` bit 4: matches are sped up, until the next main menu.
    pub sped_up: bool,
    /// The status bar as the last match left it. The Amiga draws it once
    /// at boot and matches draw over it, so the replay viewer shows it.
    pub status_bar: Option<Canvas>,
    pub manager_tables: ManagerTables,
    /// Where the manager screens' cursors were left.
    pub cursors: Cursors,
    pub save_disk: SaveDisk,
    seed: u64,
}

impl Game {
    pub fn new(
        rom: Rc<AtariRom>,
        ram: MenuRam,
        assets: MenuAssets,
        renderer: MatchRenderer,
        save_disk: SaveDisk,
        seed: u64,
    ) -> Self {
        let squads = PerTeam::new(
            ram.squad(addr::TEAM_INFO_TABLE),
            ram.squad(addr::TEAM_INFO_TABLE_OTHER),
        );
        let career = Career::new(ram.team_records(), squads, Market::new(ram.market()));
        let manager_tables = ManagerTables::read(&ram);
        let scores_table =
            ScoresTable::new(ram.slice(addr::SCORES_TABLE, addr::SCORES_TABLE_LENGTH));
        Self {
            rom,
            ram,
            assets,
            renderer,
            career,
            rng: Rng::new(seed as u32, (seed >> 32) as u32 | 1),
            scores_table,
            coin_value: 100,
            manager_mode: false,
            one_player: false,
            sped_up: false,
            status_bar: None,
            manager_tables,
            cursors: Cursors::default(),
            save_disk,
            seed,
        }
    }

    /// A new seed for each match. (The original's random numbers carried on
    /// from the menus, where they were stirred while waiting for input.)
    fn next_seed(&mut self) -> u64 {
        self.seed = self.seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
        self.seed
    }

    fn title(&self, address: usize) -> Title {
        self.ram.title(address)
    }

    pub(super) fn titled_page(&self, address: usize) -> Page {
        Page::titled(&self.assets, &self.title(address))
    }

    fn may_save_replays(&self) -> bool {
        self.one_player && !self.manager_mode
    }

    /// Saving goal replays, if `misc_flags` allow it (`save_replay`).
    pub(super) fn replay_saves(&self) -> Option<ReplaySaves<'_>> {
        self.may_save_replays()
            .then(|| ReplaySaves::new(&self.save_disk, &self.ram))
    }
}

/// What the main menu leads to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MainChoice {
    OnePlayer,
    TwoPlayers,
    Demo,
    ReplayGoals,
}

impl MainChoice {
    /// What "GAME OVER" says afterwards.
    fn game_over(self) -> usize {
        match self {
            MainChoice::ReplayGoals => addr::STR_REPLAYS_OVER,
            _ => addr::STR_GAME_OVER,
        }
    }
}

/// `start`'s loop: the main menu, for ever. Returns when the player quits.
pub fn run(ui: &mut Ui, game: &mut Game) {
    loop {
        match main_menu(ui, game) {
            Ok(()) | Err(Stop::Abort) => {}
            Err(Stop::Quit) => return,
        }
    }
}

/// `main_menu`: pick a game, play it, then "GAME OVER".
fn main_menu(ui: &mut Ui, game: &mut Game) -> Flow {
    game.career.start_main_menu();
    game.career.restore_identities(&game.ram.base_identities());
    game.sped_up = false;
    let choice = select_game(ui, game)?;
    match choice {
        MainChoice::OnePlayer => one_player(ui, game)?,
        MainChoice::TwoPlayers => two_players(ui, game)?,
        MainChoice::Demo => demo(ui, game)?,
        MainChoice::ReplayGoals => super::replays::replay_goals(ui, game)?,
    }
    let mut page = game.titled_page(addr::TITLE_SPEEDBALL_2_PUSH_START);
    page.big_text(&game.assets, &game.ram.string(choice.game_over()));
    ui.transition(&page)?;
    wait_any_fire(ui)
}

/// The "SELECT GAME" screen. Escape here quits the port (the original had
/// nowhere to go back to).
fn select_game(ui: &mut Ui, game: &Game) -> Flow<MainChoice> {
    let mut base = game.titled_page(addr::TITLE_SELECT_GAME);
    base.big_text(&game.assets, &game.ram.string(addr::STR_MENU_GAME_SELECT));
    let arrows = game.ram.arrows(addr::MENU_ARROWS_GAME_TYPE, 4);
    let mut position = 0;
    loop {
        let mut page = base.clone();
        page.arrows(&game.assets, arrows[position]);
        ui.transition(&page)?;
        ui.clear_input();
        let mut idle = 0;
        let pressed = loop {
            ui.vblank()?;
            idle += 1;
            if idle == DEMO_TIMEOUT_VBLANKS {
                return Ok(MainChoice::Demo);
            }
            if let Some(button) = ui.next_press(|b| is_player_1(b) || b == Button::Escape) {
                break button;
            }
        };
        match pressed {
            Button::Escape => return Err(Stop::Quit),
            Button::Joystick(_, Control::Up) => position = (position + 3) % 4,
            Button::Joystick(_, Control::Down) => position = (position + 1) % 4,
            Button::Joystick(_, Control::Fire) => {
                ui.play_sound(MENU_SELECT);
                return Ok([
                    MainChoice::OnePlayer,
                    MainChoice::TwoPlayers,
                    MainChoice::Demo,
                    MainChoice::ReplayGoals,
                ][position]);
            }
            // Left and right just redraw the menu.
            _ => continue,
        }
        ui.play_sound(MENU_MOVE);
    }
}

/// `shared_menu_function`: a list menu. Returns the chosen item.
pub(super) fn list_menu(ui: &mut Ui, game: &Game, menu: &MenuSpec) -> Flow<usize> {
    let mut base = Page::titled(&game.assets, &menu.title);
    base.big_text(&game.assets, &menu.items);
    let count = menu.arrows.len();
    let mut position = 0;
    loop {
        let mut page = base.clone();
        page.arrows(&game.assets, menu.arrows[position]);
        ui.transition(&page)?;
        ui.clear_input();
        let pressed = loop {
            ui.vblank()?;
            let wanted = |b| {
                b == Button::Escape
                    || [Control::Up, Control::Down, Control::Fire]
                        .iter()
                        .any(|&c| b == Button::Joystick(Side::Team1, c))
            };
            if let Some(button) = ui.next_press(wanted) {
                break button;
            }
        };
        match pressed {
            Button::Escape => return Err(Stop::Abort),
            Button::Joystick(_, Control::Up) => position = (position + count - 1) % count,
            Button::Joystick(_, Control::Down) => position = (position + 1) % count,
            _ => {
                ui.play_sound(MENU_SELECT);
                return Ok(position);
            }
        }
        ui.play_sound(MENU_MOVE);
    }
}

fn is_player_1(button: Button) -> bool {
    matches!(button, Button::Joystick(Side::Team1, _))
}

/// `wait_any_fire_pressed_timeout`: either fire button, Escape, or about
/// five seconds.
pub(super) fn wait_any_fire(ui: &mut Ui) -> Flow {
    ui.clear_input();
    for _ in 0..FIRE_TIMEOUT_VBLANKS {
        ui.vblank()?;
        let fire = |b| matches!(b, Button::Joystick(_, Control::Fire)) || b == Button::Escape;
        match ui.next_press(fire) {
            Some(Button::Escape) => return Err(Stop::Abort),
            Some(_) => return Ok(()),
            None => {}
        }
    }
    Ok(())
}

/// `wait_fire_pressed`: player 1's fire button or Escape, with no time
/// limit (the table screens).
pub(super) fn wait_fire(ui: &mut Ui) -> Flow {
    ui.clear_input();
    loop {
        ui.vblank()?;
        let fire = |b| b == Button::Joystick(Side::Team1, Control::Fire) || b == Button::Escape;
        match ui.next_press(fire) {
            Some(Button::Escape) => return Err(Stop::Abort),
            Some(_) => return Ok(()),
            None => {}
        }
    }
}

// -- One player ------------------------------------------------------------------

/// `game_single_player`: knockout, league, cup or practice.
fn one_player(ui: &mut Ui, game: &mut Game) -> Flow {
    game.career.init_squad(Side::Team1, 100);
    game.manager_mode = false;
    game.one_player = true;
    let menu = game.ram.menu(addr::MENU_SELECT_MATCH);
    match list_menu(ui, game, &menu)? {
        0 => knockout(ui, game),
        1 => super::league::league(ui, game),
        2 => super::cup::cup(ui, game),
        _ => practice(ui, game),
    }
}

/// `game_knockout`: beat the other fifteen teams one after another, with
/// the gym before each match. A draw replays the round; a defeat ends the
/// run.
fn knockout(ui: &mut Ui, game: &mut Game) -> Flow {
    game.career.clear_records();
    game.career.set_cash_cap(1100);
    let mut round = 1;
    while round <= KNOCKOUT_ROUNDS {
        let teams = PerTeam::new(TeamId::League(BRUTAL_DELUXE), TeamId::League(round));
        let (score1, score2) =
            brutal_deluxe_match(ui, game, Competition::Knockout, round, 0, Standings::None)?;
        let scores = PerTeam::new(score1, score2);
        game.career.record_result(teams, scores);
        match compare_scores(scores.team1, scores.team2) {
            std::cmp::Ordering::Equal => continue,
            std::cmp::Ordering::Less => return Ok(()),
            std::cmp::Ordering::Greater => round += 1,
        }
    }
    win_shared(ui, &game.assets.knockout_win)
}

/// `win_shared`: a trophy picture (knockout, league, cup or promotion).
pub(super) fn win_shared(ui: &mut Ui, picture: &Picture) -> Flow {
    ui.transition(&Page::picture(picture))?;
    ui.play_sound(VICTORY_CHEER);
    wait_any_fire(ui)?;
    ui.play_sound(WIN_FANFARE);
    ui.play_sound(WIN_AFTER);
    wait_any_fire(ui)
}

/// `game_practice`: Brutal Deluxe on their own, with no result screen.
fn practice(ui: &mut Ui, game: &mut Game) -> Flow {
    game.career.set_cash_cap(10_000);
    game.one_player = false;
    let mut rules = MatchRules::practice(&game.rom);
    rules.team1.skills = SquadSkills::Players(Box::new(game.career.squads.team1.clone()));
    let teams = PerTeam::new(TeamId::League(BRUTAL_DELUXE), TeamId::League(1));
    play_match(ui, game, rules, teams, ResultScreen::None, false).map(|_| ())
}

// -- Two players -----------------------------------------------------------------

/// `game_two_player`: one match, or the best of three or five. Each match
/// is two legs with ends swapped, and level aggregates go to another leg;
/// both players get the gym before every leg.
fn two_players(ui: &mut Ui, game: &mut Game) -> Flow {
    game.career.clear_records();
    game.career.set_cash_cap(2000);
    game.career.init_squad(Side::Team1, 170);
    game.career.init_squad(Side::Team2, 170);
    game.manager_mode = false;
    game.one_player = false;
    let menu = game.ram.menu(addr::MENU_SELECT_ONE_THREE_FIVE);
    let matches = list_menu(ui, game, &menu)? * 2 + 1;
    let teams = PerTeam::new(TeamId::League(BRUTAL_DELUXE), TeamId::PlayerTwo);
    let mut wins = PerTeam::new(0, 0);
    for number in 1..=matches {
        let mut text = game.ram.string(addr::STR_MATCH);
        text[8] = 0x5B + number as u8;
        message(ui, game, addr::TITLE_TWO_PLAYER, &text)?;

        let mut initial_dir = 0;
        gym_two_player(ui, game)?;
        let rules = two_player_rules(game, initial_dir);
        let mut total = play_match(ui, game, rules, teams, ResultScreen::Score, false)?;
        message(
            ui,
            game,
            addr::TITLE_TWO_PLAYER,
            &game.ram.string(addr::STR_SECOND_LEG),
        )?;
        loop {
            initial_dir ^= 4;
            gym_two_player(ui, game)?;
            let rules = two_player_rules(game, initial_dir);
            let leg = play_match(ui, game, rules, teams, ResultScreen::Score, false)?;
            total = PerTeam::new(total.team1 + leg.team1, total.team2 + leg.team2);
            result_screen(ui, game, total, addr::STR_TOTAL)?;
            if total.team1 != total.team2 {
                break;
            }
        }
        if compare_scores(total.team2, total.team1).is_gt() {
            wins.team2 += 1;
        } else {
            wins.team1 += 1;
        }
        if wins.team1 > matches / 2 || wins.team2 > matches / 2 {
            break;
        }
        // Only matches that leave the series open are recorded.
        game.career.record_result(teams, total);
    }
    let mut text = game.ram.string(addr::STR_PLAYER_WINS);
    text[9] = if wins.team1 < wins.team2 { 0x5D } else { 0x5C };
    message(ui, game, addr::TITLE_TWO_PLAYER, &text)
}

/// `gym_two_player`: each player in turn gets the gym, with their own
/// joystick and money.
fn gym_two_player(ui: &mut Ui, game: &mut Game) -> Flow {
    let teams = (TeamId::League(BRUTAL_DELUXE), TeamId::PlayerTwo);
    for (side, digit) in [(Side::Team1, 0x5C), (Side::Team2, 0x5D)] {
        let mut text = game.ram.string(addr::STR_PLAYER_0);
        text[9] = digit;
        message(ui, game, addr::TITLE_TWO_PLAYER, &text)?;
        manager::gym_screen(ui, game, side, Competition::TwoPlayers, teams)?;
    }
    Ok(())
}

fn two_player_rules(game: &Game, initial_dir: i32) -> MatchRules {
    let mut rules = MatchRules::two_players(&game.rom, initial_dir);
    rules.team1.skills = SquadSkills::Players(Box::new(game.career.squads.team1.clone()));
    rules.team2.skills = SquadSkills::Players(Box::new(game.career.squads.team2.clone()));
    rules
}

/// A title, one line of big text, then wait for fire.
pub(super) fn message(ui: &mut Ui, game: &Game, title: usize, text: &[u8]) -> Flow {
    let mut page = game.titled_page(title);
    page.big_text(&game.assets, text);
    ui.transition(&page)?;
    wait_any_fire(ui)
}

// -- Demo ------------------------------------------------------------------------

/// The demo game, then a look at the manager and gym screens.
fn demo(ui: &mut Ui, game: &mut Game) -> Flow {
    game.manager_mode = true;
    game.one_player = true;
    let rules = MatchRules::demo(&game.rom);
    let teams = PerTeam::new(TeamId::League(14), TeamId::League(13));
    play_match(ui, game, rules, teams, ResultScreen::Score, false)?;
    manager::show_off(ui, game)
}

// -- Matches and results ---------------------------------------------------------

/// `cup_league_match`: a league or cup match. Brutal Deluxe's are played
/// (with them as team 1, the scores swapped back afterwards); the rest
/// come from the results table.
pub(super) fn cup_league_match(
    ui: &mut Ui,
    game: &mut Game,
    competition: Competition,
    teams: (TeamId, TeamId),
    initial_dir: i32,
    standings: Standings,
) -> Flow<(i32, i32)> {
    let opponent = |team: TeamId| match team {
        TeamId::League(index) => index,
        TeamId::PlayerTwo => unreachable!("player 2 only plays two-player games"),
    };
    let brutal_deluxe = TeamId::League(BRUTAL_DELUXE);
    if teams.0 == brutal_deluxe {
        brutal_deluxe_match(
            ui,
            game,
            competition,
            opponent(teams.1),
            initial_dir,
            standings,
        )
    } else if teams.1 == brutal_deluxe {
        let (ours, theirs) = brutal_deluxe_match(
            ui,
            game,
            competition,
            opponent(teams.0),
            initial_dir,
            standings,
        )?;
        Ok((theirs, ours))
    } else {
        Ok(game.scores_table.result(&mut game.rng, teams.0, teams.1))
    }
}

/// `cup_league_match_bd`: the next-fixture screen, the manager screens
/// (only the gym in a knockout), then the match.
fn brutal_deluxe_match(
    ui: &mut Ui,
    game: &mut Game,
    competition: Competition,
    opponent: usize,
    initial_dir: i32,
    standings: Standings,
) -> Flow<(i32, i32)> {
    let teams = (TeamId::League(BRUTAL_DELUXE), TeamId::League(opponent));
    ui.transition(&panels::next_fixture(game, competition, teams))?;
    wait_fire(ui)?;
    if competition == Competition::Knockout {
        manager::gym_screen(ui, game, Side::Team1, competition, teams)?;
    } else {
        manager::manager_screen(ui, game, competition, teams, standings)?;
    }
    let mut rules = MatchRules::one_player(&game.rom, opponent);
    rules.team1.skills = SquadSkills::Players(Box::new(game.career.squads.team1.clone()));
    rules.computer.team1 = game.manager_mode;
    rules.initial_dir_team1 = initial_dir;
    rules.coin_value = game.coin_value;
    let teams = PerTeam::new(teams.0, teams.1);
    // `match_kind` 1 or 2.
    let may_speed_up =
        game.manager_mode && matches!(competition, Competition::League | Competition::Cup);
    let scores = play_match(ui, game, rules, teams, ResultScreen::Pictures, may_speed_up)?;
    Ok((scores.team1, scores.team2))
}

/// Which result screen follows a match (`display_match_result`).
#[derive(Clone, Copy, PartialEq, Eq)]
enum ResultScreen {
    /// Practice has none.
    None,
    /// The score and who won.
    Score,
    /// Brutal Deluxe's games: a victory or defeat picture, or the score for
    /// a draw.
    Pictures,
}

/// `play_match`: the match, its result screen, then the prize money.
fn play_match(
    ui: &mut Ui,
    game: &mut Game,
    mut rules: MatchRules,
    teams: PerTeam<TeamId>,
    result: ResultScreen,
    may_speed_up: bool,
) -> Flow<PerTeam<i32>> {
    let money = game.career.money;
    rules.min_credits = i32::from(money.start);
    rules.max_credits = PerTeam::new(i32::from(money.cap.team1), i32::from(money.cap.team2));
    let managed = PerTeam::new(
        matches!(rules.team1.skills, SquadSkills::Players(_)),
        matches!(rules.team2.skills, SquadSkills::Players(_)),
    );
    let seed = game.next_seed();
    let setup = MatchSetup::new(&game.rom, rules, seed);
    let mut game_match = Match::new(setup, Rc::clone(&game.rom));
    let controls = Controls {
        take_over_on_touch: false,
        hold_at_end: false,
        replay_saves: game
            .may_save_replays()
            .then(|| ReplaySaves::new(&game.save_disk, &game.ram)),
        sped_up: &mut game.sped_up,
        may_speed_up,
    };
    let played = run_match(ui, &mut game_match, &game.renderer, controls);
    game.status_bar = Some(ui.front().clone());
    // Substitutions change the squad for good, even in an abandoned match.
    for (side, managed) in [(Side::Team1, managed.team1), (Side::Team2, managed.team2)] {
        if managed {
            let after = game_match.squad_after(side, game.career.squads.get(side));
            *game.career.squads.get_mut(side) = after;
        }
    }
    let scores = played?;
    if result == ResultScreen::None {
        return Ok(scores);
    }
    match compare_scores(scores.team1, scores.team2) {
        std::cmp::Ordering::Greater if result == ResultScreen::Pictures => {
            victory(ui, game, scores, teams.team1)?;
        }
        std::cmp::Ordering::Less if result == ResultScreen::Pictures => {
            defeat(ui, game, scores, teams.team2)?;
        }
        _ => result_screen(ui, game, scores, addr::STR_SCORE)?,
    }
    game.career.last_match = Some(LastMatch {
        teams,
        statistics: game_match.statistics(),
    });
    game.career.settle_prize_money(game_match.credits_earned());
    Ok(scores)
}

/// `display_total_match_result` for Brutal Deluxe's cup ties: the victory
/// or defeat picture with the aggregate, or the plain total for a draw.
pub(super) fn display_total_match_result(
    ui: &mut Ui,
    game: &Game,
    teams: (TeamId, TeamId),
    total: (i32, i32),
) -> Flow {
    let scores = PerTeam::new(total.0, total.1);
    let (picture, winner) = match compare_scores(total.0, total.1) {
        std::cmp::Ordering::Equal => return result_screen(ui, game, scores, addr::STR_TOTAL),
        std::cmp::Ordering::Greater => (&game.assets.victory, teams.0),
        std::cmp::Ordering::Less => (&game.assets.defeat, teams.1),
    };
    let page = total_win_picture(game, picture, scores, winner);
    ui.transition(&page)?;
    if winner == teams.0 {
        // `total_victory`.
        ui.play_sound(VICTORY_CHEER);
        wait_any_fire(ui)?;
        ui.play_sound(VICTORY_AFTER);
    }
    wait_any_fire(ui)
}

fn result_screen(ui: &mut Ui, game: &Game, scores: PerTeam<i32>, score_text: usize) -> Flow {
    let mut page = game.titled_page(addr::TITLE_SPEEDBALL_2_PUSH_START);
    page.big_text(&game.assets, &score_string(&game.ram, score_text, scores));
    let verdict = match compare_scores(scores.team1, scores.team2) {
        std::cmp::Ordering::Equal => addr::STR_MATCH_DRAWN,
        std::cmp::Ordering::Greater => addr::STR_BLUE_WIN,
        std::cmp::Ordering::Less => addr::STR_RED_WIN,
    };
    page.big_text(&game.assets, &game.ram.string(verdict));
    ui.transition(&page)?;
    wait_any_fire(ui)
}

/// `format_score_string`: the two scores into "SCORE 000 TO 000".
fn score_string(ram: &MenuRam, address: usize, scores: PerTeam<i32>) -> Vec<u8> {
    let mut text = ram.string(address);
    write_digits(&mut text, 10, scores.team1 as i16, 3);
    write_digits(&mut text, 17, scores.team2 as i16, 3);
    text
}

/// `victory`: Brutal Deluxe won.
fn victory(ui: &mut Ui, game: &Game, scores: PerTeam<i32>, winner: TeamId) -> Flow {
    let page = win_picture(game, &game.assets.victory, scores, winner);
    ui.transition(&page)?;
    ui.play_sound(VICTORY_CHEER);
    wait_any_fire(ui)?;
    ui.play_sound(VICTORY_AFTER);
    wait_any_fire(ui)
}

/// `defeat`: the other team won; the picture names them.
fn defeat(ui: &mut Ui, game: &Game, scores: PerTeam<i32>, winner: TeamId) -> Flow {
    let page = win_picture(game, &game.assets.defeat, scores, winner);
    ui.transition(&page)?;
    wait_any_fire(ui)
}

/// The victory and defeat pictures: "SCORE 000 TO 000" and "A WIN FOR"
/// the winners, centred, in the white font (`format_score_win`).
fn win_picture(game: &Game, picture: &Picture, scores: PerTeam<i32>, winner: TeamId) -> Page {
    let text = score_string(&game.ram, addr::STR_SCORE_WIN, scores);
    picture_with_winner(game, picture, text, 0x12, winner)
}

/// The same for a two-legged tie, with "TOTAL SCORE" (`format_total_score`).
fn total_win_picture(game: &Game, picture: &Picture, scores: PerTeam<i32>, winner: TeamId) -> Page {
    let mut text = game.ram.string(addr::STR_TOTAL_SCORE_WIN);
    write_digits(&mut text, 16, scores.team1 as i16, 3);
    write_digits(&mut text, 23, scores.team2 as i16, 3);
    picture_with_winner(game, picture, text, 0x18, winner)
}

fn picture_with_winner(
    game: &Game,
    picture: &Picture,
    mut text: Vec<u8>,
    win_for_x: usize,
    winner: TeamId,
) -> Page {
    const NAME_ROW: u8 = 0x30;
    let mut page = Page::picture(picture);
    let mut name = game.ram.team_name(winner);
    let x = (40 - (name.len() - 2 + 10)) / 2;
    text[win_for_x] = x as u8;
    name[0] = (x + 10) as u8;
    name[1] = NAME_ROW;
    page.small_text(&game.assets, &text, SmallFont::White, 0);
    page.small_text(&game.assets, &name, SmallFont::White, 0);
    page
}

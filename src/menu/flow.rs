//! The front end: the main menu and the game modes, ported from
//! `main_menu`, `shared_menu_function`, `game_two_player`, `game_knockout`
//! and the result screens.
//!
//! Each function follows its original, waiting on [`Ui`] where the 68000
//! code waited for the next frame.

use std::rc::Rc;

use super::assets::{MenuAssets, SmallFont};
use super::host::{Button, Control};
use super::play::{run_match, Controls};
use super::ram::{addr, MenuRam, MenuSpec, Title};
use super::text::write_digits;
use super::ui::{Flow, Page, Stop, Ui};
use crate::data::AtariRom;
use crate::game::events::Sound;
use crate::game::geometry::{PerTeam, Side};
use crate::game::setup::{MatchRules, BRUTAL_DELUXE};
use crate::game::{Match, MatchSetup};
use crate::graphics::ilbm::Picture;
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
    seed: u64,
}

impl Game {
    pub fn new(
        rom: Rc<AtariRom>,
        ram: MenuRam,
        assets: MenuAssets,
        renderer: MatchRenderer,
        seed: u64,
    ) -> Self {
        Self {
            rom,
            ram,
            assets,
            renderer,
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

    fn titled_page(&self, address: usize) -> Page {
        Page::titled(&self.assets, &self.title(address))
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
    let choice = select_game(ui, game)?;
    match choice {
        MainChoice::OnePlayer => one_player(ui, game)?,
        MainChoice::TwoPlayers => two_players(ui, game)?,
        MainChoice::Demo => demo(ui, game)?,
        // Needs saved replays (a later phase).
        MainChoice::ReplayGoals => return Ok(()),
    }
    let mut page = game.titled_page(addr::TITLE_SPEEDBALL_2_PUSH_START);
    page.big_text(&game.assets, &game.ram.string(addr::STR_GAME_OVER));
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
fn list_menu(ui: &mut Ui, game: &Game, menu: &MenuSpec) -> Flow<usize> {
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
fn wait_any_fire(ui: &mut Ui) -> Flow {
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

// -- One player ------------------------------------------------------------------

/// `game_single_player`: knockout, league, cup or practice.
fn one_player(ui: &mut Ui, game: &mut Game) -> Flow {
    let menu = game.ram.menu(addr::MENU_SELECT_MATCH);
    match list_menu(ui, game, &menu)? {
        0 => knockout(ui, game),
        3 => practice(ui, game),
        // League and cup need the manager screens (later phases).
        _ => Err(Stop::Abort),
    }
}

/// `game_knockout`: beat the other fifteen teams one after another. A draw
/// replays the round; a defeat ends the run. (The fixture screen and the
/// gym before each match come with the manager screens.)
fn knockout(ui: &mut Ui, game: &mut Game) -> Flow {
    let mut round = 1;
    while round <= KNOCKOUT_ROUNDS {
        let rules = MatchRules::one_player(&game.rom, round);
        let scores = play_match(ui, game, rules, ResultScreen::BrutalDeluxeAgainst(round))?;
        match scores.team1.cmp(&scores.team2) {
            std::cmp::Ordering::Equal => continue,
            std::cmp::Ordering::Less => return Ok(()),
            std::cmp::Ordering::Greater => round += 1,
        }
    }
    win_knockout(ui, game)
}

/// `win_knockout` and `win_shared`.
fn win_knockout(ui: &mut Ui, game: &Game) -> Flow {
    ui.transition(&Page::picture(&game.assets.knockout_win))?;
    ui.play_sound(VICTORY_CHEER);
    wait_any_fire(ui)?;
    ui.play_sound(WIN_FANFARE);
    ui.play_sound(WIN_AFTER);
    wait_any_fire(ui)
}

/// `game_practice`: Brutal Deluxe on their own, with no result screen.
fn practice(ui: &mut Ui, game: &mut Game) -> Flow {
    let rules = MatchRules::practice(&game.rom);
    play_match(ui, game, rules, ResultScreen::None).map(|_| ())
}

// -- Two players -----------------------------------------------------------------

/// `game_two_player`: one match, or the best of three or five. Each match
/// is two legs with ends swapped, and level aggregates go to another leg.
/// (The gym before each leg comes with the manager screens.)
fn two_players(ui: &mut Ui, game: &mut Game) -> Flow {
    let menu = game.ram.menu(addr::MENU_SELECT_ONE_THREE_FIVE);
    let matches = list_menu(ui, game, &menu)? * 2 + 1;
    let mut wins = PerTeam::new(0, 0);
    for number in 1..=matches {
        let mut text = game.ram.string(addr::STR_MATCH);
        text[8] = 0x5B + number as u8;
        message(ui, game, addr::TITLE_TWO_PLAYER, &text)?;

        let mut initial_dir = 0;
        let rules = MatchRules::two_players(&game.rom, initial_dir);
        let mut total = play_match(ui, game, rules, ResultScreen::Score)?;
        message(
            ui,
            game,
            addr::TITLE_TWO_PLAYER,
            &game.ram.string(addr::STR_SECOND_LEG),
        )?;
        loop {
            initial_dir ^= 4;
            let rules = MatchRules::two_players(&game.rom, initial_dir);
            let leg = play_match(ui, game, rules, ResultScreen::Score)?;
            total = PerTeam::new(total.team1 + leg.team1, total.team2 + leg.team2);
            display_total_match_result(ui, game, total)?;
            if total.team1 != total.team2 {
                break;
            }
        }
        if total.team2 > total.team1 {
            wins.team2 += 1;
        } else {
            wins.team1 += 1;
        }
        if wins.team1 > matches / 2 || wins.team2 > matches / 2 {
            break;
        }
    }
    let mut text = game.ram.string(addr::STR_PLAYER_WINS);
    text[9] = if wins.team1 < wins.team2 { 0x5D } else { 0x5C };
    message(ui, game, addr::TITLE_TWO_PLAYER, &text)
}

/// A title, one line of big text, then wait for fire.
fn message(ui: &mut Ui, game: &Game, title: usize, text: &[u8]) -> Flow {
    let mut page = game.titled_page(title);
    page.big_text(&game.assets, text);
    ui.transition(&page)?;
    wait_any_fire(ui)
}

// -- Demo ------------------------------------------------------------------------

/// The demo game. (The original then shows the manager and gym screens,
/// which come in a later phase.)
fn demo(ui: &mut Ui, game: &mut Game) -> Flow {
    let rules = MatchRules::demo(&game.rom);
    play_match(ui, game, rules, ResultScreen::Score).map(|_| ())
}

// -- Matches and results ---------------------------------------------------------

/// Which result screen follows a match (`display_match_result`).
#[derive(Clone, Copy)]
enum ResultScreen {
    /// Practice has none.
    None,
    /// The score and who won.
    Score,
    /// Brutal Deluxe against the league team with this index: a victory or
    /// defeat picture, or the score for a draw.
    BrutalDeluxeAgainst(usize),
}

/// `play_match`: the match, then its result screen.
fn play_match(
    ui: &mut Ui,
    game: &mut Game,
    rules: MatchRules,
    result: ResultScreen,
) -> Flow<PerTeam<i32>> {
    let seed = game.next_seed();
    let setup = MatchSetup::new(&game.rom, rules, seed);
    let mut game_match = Match::new(setup, Rc::clone(&game.rom));
    let controls = Controls {
        take_over_on_touch: false,
        hold_at_end: false,
    };
    let scores = run_match(ui, &mut game_match, &game.renderer, controls)?;
    match result {
        ResultScreen::None => {}
        ResultScreen::BrutalDeluxeAgainst(_) if scores.team1 > scores.team2 => {
            victory(ui, game, scores)?;
        }
        ResultScreen::BrutalDeluxeAgainst(opponent) if scores.team1 < scores.team2 => {
            defeat(ui, game, scores, opponent)?;
        }
        ResultScreen::Score | ResultScreen::BrutalDeluxeAgainst(_) => {
            result_screen(ui, game, scores, addr::STR_SCORE)?;
        }
    }
    Ok(scores)
}

/// `display_total_match_result`, for the aggregate of two legs.
fn display_total_match_result(ui: &mut Ui, game: &Game, total: PerTeam<i32>) -> Flow {
    result_screen(ui, game, total, addr::STR_TOTAL)
}

fn result_screen(ui: &mut Ui, game: &Game, scores: PerTeam<i32>, score_text: usize) -> Flow {
    let mut page = game.titled_page(addr::TITLE_SPEEDBALL_2_PUSH_START);
    page.big_text(&game.assets, &score_string(&game.ram, score_text, scores));
    let verdict = match scores.team1.cmp(&scores.team2) {
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
fn victory(ui: &mut Ui, game: &Game, scores: PerTeam<i32>) -> Flow {
    let page = win_picture(game, &game.assets.victory, scores, BRUTAL_DELUXE);
    ui.transition(&page)?;
    ui.play_sound(VICTORY_CHEER);
    wait_any_fire(ui)?;
    ui.play_sound(VICTORY_AFTER);
    wait_any_fire(ui)
}

/// `defeat`: the other team won; the picture names them.
fn defeat(ui: &mut Ui, game: &Game, scores: PerTeam<i32>, winner: usize) -> Flow {
    let page = win_picture(game, &game.assets.defeat, scores, winner);
    ui.transition(&page)?;
    wait_any_fire(ui)
}

/// The victory and defeat pictures: "SCORE 000 TO 000" and "A WIN FOR"
/// the winners, centred, in the white font (`format_score_win`).
fn win_picture(game: &Game, picture: &Picture, scores: PerTeam<i32>, winner: usize) -> Page {
    const WIN_FOR_X: usize = 0x12;
    const NAME_ROW: u8 = 0x30;
    let mut page = Page::picture(picture);
    let mut text = score_string(&game.ram, addr::STR_SCORE_WIN, scores);
    let mut name = game.ram.team_name(winner);
    let x = (40 - (name.len() - 2 + 10)) / 2;
    text[WIN_FOR_X] = x as u8;
    name[0] = (x + 10) as u8;
    name[1] = NAME_ROW;
    page.small_text(&game.assets, &text, SmallFont::White, 0);
    page.small_text(&game.assets, &name, SmallFont::White, 0);
    page
}

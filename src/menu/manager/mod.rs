//! The manager screens: the squad and keypad (`do_manager_screen`), the
//! transfer market (`do_transfer_screen`) and the gym (`do_gym_screen`).
//!
//! Everything is driven from a keypad of 32x32 buttons. Moving off its left
//! or right edge takes the joystick into the squad table (or, in the gym,
//! the upgrade buttons), and off their edges back again, as on the Amiga,
//! which had only one fire button.

pub mod data;
pub mod draw;

use super::flow::{wait_fire, Game};
use super::host::{Button, Control};
use super::panels::{self, Competition as Fixtures};
use super::saving;
use super::ui::{Flow, Stop, Ui};
use crate::game::career::TeamId;
use crate::game::cup::Cup;
use crate::game::events::Sound;
use crate::game::geometry::Side;
use crate::game::league::League;
use crate::game::save::Progress;
use crate::game::squad::{self, Context, Manager, Refusal};
use data::KeyAction;
use draw::Scene;

const MOVE: Sound = Sound {
    id: 0x14,
    priority: 1,
};
const SELECT: Sound = Sound {
    id: 0x17,
    priority: 1,
};
const BOUGHT: Sound = Sound {
    id: 0x22,
    priority: 1,
};
const FLASH: Sound = Sound { id: 8, priority: 2 };

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
    Manager,
    Transfer,
    Gym,
}

/// What the left and right keys go through, and the gym buys for
/// (`buy_mode_table`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BuyMode {
    Player,
    Group,
    Team,
    Market,
}

impl BuyMode {
    fn last_index(self) -> usize {
        match self {
            BuyMode::Player => 11,
            BuyMode::Group => 3,
            BuyMode::Team => 0,
            BuyMode::Market => 17,
        }
    }

    /// How many players an upgrade is bought for.
    pub fn cost_multiplier(self) -> u16 {
        match self {
            BuyMode::Player | BuyMode::Market => 1,
            BuyMode::Group => 3,
            BuyMode::Team => 12,
        }
    }
}

/// Shown instead of the upgrade's name after a purchase fails.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Message {
    NotEnoughCash,
    MaximumBoost,
}

/// The screens' cursors and choices. The original kept them in globals,
/// so they carry over from one visit to the next.
#[derive(Clone, Debug)]
pub struct Cursors {
    pub keypad: (usize, usize),
    pub grid: (usize, usize),
    /// 0, or 3 when the table has scrolled to show the substitutes.
    pub table_offset: usize,
    pub upgrade: (usize, usize),
    pub selected_upgrade: usize,
    previous_selection: Option<usize>,
}

impl Default for Cursors {
    fn default() -> Self {
        Self {
            keypad: (1, 0),
            grid: (0, 0),
            table_offset: 0,
            upgrade: (0, 0),
            selected_upgrade: 0,
            previous_selection: None,
        }
    }
}

/// Everything one frame of a manager screen shows.
pub struct View {
    pub screen: Screen,
    pub mode: BuyMode,
    /// `buy_index`: which player, group or market player is shown.
    pub index: usize,
    pub key_highlight: Option<(usize, usize)>,
    pub key_pressed: bool,
    pub grid: (usize, usize),
    pub table_offset: usize,
    /// The lit upgrade button, while the joystick is on them.
    pub upgrade_cursor: Option<usize>,
    pub selected_upgrade: usize,
    pub message: Option<Message>,
}

impl View {
    /// `player_grid_idx`.
    pub fn grid_slot(&self) -> usize {
        self.grid.1 * 3 + self.grid.0
    }

    /// `player_table_y`: the table moves up to show the substitutes.
    pub fn table_y(&self) -> i32 {
        if self.table_offset == 0 {
            0x24
        } else {
            -0xC
        }
    }
}

/// The league or cup being played, and how far it has got: for the
/// "TABLE" key and for saving.
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
    fn snapshot(&self) -> Option<saving::Snapshot> {
        match self {
            Standings::None => None,
            Standings::League {
                league,
                round,
                game,
                initial_dir,
            } => Some(saving::Snapshot {
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
            } => Some(saving::Snapshot {
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

/// A visit to the manager screens.
struct Session<'a> {
    side: Side,
    context: Context,
    fixtures: Fixtures,
    teams: (TeamId, TeamId),
    standings: Standings<'a>,
    view: View,
}

impl Session<'_> {
    fn player(&self) -> Side {
        self.side
    }
}

/// `do_manager_screen`: before a league or cup match.
pub fn manager_screen(
    ui: &mut Ui,
    game: &mut Game,
    fixtures: Fixtures,
    teams: (TeamId, TeamId),
    standings: Standings,
) -> Flow {
    let context = manager_context(game, fixtures);
    let upgrades = game.ram.upgrades();
    game.career.market.update(&mut game.rng, &upgrades, context);
    let mut session = Session {
        side: Side::Team1,
        context,
        fixtures,
        teams,
        standings,
        view: new_view(game, Screen::Manager, BuyMode::Player),
    };
    transition(ui, game, &session)?;
    if game.career.market.buyable_count > 0 {
        // The transfer key flashes four times when there are players to buy.
        game.cursors.keypad = (0, 0);
        session.view.key_highlight = Some((0, 0));
        for _ in 0..4 {
            ui.play_sound(FLASH);
            show(ui, game, &session);
            ui.vblanks(7)?;
            session.view.key_highlight = None;
            show(ui, game, &session);
            ui.vblanks(6)?;
            session.view.key_highlight = Some((0, 0));
        }
    }
    loop {
        if keypad(ui, game, &mut session)? == KeypadExit::ExitKey {
            break;
        }
        player_grid(ui, game, &mut session)?;
        transition(ui, game, &session)?;
    }
    game.career.market.rounds += 1;
    Ok(())
}

/// `do_gym_screen` on its own: before a knockout match, and for each
/// player before a two-player leg.
pub fn gym_screen(
    ui: &mut Ui,
    game: &mut Game,
    side: Side,
    fixtures: Fixtures,
    teams: (TeamId, TeamId),
) -> Flow {
    let context = manager_context(game, fixtures);
    let mut session = Session {
        side,
        context,
        fixtures,
        teams,
        standings: Standings::None,
        view: new_view(game, Screen::Gym, BuyMode::Team),
    };
    gym(ui, game, &mut session)
}

/// The demo's closing look at the manager and gym screens, with nothing
/// to press (`main_menu` after the demo).
pub fn show_off(ui: &mut Ui, game: &mut Game) -> Flow {
    let context = manager_context(game, Fixtures::League);
    for (screen, mode) in [
        (Screen::Manager, BuyMode::Player),
        (Screen::Gym, BuyMode::Player),
    ] {
        let session = Session {
            side: Side::Team1,
            context,
            fixtures: Fixtures::League,
            teams: (TeamId::League(0), TeamId::League(1)),
            standings: Standings::None,
            view: new_view(game, screen, mode),
        };
        transition(ui, game, &session)?;
        super::flow::wait_any_fire(ui)?;
    }
    Ok(())
}

fn manager_context(game: &Game, fixtures: Fixtures) -> Context {
    let placing = game.career.record(TeamId::League(0)).league_placing;
    Context {
        competition: match fixtures {
            Fixtures::Knockout => squad::Competition::Knockout,
            Fixtures::League => squad::Competition::League,
            Fixtures::Cup => squad::Competition::Cup,
            Fixtures::TwoPlayers => squad::Competition::TwoPlayers,
        },
        manager_mode: game.manager_mode,
        in_division_1: placing <= 8,
    }
}

fn new_view(game: &Game, screen: Screen, mode: BuyMode) -> View {
    let cursors = &game.cursors;
    View {
        screen,
        mode,
        index: 0,
        key_highlight: None,
        key_pressed: false,
        grid: cursors.grid,
        table_offset: cursors.table_offset,
        upgrade_cursor: None,
        selected_upgrade: cursors.selected_upgrade,
        message: None,
    }
}

/// Draws the session's screen.
fn page(game: &Game, session: &Session) -> super::ui::Page {
    let career = &game.career;
    let scene = Scene {
        graphics: &game.assets.manager,
        tables: &game.manager_tables,
        squad: career.squads.get(session.side),
        market: &career.market.players,
        buyable_count: career.market.buyable_count,
        rounds: career.market.rounds,
        cash: *career.money.cash.get(session.side),
        upgrades: &game.ram.upgrades(),
        context: session.context,
    };
    draw::draw(&scene, &session.view)
}

/// Puts the screen up straight away (a screen flip).
fn show(ui: &mut Ui, game: &Game, session: &Session) {
    ui.show_page(&page(game, session));
}

/// `draw_manager_screen` and friends end with `display_fade_transition`.
fn transition(ui: &mut Ui, game: &Game, session: &Session) -> Flow {
    ui.transition(&page(game, session))
}

/// Reads the next fresh press from the session's joystick (or Escape).
fn next_press(ui: &mut Ui, side: Side) -> Flow<Option<Button>> {
    ui.vblank()?;
    Ok(ui.next_press(|b| matches!(b, Button::Joystick(s, _) if s == side) || b == Button::Escape))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum KeypadExit {
    ExitKey,
    /// Moved off the left (`true`) or right edge.
    Edge,
}

/// `do_keypad`: move round the keys and press them. Returns when the
/// exit key is pressed or the joystick leaves the keypad.
fn keypad(ui: &mut Ui, game: &mut Game, session: &mut Session) -> Flow<KeypadExit> {
    loop {
        session.view.key_highlight = Some(game.cursors.keypad);
        session.view.key_pressed = false;
        show(ui, game, session);
        ui.clear_input();
        let pressed = loop {
            if let Some(button) = next_press(ui, session.player())? {
                break button;
            }
        };
        let (x, y) = &mut game.cursors.keypad;
        match pressed {
            Button::Escape => return Err(Stop::Abort),
            Button::Joystick(_, Control::Left) => {
                ui.play_sound(MOVE);
                if *x == 0 {
                    leave_keypad(game);
                    return Ok(KeypadExit::Edge);
                }
                *x -= 1;
                if *y == 2 {
                    *x = 0;
                }
            }
            Button::Joystick(_, Control::Right) => {
                ui.play_sound(MOVE);
                if *x == 3 {
                    leave_keypad(game);
                    return Ok(KeypadExit::Edge);
                }
                *x += 1;
                if *y == 2 {
                    *x = 3;
                }
            }
            Button::Joystick(_, Control::Up) => {
                ui.play_sound(MOVE);
                let edge = *x == 0 || *x == 3;
                *y = match (*y, edge) {
                    (0, true) => 2,
                    (0, false) => 1,
                    (y, true) => y - 1,
                    (y, false) => (y - 1) & 1,
                };
            }
            Button::Joystick(_, Control::Down) => {
                ui.play_sound(MOVE);
                let edge = *x == 0 || *x == 3;
                *y = if edge { (*y + 1) % 3 } else { (*y + 1) & 1 };
            }
            Button::Joystick(_, Control::Fire) => {
                session.view.key_pressed = true;
                show(ui, game, session);
                ui.play_sound(SELECT);
                ui.vblank()?;
                let action = current_key(game, session).map_or(KeyAction::Noop, |k| k.action);
                if action == KeyAction::Exit {
                    return Ok(KeypadExit::ExitKey);
                }
                press_key(ui, game, session, action)?;
            }
            Button::Pause | Button::Step | Button::FastForward => {}
        }
    }
}

/// Leaving the keypad sideways lands on the gym's upgrade buttons on the
/// far side.
fn leave_keypad(game: &mut Game) {
    let (x, y) = game.cursors.keypad;
    game.cursors.upgrade = ((x & 1) ^ 1, y + 1);
}

fn current_key(game: &Game, session: &Session) -> Option<data::Key> {
    let tables = &game.manager_tables;
    let keypad = match session.view.screen {
        Screen::Manager => &tables.manager_keypad,
        Screen::Transfer => &tables.transfer_keypad,
        Screen::Gym => &tables.gym_keypad,
    };
    let (x, y) = game.cursors.keypad;
    keypad[y][x]
}

/// What each key does.
fn press_key(ui: &mut Ui, game: &mut Game, session: &mut Session, action: KeyAction) -> Flow {
    let view = &mut session.view;
    match action {
        KeyAction::Noop | KeyAction::Exit => {}
        KeyAction::Previous => {
            view.index = view.index.checked_sub(1).unwrap_or(view.mode.last_index());
            view.message = None;
        }
        KeyAction::Next => {
            view.index = if view.index >= view.mode.last_index() {
                0
            } else {
                view.index + 1
            };
            view.message = None;
        }
        KeyAction::Transfer => {
            if game.career.market.buyable_count > 0 {
                transfers(ui, game, session)?;
            }
            transition(ui, game, session)?;
        }
        // Only every fourth management round (the key is blank otherwise).
        KeyAction::Save => {
            if game.career.market.rounds & 3 == 0 {
                if let Some(snapshot) = session.standings.snapshot() {
                    let save = saving::snapshot(game, snapshot);
                    saving::save(ui, game, save)?;
                }
                transition(ui, game, session)?;
            }
        }
        KeyAction::Table => {
            match &mut session.standings {
                Standings::League { league, .. } => {
                    league.update_placings(&mut game.career);
                    super::league::show_league_placings(ui, game, league)?;
                }
                Standings::Cup { cup, .. } => {
                    ui.transition(&panels::fixture_list(game, cup))?;
                    wait_fire(ui)?;
                }
                Standings::None => {}
            }
            transition(ui, game, session)?;
        }
        KeyAction::Gym => {
            let mut gym_session = Session {
                side: session.side,
                context: session.context,
                fixtures: session.fixtures,
                teams: session.teams,
                standings: Standings::None,
                view: new_view(game, Screen::Gym, BuyMode::Team),
            };
            gym(ui, game, &mut gym_session)?;
            session.view.mode = BuyMode::Player;
            session.view.index = 0;
            transition(ui, game, session)?;
        }
        KeyAction::Fixture => {
            ui.transition(&panels::next_fixture(game, session.fixtures, session.teams))?;
            wait_fire(ui)?;
            transition(ui, game, session)?;
        }
        KeyAction::Statistics => {
            if let Some(page) = panels::statistics(game) {
                ui.transition(&page)?;
                wait_fire(ui)?;
            }
            transition(ui, game, session)?;
        }
        KeyAction::Substitute => {
            let target = view.grid_slot();
            let market = &game.career.market.players;
            squad::substitute(
                game.career.squads.get_mut(session.side),
                market,
                view.index,
                target,
            );
        }
        KeyAction::TransferPrevious | KeyAction::TransferNext => {
            if game.career.market.buyable_count > 0 {
                let step = if action == KeyAction::TransferNext {
                    1
                } else {
                    -1
                };
                view.index = next_buyable(game, view.index, step);
            }
        }
        KeyAction::TransferBuy => {
            let (index, slot) = (view.index, view.grid_slot());
            let upgrades = game.ram.upgrades();
            let career = &mut game.career;
            let (squad, cash) = (
                career.squads.get_mut(session.side),
                career.money.cash.get_mut(session.side),
            );
            career
                .market
                .buy(index, slot, squad, cash, &upgrades, session.context);
            if career.market.buyable_count > 0 && !career.market.players[index].buyable {
                session.view.index = next_buyable(game, index, 1);
            }
            transition(ui, game, session)?;
        }
        KeyAction::GymPlayer | KeyAction::GymGroup | KeyAction::GymTeam => {
            let mode = match action {
                KeyAction::GymPlayer => BuyMode::Player,
                KeyAction::GymGroup => BuyMode::Group,
                _ => BuyMode::Team,
            };
            if view.mode == mode {
                // Pressing it again moves on (except for the team).
                if mode != BuyMode::Team {
                    return press_key(ui, game, session, KeyAction::Next);
                }
            } else {
                view.mode = mode;
                view.index = 0;
                view.message = None;
            }
        }
        KeyAction::BuyAll => buy_all(ui, game, session)?,
        KeyAction::Buy => buy(ui, game, session)?,
    }
    Ok(())
}

/// `next_buyable_player` / `previous_buyable_player`.
fn next_buyable(game: &Game, from: usize, step: isize) -> usize {
    let players = &game.career.market.players;
    let count = players.len() as isize;
    let mut index = from as isize;
    for _ in 0..count {
        index = (index + step).rem_euclid(count);
        if players[index as usize].buyable {
            return index as usize;
        }
    }
    from
}

/// `do_player_grid`: the cursor in the squad table. Fire picks the place
/// (for substitutions and transfers); moving off the sides goes back to
/// the keypad.
fn player_grid(ui: &mut Ui, game: &mut Game, session: &mut Session) -> Flow {
    session.view.key_highlight = None;
    let tables = &game.manager_tables;
    let (left, right, up, down) = (
        tables.grid_left,
        tables.grid_right,
        tables.grid_up,
        tables.grid_down,
    );
    // Start on the middle face of the row.
    let view = &mut session.view;
    view.grid.0 = match view.grid.1 {
        0 => 0,
        3 => 1,
        _ => 2,
    };
    loop {
        show(ui, game, session);
        ui.clear_input();
        let pressed = loop {
            if let Some(button) = next_press(ui, session.player())? {
                break button;
            }
        };
        let view = &mut session.view;
        let (x, y) = view.grid;
        let moved = |table: &data::GridMoves| table[y][x];
        match pressed {
            Button::Escape => return Err(Stop::Abort),
            Button::Joystick(_, Control::Left) => {
                ui.play_sound(MOVE);
                match moved(&left) {
                    n if n < 0 => {
                        game.cursors.keypad.0 = 3;
                        break;
                    }
                    n => view.grid.0 = n as usize,
                }
            }
            Button::Joystick(_, Control::Right) => {
                ui.play_sound(MOVE);
                match moved(&right) {
                    n if n < 0 => {
                        game.cursors.keypad.0 = 0;
                        break;
                    }
                    n => view.grid.0 = n as usize,
                }
            }
            Button::Joystick(_, Control::Up) => {
                ui.play_sound(MOVE);
                let n = moved(&up);
                if n >= 0 {
                    view.grid.0 = n as usize;
                    if y == 1 && view.table_offset != 0 {
                        view.table_offset = 0;
                    }
                    view.grid.1 -= 1;
                }
            }
            Button::Joystick(_, Control::Down) => {
                ui.play_sound(MOVE);
                let n = moved(&down);
                if n >= 0 {
                    view.grid.0 = n as usize;
                    if y == 2 && view.table_offset != 3 {
                        view.table_offset = 3;
                    }
                    view.grid.1 += 1;
                }
            }
            Button::Joystick(_, Control::Fire) => {
                ui.play_sound(SELECT);
                break;
            }
            _ => {}
        }
    }
    game.cursors.grid = session.view.grid;
    game.cursors.table_offset = session.view.table_offset;
    Ok(())
}

/// `do_transfer_screen`.
fn transfers(ui: &mut Ui, game: &mut Game, session: &mut Session) -> Flow {
    let previous = session.view.screen;
    session.view.screen = Screen::Transfer;
    session.view.mode = BuyMode::Market;
    session.view.index = 0;
    if !game.career.market.players[0].buyable {
        session.view.index = next_buyable(game, 0, 1);
    }
    let result = (|| -> Flow {
        loop {
            transition(ui, game, session)?;
            if keypad(ui, game, session)? == KeypadExit::ExitKey {
                return Ok(());
            }
            player_grid(ui, game, session)?;
        }
    })();
    session.view.screen = previous;
    session.view.mode = BuyMode::Player;
    session.view.index = 0;
    result
}

/// `do_gym_screen`: the keypad, then the upgrade buttons to its right.
/// Fire on an upgrade shows it; fire again on the same one buys it.
fn gym(ui: &mut Ui, game: &mut Game, session: &mut Session) -> Flow {
    game.cursors.keypad.0 = 2;
    session.view.screen = Screen::Gym;
    session.view.mode = BuyMode::Team;
    session.view.index = 0;
    transition(ui, game, session)?;
    game.cursors.upgrade.1 = 2;
    loop {
        let row = game.cursors.upgrade.1;
        game.cursors.keypad.1 = row.saturating_sub(1).min(2);
        if keypad(ui, game, session)? == KeypadExit::ExitKey {
            return Ok(());
        }
        session.view.key_highlight = None;
        upgrade_buttons(ui, game, session)?;
    }
}

/// The upgrade buttons' part of `do_gym_screen`. Returns when the
/// joystick goes back to the keypad.
fn upgrade_buttons(ui: &mut Ui, game: &mut Game, session: &mut Session) -> Flow {
    loop {
        let (x, y) = &mut game.cursors.upgrade;
        *y &= 3;
        session.view.upgrade_cursor = Some(*y * 2 + *x);
        show(ui, game, session);
        ui.clear_input();
        loop {
            let Some(pressed) = next_press(ui, session.player())? else {
                continue;
            };
            let (x, y) = &mut game.cursors.upgrade;
            match pressed {
                Button::Escape => return Err(Stop::Abort),
                Button::Joystick(_, Control::Left) if *x == 0 => {
                    ui.play_sound(MOVE);
                    leave_upgrades(game, session, 3);
                    return Ok(());
                }
                Button::Joystick(_, Control::Right) if *x == 1 => {
                    ui.play_sound(MOVE);
                    leave_upgrades(game, session, 0);
                    return Ok(());
                }
                Button::Joystick(_, Control::Left) => *x -= 1,
                Button::Joystick(_, Control::Right) => *x += 1,
                Button::Joystick(_, Control::Up) => *y = y.wrapping_sub(1),
                Button::Joystick(_, Control::Down) => *y += 1,
                Button::Joystick(_, Control::Fire) => {
                    ui.play_sound(SELECT);
                    let chosen = *y * 2 + *x;
                    // `select_current_upgrade`, then buy on a second press.
                    session.view.selected_upgrade = chosen;
                    session.view.message = None;
                    game.cursors.selected_upgrade = chosen;
                    show(ui, game, session);
                    ui.vblank()?;
                    if game.cursors.previous_selection == Some(chosen) {
                        buy(ui, game, session)?;
                    }
                    game.cursors.previous_selection = Some(chosen);
                    continue;
                }
                _ => continue,
            }
            ui.play_sound(MOVE);
            break;
        }
    }
}

fn leave_upgrades(game: &mut Game, session: &mut Session, keypad_x: usize) {
    game.cursors.previous_selection = None;
    session.view.upgrade_cursor = None;
    game.cursors.keypad.0 = keypad_x;
}

/// `keypad_fn_buy`: the selected upgrade for the current player, group or
/// team.
fn buy(ui: &mut Ui, game: &mut Game, session: &mut Session) -> Flow {
    let upgrade = game.manager_tables.upgrades[session.view.selected_upgrade].upgrade;
    let career = &mut game.career;
    let mut manager = Manager {
        squad: career.squads.get_mut(session.side),
        cash: career.money.cash.get_mut(session.side),
        market: &career.market.players,
        context: session.context,
    };
    let view = &mut session.view;
    let result = if *manager.cash == 0 {
        Err(Refusal::NotEnoughCash)
    } else {
        match view.mode {
            BuyMode::Player => manager.buy_for_player(view.index, upgrade),
            BuyMode::Group => manager.buy_for_group(view.index, upgrade),
            BuyMode::Team => manager.buy_for_team(upgrade),
            BuyMode::Market => Ok(()),
        }
    };
    view.message = match result {
        Ok(()) => {
            ui.play_sound(BOUGHT);
            None
        }
        // Only a single player's purchase says it is at the maximum.
        Err(Refusal::MaximumBoost) if view.mode != BuyMode::Player => None,
        Err(Refusal::MaximumBoost) => Some(Message::MaximumBoost),
        Err(Refusal::NotEnoughCash) => Some(Message::NotEnoughCash),
    };
    show(ui, game, session);
    ui.vblank()?;
    // The message only lasts until the description is next drawn.
    session.view.message = None;
    Ok(())
}

/// `keypad_fn_gym_buy_all`: every upgrade in turn, stopping when the
/// money runs out.
fn buy_all(ui: &mut Ui, game: &mut Game, session: &mut Session) -> Flow {
    for index in game.manager_tables.buy_all_order.clone() {
        session.view.selected_upgrade = index;
        game.cursors.selected_upgrade = index;
        show(ui, game, session);
        ui.vblank()?;
        let upgrade = game.manager_tables.upgrades[index].upgrade;
        let cost = upgrade
            .price
            .wrapping_mul(session.view.mode.cost_multiplier());
        if cost as i16 > *game.career.money.cash.get(session.side) as i16 {
            session.view.message = Some(Message::NotEnoughCash);
            show(ui, game, session);
            ui.vblank()?;
            session.view.message = None;
            return Ok(());
        }
        buy(ui, game, session)?;
    }
    Ok(())
}

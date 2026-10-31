//! `do_gym_screen`: the keypad, then the upgrade buttons to its right.
//! Fire on an upgrade shows it; fire again on the same one buys it.

use super::keypad::{keypad, KeypadExit};
use super::session::Session;
use super::view::{BuyMode, Message, Screen};
use super::{BOUGHT, MOVE, SELECT};
use crate::game::squad::{Manager, Refusal};
use crate::menu::flow::Game;
use crate::menu::host::{Button, Control};
use crate::menu::ui::{Flow, Stop, Ui};

pub(super) fn gym(ui: &mut Ui, game: &mut Game, session: &mut Session) -> Flow {
    game.cursors.keypad.0 = 2;
    session.view.screen = Screen::Gym;
    session.view.mode = BuyMode::Team;
    session.view.index = 0;
    session.transition(ui, game)?;
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

/// The upgrade buttons' part of `do_gym_screen`, two columns of four.
/// Returns when the joystick goes back to the keypad.
fn upgrade_buttons(ui: &mut Ui, game: &mut Game, session: &mut Session) -> Flow {
    loop {
        let (x, y) = &mut game.cursors.upgrade;
        *y &= 3;
        session.view.upgrade_cursor = Some(*y * 2 + *x);
        session.show(ui, game);
        ui.clear_input();
        loop {
            let Some(pressed) = session.next_press(ui)? else {
                continue;
            };
            let (x, y) = &mut game.cursors.upgrade;
            match pressed {
                Button::Escape => return Err(Stop::Abort),
                Button::Joystick(_, Control::Left) if *x == 0 => {
                    ui.play_sound(MOVE);
                    leave(game, session, 3);
                    return Ok(());
                }
                Button::Joystick(_, Control::Right) if *x == 1 => {
                    ui.play_sound(MOVE);
                    leave(game, session, 0);
                    return Ok(());
                }
                Button::Joystick(_, Control::Left) => *x -= 1,
                Button::Joystick(_, Control::Right) => *x += 1,
                Button::Joystick(_, Control::Up) => *y = y.wrapping_sub(1),
                Button::Joystick(_, Control::Down) => *y += 1,
                Button::Joystick(_, Control::Fire) => {
                    let chosen = *y * 2 + *x;
                    choose(ui, game, session, chosen)?;
                    continue;
                }
                _ => continue,
            }
            ui.play_sound(MOVE);
            break;
        }
    }
}

/// `select_current_upgrade`, then buy on a second press of the same one.
fn choose(ui: &mut Ui, game: &mut Game, session: &mut Session, chosen: usize) -> Flow {
    ui.play_sound(SELECT);
    session.view.selected_upgrade = chosen;
    session.view.message = None;
    game.cursors.selected_upgrade = chosen;
    session.show(ui, game);
    ui.vblank()?;
    if game.cursors.previous_selection == Some(chosen) {
        buy(ui, game, session)?;
    }
    game.cursors.previous_selection = Some(chosen);
    Ok(())
}

fn leave(game: &mut Game, session: &mut Session, keypad_x: usize) {
    game.cursors.previous_selection = None;
    session.view.upgrade_cursor = None;
    game.cursors.keypad.0 = keypad_x;
}

/// `keypad_fn_buy`: the selected upgrade for the current player, group or
/// team.
pub(super) fn buy(ui: &mut Ui, game: &mut Game, session: &mut Session) -> Flow {
    let upgrade = game.tables.upgrades[session.view.selected_upgrade];
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
    flash_message(ui, game, session)
}

/// `keypad_fn_gym_buy_all`: every upgrade in turn, stopping when the
/// money runs out.
pub(super) fn buy_all(ui: &mut Ui, game: &mut Game, session: &mut Session) -> Flow {
    for index in game.manager_tables.buy_all_order.clone() {
        session.view.selected_upgrade = index;
        game.cursors.selected_upgrade = index;
        session.show(ui, game);
        ui.vblank()?;
        let upgrade = game.tables.upgrades[index];
        let cost = upgrade
            .price
            .wrapping_mul(session.view.mode.cost_multiplier());
        if cost as i16 > *game.career.money.cash.get(session.side) as i16 {
            session.view.message = Some(Message::NotEnoughCash);
            return flash_message(ui, game, session);
        }
        buy(ui, game, session)?;
    }
    Ok(())
}

/// Shows the screen for a frame. A message only lasts until the
/// description is next drawn.
fn flash_message(ui: &mut Ui, game: &Game, session: &mut Session) -> Flow {
    session.show(ui, game);
    ui.vblank()?;
    session.view.message = None;
    Ok(())
}

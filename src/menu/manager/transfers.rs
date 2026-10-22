//! `do_transfer_screen`: the players for sale.

use super::keypad::{keypad, KeypadExit};
use super::session::Session;
use super::squad_table::squad_table;
use super::view::{BuyMode, Screen};
use crate::menu::flow::Game;
use crate::menu::ui::{Flow, Ui};

/// The transfer screen, entered from the manager screen's keypad.
pub(super) fn transfers(ui: &mut Ui, game: &mut Game, session: &mut Session) -> Flow {
    let previous = session.view.screen;
    session.view.screen = Screen::Transfer;
    session.view.mode = BuyMode::Market;
    session.view.index = 0;
    if !game.career.market.players[0].buyable {
        session.view.index = next_buyable(game, 0, 1);
    }
    let result = run(ui, game, session);
    session.view.screen = previous;
    session.view.mode = BuyMode::Player;
    session.view.index = 0;
    result
}

fn run(ui: &mut Ui, game: &mut Game, session: &mut Session) -> Flow {
    loop {
        session.transition(ui, game)?;
        if keypad(ui, game, session)? == KeypadExit::ExitKey {
            return Ok(());
        }
        squad_table(ui, game, session)?;
    }
}

/// The transfer screen's previous and next keys, if anyone is for sale.
pub(super) fn step_buyable(game: &Game, session: &mut Session, step: isize) {
    if game.career.market.buyable_count > 0 {
        session.view.index = next_buyable(game, session.view.index, step);
    }
}

/// `keypad_fn_transfer_buy`: the market player shown into the chosen place.
pub(super) fn buy(ui: &mut Ui, game: &mut Game, session: &mut Session) -> Flow {
    let (index, slot) = (session.view.index, session.view.grid_slot());
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
    session.transition(ui, game)
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

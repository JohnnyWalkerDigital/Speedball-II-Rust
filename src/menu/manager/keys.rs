//! What each keypad key does.

use super::data::KeyAction;
use super::session::Session;
use super::standings::Standings;
use super::view::BuyMode;
use super::{gym, transfers};
use crate::game::squad;
use crate::menu::flow::{wait_fire, Game};
use crate::menu::ui::{Flow, Ui};
use crate::menu::{league, panels, saving};

pub(super) fn press_key(
    ui: &mut Ui,
    game: &mut Game,
    session: &mut Session,
    action: KeyAction,
) -> Flow {
    match action {
        KeyAction::Noop | KeyAction::Exit => Ok(()),
        KeyAction::Previous => {
            session.view.step_index(false);
            Ok(())
        }
        KeyAction::Next => {
            session.view.step_index(true);
            Ok(())
        }
        KeyAction::Transfer => {
            if game.career.market.buyable_count > 0 {
                transfers::transfers(ui, game, session)?;
            }
            session.transition(ui, game)
        }
        KeyAction::Save => save(ui, game, session),
        KeyAction::Table => table(ui, game, session),
        KeyAction::Gym => {
            let mut gym_session = session.gym(game);
            gym::gym(ui, game, &mut gym_session)?;
            session.view.mode = BuyMode::Player;
            session.view.index = 0;
            session.transition(ui, game)
        }
        KeyAction::Fixture => {
            ui.transition(&panels::next_fixture(
                game,
                session.competition,
                session.teams,
            ))?;
            wait_fire(ui)?;
            session.transition(ui, game)
        }
        KeyAction::Statistics => {
            if let Some(page) = panels::statistics(game) {
                ui.transition(&page)?;
                wait_fire(ui)?;
            }
            session.transition(ui, game)
        }
        KeyAction::Substitute => {
            squad::substitute(
                game.career.squads.get_mut(session.side),
                &game.career.market.players,
                session.view.index,
                session.view.grid_slot(),
            );
            Ok(())
        }
        KeyAction::TransferPrevious => {
            transfers::step_buyable(game, session, -1);
            Ok(())
        }
        KeyAction::TransferNext => {
            transfers::step_buyable(game, session, 1);
            Ok(())
        }
        KeyAction::TransferBuy => transfers::buy(ui, game, session),
        KeyAction::GymPlayer => gym_mode(ui, game, session, BuyMode::Player),
        KeyAction::GymGroup => gym_mode(ui, game, session, BuyMode::Group),
        KeyAction::GymTeam => gym_mode(ui, game, session, BuyMode::Team),
        KeyAction::BuyAll => gym::buy_all(ui, game, session),
        KeyAction::Buy => gym::buy(ui, game, session),
    }
}

/// Saves the game, when the market allows it (the key is blank
/// otherwise).
fn save(ui: &mut Ui, game: &mut Game, session: &mut Session) -> Flow {
    if !game.career.market.may_save() {
        return Ok(());
    }
    if let Some(snapshot) = session.standings.snapshot() {
        let save = saving::snapshot(game, snapshot);
        saving::save(ui, game, save)?;
    }
    session.transition(ui, game)
}

/// The league tables or the cup's fixture list.
fn table(ui: &mut Ui, game: &mut Game, session: &mut Session) -> Flow {
    match &mut session.standings {
        Standings::League { league, .. } => {
            league.update_placings(&mut game.career);
            league::show_league_placings(ui, game, league)?;
        }
        Standings::Cup { cup, .. } => {
            ui.transition(&panels::fixture_list(game, cup))?;
            wait_fire(ui)?;
        }
        Standings::None => {}
    }
    session.transition(ui, game)
}

/// The gym's player, group and team keys. Pressing the current one again
/// moves on (except for the team).
fn gym_mode(ui: &mut Ui, game: &mut Game, session: &mut Session, mode: BuyMode) -> Flow {
    if session.view.mode != mode {
        session.view.switch_mode(mode);
        Ok(())
    } else if mode != BuyMode::Team {
        press_key(ui, game, session, KeyAction::Next)
    } else {
        Ok(())
    }
}

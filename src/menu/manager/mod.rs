//! The manager screens: the squad and keypad (`do_manager_screen`), the
//! transfer market (`do_transfer_screen`) and the gym (`do_gym_screen`).
//!
//! Everything is driven from a keypad of 32x32 buttons. Moving off its left
//! or right edge takes the joystick into the squad table (or, in the gym,
//! the upgrade buttons), and off their edges back again, as on the Amiga,
//! which had only one fire button.
//!
//! - [`session`]: a visit (whose squad, what for, and what is on screen),
//!   drawn by [`draw`] from the tables in [`data`];
//! - [`keypad`] moves round the keys, and [`keys`] does what they say;
//! - [`squad_table`], [`transfers`] and [`gym`] are the other places the
//!   joystick goes.

pub mod data;
pub mod draw;
mod gym;
mod keypad;
mod keys;
mod session;
mod squad_table;
mod standings;
mod transfers;
mod view;

use super::flow::{wait_any_fire, Game};
use super::panels::Competition as Fixtures;
use super::ui::{Flow, Ui};
use crate::game::career::TeamId;
use crate::game::events::Sound;
use crate::game::geometry::Side;
use keypad::{keypad, KeypadExit};
use session::Session;
use squad_table::squad_table;

pub use standings::Standings;
pub use view::Cursors;
use view::{BuyMode, Screen};

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

/// `do_manager_screen`: before a league or cup match.
pub fn manager_screen(
    ui: &mut Ui,
    game: &mut Game,
    fixtures: Fixtures,
    teams: (TeamId, TeamId),
    standings: Standings,
) -> Flow {
    let mut session = Session::new(
        game,
        Side::Team1,
        fixtures,
        teams,
        standings,
        Screen::Manager,
        BuyMode::Player,
    );
    let upgrades = game.ram.upgrades();
    game.career
        .market
        .update(&mut game.rng, &upgrades, session.context);
    session.transition(ui, game)?;
    if game.career.market.buyable_count > 0 {
        flash_transfer_key(ui, game, &mut session)?;
    }
    loop {
        if keypad(ui, game, &mut session)? == KeypadExit::ExitKey {
            break;
        }
        squad_table(ui, game, &mut session)?;
        session.transition(ui, game)?;
    }
    game.career.market.rounds += 1;
    Ok(())
}

/// The transfer key flashes four times when there are players to buy,
/// and the cursor moves onto it.
fn flash_transfer_key(ui: &mut Ui, game: &mut Game, session: &mut Session) -> Flow {
    const TRANSFER_KEY: (usize, usize) = (0, 0);
    game.cursors.keypad = TRANSFER_KEY;
    session.view.key_highlight = Some(TRANSFER_KEY);
    for _ in 0..4 {
        ui.play_sound(FLASH);
        session.show(ui, game);
        ui.vblanks(7)?;
        session.view.key_highlight = None;
        session.show(ui, game);
        ui.vblanks(6)?;
        session.view.key_highlight = Some(TRANSFER_KEY);
    }
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
    let mut session = Session::new(
        game,
        side,
        fixtures,
        teams,
        Standings::None,
        Screen::Gym,
        BuyMode::Team,
    );
    gym::gym(ui, game, &mut session)
}

/// The demo's closing look at the manager and gym screens, with nothing
/// to press (`main_menu` after the demo).
pub fn show_off(ui: &mut Ui, game: &mut Game) -> Flow {
    for screen in [Screen::Manager, Screen::Gym] {
        let session = Session::new(
            game,
            Side::Team1,
            Fixtures::League,
            (TeamId::League(0), TeamId::League(1)),
            Standings::None,
            screen,
            BuyMode::Player,
        );
        session.transition(ui, game)?;
        wait_any_fire(ui)?;
    }
    Ok(())
}

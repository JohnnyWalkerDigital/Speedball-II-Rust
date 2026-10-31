//! A visit to the manager screens: whose squad, what for, and what is on
//! screen.

use super::draw::{self, Scene};
use super::standings::Standings;
use super::view::{BuyMode, Screen, View};
use crate::game::career::{TeamId, BRUTAL_DELUXE};
use crate::game::geometry::Side;
use crate::game::squad::{self, Context};
use crate::menu::flow::Game;
use crate::menu::host::Button;
use crate::menu::ui::{Flow, Page, Ui};

pub(super) struct Session<'a> {
    /// Whose squad and joystick.
    pub side: Side,
    pub context: Context,
    pub competition: squad::Competition,
    pub teams: (TeamId, TeamId),
    pub standings: Standings<'a>,
    pub view: View,
}

impl<'a> Session<'a> {
    pub fn new(
        game: &Game,
        side: Side,
        competition: squad::Competition,
        teams: (TeamId, TeamId),
        standings: Standings<'a>,
        screen: Screen,
        mode: BuyMode,
    ) -> Self {
        Self {
            side,
            context: context(game, competition),
            competition,
            teams,
            standings,
            view: View::new(&game.cursors, screen, mode),
        }
    }

    /// The gym, entered from this session's keypad.
    pub fn gym(&self, game: &Game) -> Session<'static> {
        Session {
            side: self.side,
            context: self.context,
            competition: self.competition,
            teams: self.teams,
            standings: Standings::None,
            view: View::new(&game.cursors, Screen::Gym, BuyMode::Team),
        }
    }

    /// Draws the session's screen.
    fn page(&self, game: &Game) -> Page {
        let career = &game.career;
        let scene = Scene {
            graphics: &game.assets.manager,
            tables: &game.manager_tables,
            squad: career.squads.get(self.side),
            market: &career.market.players,
            buyable_count: career.market.buyable_count,
            may_save: career.market.may_save(),
            cash: *career.money.cash.get(self.side),
            upgrades: &game.tables.upgrades,
            context: self.context,
        };
        draw::draw(&scene, &self.view)
    }

    /// Puts the screen up straight away (a screen flip).
    pub fn show(&self, ui: &mut Ui, game: &Game) {
        ui.show_page(&self.page(game));
    }

    /// Fades over to the screen as it is now drawn.
    pub fn transition(&self, ui: &mut Ui, game: &Game) -> Flow {
        ui.transition(&self.page(game))
    }

    /// Shows the screen, then waits for the next fresh press of the
    /// session's joystick (or Escape).
    pub fn wait_press(&self, ui: &mut Ui, game: &Game) -> Flow<Button> {
        self.show(ui, game);
        ui.clear_input();
        loop {
            if let Some(button) = self.next_press(ui)? {
                return Ok(button);
            }
        }
    }

    /// Waits a frame for a fresh press of the session's joystick (or
    /// Escape).
    pub fn next_press(&self, ui: &mut Ui) -> Flow<Option<Button>> {
        ui.vblank()?;
        let side = self.side;
        Ok(ui.next_press(|b| {
            matches!(b, Button::Joystick(s, _) if s == side) || b == Button::Escape
        }))
    }
}

/// The rules the gym and market follow for this competition.
fn context(game: &Game, competition: squad::Competition) -> Context {
    let placing = game.career.record(BRUTAL_DELUXE).league_placing;
    Context {
        competition,
        manager_mode: game.manager_mode,
        in_division_1: placing <= 8,
    }
}

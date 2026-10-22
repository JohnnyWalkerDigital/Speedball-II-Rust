//! `do_keypad`: moving round the keypad and pressing its keys.

use super::data::{Key, KeyAction};
use super::keys::press_key;
use super::session::Session;
use super::view::Screen;
use super::{MOVE, SELECT};
use crate::menu::flow::Game;
use crate::menu::host::{Button, Control};
use crate::menu::ui::{Flow, Stop, Ui};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum KeypadExit {
    ExitKey,
    /// The joystick moved off the keypad's left or right edge.
    Edge,
}

/// The keypad's three rows of four, the bottom row having only its ends.
/// Returns where the cursor goes, or `None` off the left or right edge.
fn step(cursor: (usize, usize), control: Control) -> Option<(usize, usize)> {
    let (x, y) = cursor;
    let edge = x == 0 || x == 3;
    Some(match control {
        Control::Left if x == 0 => return None,
        Control::Right if x == 3 => return None,
        Control::Left => (if y == 2 { 0 } else { x - 1 }, y),
        Control::Right => (if y == 2 { 3 } else { x + 1 }, y),
        Control::Up => {
            let y = match (y, edge) {
                (0, true) => 2,
                (0, false) => 1,
                (y, true) => y - 1,
                (y, false) => (y - 1) & 1,
            };
            (x, y)
        }
        Control::Down => (x, if edge { (y + 1) % 3 } else { (y + 1) & 1 }),
        Control::Fire => (x, y),
    })
}

/// Runs the keypad until the exit key is pressed or the joystick leaves it.
pub(super) fn keypad(ui: &mut Ui, game: &mut Game, session: &mut Session) -> Flow<KeypadExit> {
    loop {
        session.view.key_highlight = Some(game.cursors.keypad);
        session.view.key_pressed = false;
        match session.wait_press(ui, game)? {
            Button::Escape => return Err(Stop::Abort),
            Button::Joystick(_, Control::Fire) => {
                session.view.key_pressed = true;
                session.show(ui, game);
                ui.play_sound(SELECT);
                ui.vblank()?;
                let action = current_key(game, session).map_or(KeyAction::Noop, |k| k.action);
                if action == KeyAction::Exit {
                    return Ok(KeypadExit::ExitKey);
                }
                press_key(ui, game, session, action)?;
            }
            Button::Joystick(_, control) => {
                ui.play_sound(MOVE);
                match step(game.cursors.keypad, control) {
                    Some(cursor) => game.cursors.keypad = cursor,
                    None => {
                        leave(game);
                        return Ok(KeypadExit::Edge);
                    }
                }
            }
            _ => {}
        }
    }
}

/// Leaving the keypad sideways lands on the gym's upgrade buttons on the
/// far side.
fn leave(game: &mut Game) {
    let (x, y) = game.cursors.keypad;
    game.cursors.upgrade = ((x & 1) ^ 1, y + 1);
}

fn current_key(game: &Game, session: &Session) -> Option<Key> {
    let tables = &game.manager_tables;
    let keypad = match session.view.screen {
        Screen::Manager => &tables.manager_keypad,
        Screen::Transfer => &tables.transfer_keypad,
        Screen::Gym => &tables.gym_keypad,
    };
    let (x, y) = game.cursors.keypad;
    keypad[y][x]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bottom_row_has_only_its_ends() {
        assert_eq!(step((0, 1), Control::Down), Some((0, 2)));
        assert_eq!(
            step((1, 1), Control::Down),
            Some((1, 0)),
            "middle keys skip it"
        );
        assert_eq!(step((0, 2), Control::Right), Some((3, 2)));
        assert_eq!(step((3, 0), Control::Up), Some((3, 2)));
        assert_eq!(step((2, 0), Control::Up), Some((2, 1)));
    }

    #[test]
    fn the_sides_lead_off_the_keypad() {
        assert_eq!(step((0, 0), Control::Left), None);
        assert_eq!(step((3, 2), Control::Right), None);
        assert_eq!(step((1, 0), Control::Left), Some((0, 0)));
    }
}

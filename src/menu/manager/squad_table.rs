//! `do_player_grid`: the cursor in the squad table. Fire picks the place
//! (for substitutions and transfers); moving off the sides goes back to
//! the keypad.

use super::data::GridMoves;
use super::session::Session;
use super::{MOVE, SELECT};
use crate::menu::flow::Game;
use crate::menu::host::{Button, Control};
use crate::menu::ui::{Flow, Stop, Ui};

pub(super) fn squad_table(ui: &mut Ui, game: &mut Game, session: &mut Session) -> Flow {
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
        let pressed = session.wait_press(ui, game)?;
        let view = &mut session.view;
        let (x, y) = view.grid;
        let moved = |table: &GridMoves| table[y][x];
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
                    // The table scrolls back up to the first rows.
                    if y == 1 {
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
                    // And down to show the substitutes.
                    if y == 2 {
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

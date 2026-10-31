//! The Amiga's sprite banks.
//!
//! The port numbers sprites as the Atari ST did, which is what the entity
//! records and animation scripts hold. The Amiga drew from four banks, and
//! its goal replays store sprites by bank and index, so saving and showing
//! replays needs the Amiga's numbering.

use super::entity::{Entity, EntityId, Flags, Gfx, Think};
use super::geometry::Point;
use crate::data::animations::ScriptRef;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bank {
    /// `sprites_ball`'s bank: the ball, items, indicators (16x16).
    Entities,
    /// `sprites_players`: players and medics (32x32).
    Players,
    /// `sprites_big_ball` (32x32).
    BigBall,
    /// `sprites_launcher`: the launcher, the domes and the electrobounces
    /// (32x32, five bitplanes).
    Launcher,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AmigaSprite {
    pub bank: Bank,
    pub index: i32,
}

/// The first ST player-bank frame after the Amiga's extra one (the
/// double dome).
const FIRST_SHIFTED_FRAME: i32 = 0x87;
/// The ST token the Amiga's entity bank leaves out.
const UNUSED_TOKEN: i32 = 67;
/// The domes' only frame on the Amiga; the rest are pitch tiles.
const DOME_HIT_FRAME: i32 = 0x88;
/// Where the big ball and launcher banks follow the players' in the port's
/// sheet of 32x32 sprites.
pub const BIG_BALL_FIRST: i32 = 130;
pub const LAUNCHER_FIRST: i32 = 135;

/// The Amiga sprite `e` is drawn with, or `None` if the Amiga draws no
/// sprite for it.
pub fn amiga_sprite(e: &Entity) -> Option<AmigaSprite> {
    let sprite = |bank, index| Some(AmigaSprite { bank, index });
    if e.is_electrobounce() {
        return sprite(
            Bank::Launcher,
            i32::from(e.id != EntityId::Electrobounce(0)),
        );
    }
    if matches!(e.id, EntityId::BounceDome(_)) && e.sprite != DOME_HIT_FRAME {
        return None;
    }
    match e.gfx {
        Gfx::Entities => {
            let mut index = e.sprite;
            if e.is_ball() && e.is_charged() {
                index += 11;
            }
            if index > UNUSED_TOKEN {
                index -= 1;
            }
            sprite(Bank::Entities, index)
        }
        Gfx::Players => {
            let mut index = e.sprite;
            if index >= FIRST_SHIFTED_FRAME {
                index += 1;
            }
            match index {
                i if i >= LAUNCHER_FIRST => sprite(Bank::Launcher, i - LAUNCHER_FIRST),
                i if i >= BIG_BALL_FIRST => sprite(Bank::BigBall, i - BIG_BALL_FIRST),
                i => sprite(Bank::Players, i),
            }
        }
        Gfx::BigBall => sprite(Bank::BigBall, e.sprite),
        _ => None,
    }
}

/// An entity the port's renderer draws as `sprite`, with its top-left
/// corner at `top_left` on the pitch: the inverse of [`amiga_sprite`], for
/// sprites read back from a replay. `team1` picks team 1's colours for the
/// players' bank.
pub fn entity_for(sprite: AmigaSprite, team1: bool, top_left: Point, size: i32) -> Entity {
    let index = sprite.index;
    let (id, gfx, st_index) = match sprite.bank {
        Bank::Entities => {
            let st_index = if index >= UNUSED_TOKEN {
                index + 1
            } else {
                index
            };
            (EntityId::Unattached, Gfx::Entities, st_index)
        }
        Bank::Players => {
            // Each team's first player.
            let first = if team1 { 0 } else { 9 };
            (EntityId::Player(first), Gfx::Players, index)
        }
        Bank::BigBall => (EntityId::BigBall, Gfx::BigBall, index),
        Bank::Launcher => match index {
            0 | 1 => (EntityId::Electrobounce(index as u8), Gfx::Players, 0),
            2 => (EntityId::BounceDome(0), Gfx::Players, DOME_HIT_FRAME),
            _ => (EntityId::Launcher, Gfx::Players, LAUNCHER_FIRST + index - 1),
        },
    };
    let mut flags = Flags::default();
    flags.set_bit3(team1);
    Entity {
        id,
        gfx,
        think: Think::Idle,
        script: ScriptRef(0),
        width: size,
        height: size,
        pos: top_left,
        velocity: Point::ZERO,
        script_index: 0,
        dir: 0,
        timer: 0,
        sprite: st_index,
        flags,
        origin: Point::new(size / 2, size / 2),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replayed_sprites_draw_as_recorded() {
        let sprites = [
            (Bank::Entities, 0),
            (Bank::Entities, 66),
            (Bank::Entities, 67),
            (Bank::Entities, 90),
            (Bank::Players, 0),
            (Bank::Players, 127),
            (Bank::BigBall, 3),
            (Bank::Launcher, 0),
            (Bank::Launcher, 1),
            (Bank::Launcher, 2),
            (Bank::Launcher, 9),
        ];
        for (bank, index) in sprites {
            let sprite = AmigaSprite { bank, index };
            for team1 in [false, true] {
                let e = entity_for(sprite, team1, Point::new(100, 200), 32);
                assert_eq!(amiga_sprite(&e), Some(sprite), "{sprite:?}");
            }
        }
    }

    #[test]
    fn the_amiga_has_no_sprites_for_the_domes_other_frames() {
        let mut dome = entity_for(
            AmigaSprite {
                bank: Bank::Launcher,
                index: 2,
            },
            false,
            Point::ZERO,
            32,
        );
        dome.sprite = 0x87;
        assert_eq!(amiga_sprite(&dome), None);
    }
}

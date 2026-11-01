//! The match engine's lookup tables: movement, the skill curves, tackling,
//! equipment, the AI's support positions and where sprites hold the ball.
//!
//! Read from the ST disk ([`Lookups::read`]) or an asset pack. A lookup
//! outside a table reads the original memory around it, as the original
//! game did, so out-of-range indices behave exactly as before.

use serde::{Deserialize, Serialize};

use super::addr;
use super::rom::Rom;

/// Directions are 0 (up) to 7, clockwise.
pub const DIRECTIONS: usize = 8;
/// Speeds index the velocity table from 0 to 8.
pub const SPEEDS: usize = 9;
/// Skill curves have one entry for every ten points from 100 (skills of
/// 91 to 109 share the first).
pub const SKILL_STEPS: usize = 16;
pub const SUPPORT_COLUMNS: usize = 9;
const SUPPORT_ROWS: usize = 6;
const STATUS_LEDS: usize = 6;
const EQUIPMENT: usize = 8;
/// The sprites a carrier can show, up to the next table in memory.
const BALL_OFFSETS: usize = 117;
const SPRITE_ORIGINS: usize = 120;
/// Steps in each multiplier path, up to the next one in memory.
pub const PATH_STEPS: usize = 50;

/// The ball's ways through the score-multiplier ramps, one after another
/// in memory in this order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MultiplierPath {
    LeftUp,
    LeftDown,
    RightDown,
    RightUp,
}

impl MultiplierPath {
    fn address(self) -> i32 {
        match self {
            MultiplierPath::LeftUp => addr::MULTIPLIER_PATH_LEFT_UP,
            MultiplierPath::LeftDown => addr::MULTIPLIER_PATH_LEFT_DOWN,
            MultiplierPath::RightDown => addr::MULTIPLIER_PATH_RIGHT_DOWN,
            MultiplierPath::RightUp => addr::MULTIPLIER_PATH_RIGHT_UP,
        }
    }

    /// Where `step` is in memory, for reads past the table.
    pub(super) fn step_address(self, step: i32) -> i32 {
        self.address() + step * 2
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Lookups {
    /// By speed, then direction: `[x, y]` pixels a frame.
    pub velocity: Vec<Vec<[i32; 2]>>,
    /// The direction for each joystick bit set (up 1, down 2, left 4,
    /// right 8), or -1 for none.
    pub direction_from_joystick: Vec<i32>,
    /// A direction bounced off a side wall.
    pub mirror_x: Vec<i32>,
    /// A direction bounced off an end wall.
    pub mirror_y: Vec<i32>,
    /// The direction a diving goalkeeper knocks the ball, by the keeper's
    /// direction: for the team attacking up, then down.
    pub goalie_deflect: [Vec<i32>; 2],
    /// Frames between a computer player's decisions, by intelligence.
    pub think_delay: Vec<i32>,
    /// How many frames ahead (as a power of two) a player predicts where
    /// others will be, by intelligence.
    pub prediction_shift: Vec<i32>,
    /// Extra jump, by speed.
    pub jump_bonus: Vec<i32>,
    /// Taken off a defender's defence when tackled, by the angle between
    /// the two players' directions (0 to 7).
    pub tackle_defence_penalty: Vec<i32>,
    /// The skill each piece of equipment (sprites 80 to 87) raises: 72
    /// aggression, then attack, defence, speed, throwing, power, stamina,
    /// 79 intelligence.
    pub equipment_skill: Vec<i32>,
    /// Where an attacker goes to support near the opponents' goal, by row
    /// then column of 32-pixel cells (left half, attacking up).
    pub support_positions: Vec<Vec<[i32; 2]>>,
    /// The same, for forwards.
    pub support_positions_forward: Vec<Vec<[i32; 2]>>,
    /// The status bar's position lights: `[x, sprite]`.
    pub status_leds: Vec<[i32; 2]>,
    /// Where a carried ball sits, by the carrier's sprite: `[x, y]`.
    pub ball_offset: Vec<[i32; 2]>,
    /// The offset from a player's position to its sprite's centre, by
    /// sprite: `[x, y]`.
    pub sprite_origin: Vec<[i32; 2]>,
    /// The ball's way through the score-multiplier ramps, a step a frame:
    /// left going up, left going down, right going down, right going up.
    /// `[0, 0]` ends it, and the ball shoots back out.
    pub multiplier_paths: Vec<Vec<[i32; 2]>>,
}

impl Lookups {
    pub fn read(rom: &Rom) -> Self {
        let bytes = |base: i32, count: usize, read: fn(&Rom, i32) -> i32| {
            (0..count as i32).map(|i| read(rom, base + i)).collect()
        };
        let pairs = |base: i32, count: usize| {
            (0..count as i32)
                .map(|i| [rom.i8(base + i * 2), rom.i8(base + i * 2 + 1)])
                .collect()
        };
        let words = |at: i32| [rom.u16(at), rom.u16(at + 2)];
        let grid = |base: i32| {
            (0..SUPPORT_ROWS as i32)
                .map(|row| {
                    (0..SUPPORT_COLUMNS as i32)
                        .map(|column| words(base + column * 4 + row * 36))
                        .collect()
                })
                .collect()
        };
        Self {
            velocity: (0..SPEEDS as i32)
                .map(|speed| {
                    (0..DIRECTIONS as i32)
                        .map(|dir| {
                            let at = addr::VELOCITY_BY_SPEED_DIR + 32 * speed + dir * 4;
                            [rom.i16(at), rom.i16(at + 2)]
                        })
                        .collect()
                })
                .collect(),
            direction_from_joystick: bytes(addr::DIR_FROM_DIR_BITS, 16, Rom::i8),
            mirror_x: bytes(addr::MIRROR_X_DIR, DIRECTIONS, Rom::u8),
            mirror_y: bytes(addr::MIRROR_Y_DIR, DIRECTIONS, Rom::u8),
            goalie_deflect: [0, 8]
                .map(|side| bytes(addr::GOALIE_DEFLECT_DIR + side, DIRECTIONS, Rom::u8)),
            think_delay: bytes(addr::THINK_DELAY_BY_INT, SKILL_STEPS, Rom::u8),
            prediction_shift: bytes(addr::PREDICTION_SHIFT_BY_INT, SKILL_STEPS, Rom::u8),
            jump_bonus: bytes(addr::JUMP_BONUS_BY_SPD, SKILL_STEPS, Rom::u8),
            tackle_defence_penalty: bytes(
                addr::TACKLE_DEF_MALUS_BY_ANGLE,
                DIRECTIONS,
                Rom::u8,
            ),
            equipment_skill: bytes(addr::EQUIPMENT_SKILL, EQUIPMENT, Rom::u8),
            support_positions: grid(addr::SUPPORT_POSITIONS),
            support_positions_forward: grid(addr::SUPPORT_POSITIONS_FORWARD),
            status_leds: (0..STATUS_LEDS as i32)
                .map(|slot| words(addr::STATUS_LEDS + slot * 4))
                .collect(),
            ball_offset: pairs(addr::BALL_OFFSET_BY_SPRITE, BALL_OFFSETS),
            sprite_origin: pairs(addr::SPRITE_ORIGIN, SPRITE_ORIGINS),
            multiplier_paths: [
                MultiplierPath::LeftUp,
                MultiplierPath::LeftDown,
                MultiplierPath::RightDown,
                MultiplierPath::RightUp,
            ]
            .map(|path| pairs(path.address(), PATH_STEPS))
            .to_vec(),
        }
    }

    /// Finds tables of the wrong size: a lookup past the end of an edited
    /// table would read the original's memory instead.
    pub fn check(&self) -> Result<(), String> {
        let mut problems = Vec::new();
        let mut expect = |name: &str, length: usize, wanted: usize| {
            if length != wanted {
                problems.push(format!("{name} must have {wanted} entries, not {length}"));
            }
        };
        expect("velocity", self.velocity.len(), SPEEDS);
        for speed in &self.velocity {
            expect("each speed in velocity", speed.len(), DIRECTIONS);
        }
        expect(
            "direction_from_joystick",
            self.direction_from_joystick.len(),
            16,
        );
        expect("mirror_x", self.mirror_x.len(), DIRECTIONS);
        expect("mirror_y", self.mirror_y.len(), DIRECTIONS);
        for side in &self.goalie_deflect {
            expect("each side of goalie_deflect", side.len(), DIRECTIONS);
        }
        expect("think_delay", self.think_delay.len(), SKILL_STEPS);
        expect("prediction_shift", self.prediction_shift.len(), SKILL_STEPS);
        expect("jump_bonus", self.jump_bonus.len(), SKILL_STEPS);
        expect(
            "tackle_defence_penalty",
            self.tackle_defence_penalty.len(),
            DIRECTIONS,
        );
        expect("equipment_skill", self.equipment_skill.len(), EQUIPMENT);
        for grid in [&self.support_positions, &self.support_positions_forward] {
            expect("support_positions", grid.len(), SUPPORT_ROWS);
            for row in grid {
                expect("each row of support_positions", row.len(), SUPPORT_COLUMNS);
            }
        }
        expect("status_leds", self.status_leds.len(), STATUS_LEDS);
        expect("ball_offset", self.ball_offset.len(), BALL_OFFSETS);
        expect("sprite_origin", self.sprite_origin.len(), SPRITE_ORIGINS);
        expect("multiplier_paths", self.multiplier_paths.len(), 4);
        for path in &self.multiplier_paths {
            expect("each of multiplier_paths", path.len(), PATH_STEPS);
        }
        crate::data::report(problems)
    }
}

/// `table[index]`, if there is such an entry.
pub(super) fn entry<T: Copy>(table: &[T], index: i32) -> Option<T> {
    usize::try_from(index)
        .ok()
        .and_then(|i| table.get(i))
        .copied()
}

/// `rows[row][column]` where the rows lie end to end in memory, so a
/// column off either end of a row reads the next or previous row.
pub(super) fn in_run<T: Copy>(rows: &[Vec<T>], width: usize, row: i32, column: i32) -> Option<T> {
    let flat = row * width as i32 + column;
    let row = usize::try_from(flat.div_euclid(width as i32)).ok()?;
    rows.get(row)?
        .get(flat.rem_euclid(width as i32) as usize)
        .copied()
}

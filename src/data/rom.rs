//! The Atari ST disk viewed as the game's RAM image, with typed accessors
//! for every lookup table the match engine uses.

use super::addr;
use super::bytes::BigEndian;
use crate::game::geometry::{DirBits, Point};

/// The ST disk image, addressed by original RAM address.
///
/// The game's data segment sits at a fixed disk offset, so a RAM address maps
/// to `ram - 0x400 + 0x4D600` on disk.
pub struct AtariRom {
    disk: Vec<u8>,
}

impl AtariRom {
    const RAM_BASE: i32 = 0x400;
    const DISK_BASE: i32 = 0x4D600;

    pub fn new(disk: Vec<u8>) -> Self {
        Self { disk }
    }

    /// The raw disk image (graphics live outside the data segment).
    pub fn disk(&self) -> &[u8] {
        &self.disk
    }

    pub fn ram_to_disk(ram: i32) -> usize {
        usize::try_from(ram - Self::RAM_BASE + Self::DISK_BASE)
            .expect("RAM address below the data segment")
    }

    pub fn u8(&self, ram: i32) -> i32 {
        i32::from(self.disk.u8_at(Self::ram_to_disk(ram)))
    }

    pub fn i8(&self, ram: i32) -> i32 {
        i32::from(self.disk.i8_at(Self::ram_to_disk(ram)))
    }

    pub fn u16(&self, ram: i32) -> i32 {
        i32::from(self.disk.u16_at(Self::ram_to_disk(ram)))
    }

    pub fn i16(&self, ram: i32) -> i32 {
        i32::from(self.disk.i16_at(Self::ram_to_disk(ram)))
    }

    /// A 32-bit value, used for pointers (all of which fit in an `i32`).
    pub fn u32(&self, ram: i32) -> i32 {
        self.disk.u32_at(Self::ram_to_disk(ram)) as i32
    }

    pub fn c_string(&self, ram: i32) -> String {
        self.disk.c_string_at(Self::ram_to_disk(ram))
    }

    fn signed_pair(&self, ram: i32) -> Point {
        Point::new(self.i8(ram), self.i8(ram + 1))
    }

    // -- Movement ------------------------------------------------------------

    /// Velocity vector for one of the 8 directions at a given speed (0..=8).
    pub fn velocity(&self, dir: i32, speed: i32) -> Point {
        let ram = addr::VELOCITY_BY_SPEED_DIR + 32 * speed + dir * 4;
        Point::new(self.i16(ram), self.i16(ram + 2))
    }

    /// Direction 0..=7 for a joystick bit set, or -1 for "no direction".
    pub fn dir_from_bits(&self, bits: DirBits) -> i32 {
        self.i8(addr::DIR_FROM_DIR_BITS + i32::from(bits.bits()))
    }

    pub fn mirror_dir_x(&self, dir: i32) -> i32 {
        self.u8(addr::MIRROR_X_DIR + dir)
    }

    pub fn mirror_dir_y(&self, dir: i32) -> i32 {
        self.u8(addr::MIRROR_Y_DIR + dir)
    }

    /// Direction the ball leaves in after a goalkeeper dive deflects it.
    pub fn goalie_deflect_dir(&self, dir: i32, initial_dir: i32) -> i32 {
        let side_offset = if initial_dir != 0 { 8 } else { 0 };
        self.u8(addr::GOALIE_DEFLECT_DIR + dir + side_offset)
    }

    // -- Animation -----------------------------------------------------------

    /// One word of an animation script: a sprite index or a control code.
    pub fn script_word(&self, script: i32, index: i32) -> i32 {
        self.u16(script + index * 2)
    }

    /// The script for `dir` from a per-direction animation list.
    pub fn script_for_dir(&self, list: i32, dir: i32) -> i32 {
        self.u32(list + dir * 4)
    }

    /// Offset from a player's position to its sprite's visual centre.
    pub fn sprite_origin(&self, sprite: i32) -> Point {
        self.signed_pair(addr::SPRITE_ORIGIN + sprite * 2)
    }

    /// Where a carried ball sits relative to the carrier, per carrier sprite.
    pub fn ball_offset(&self, carrier_sprite: i32) -> Point {
        self.signed_pair(addr::BALL_OFFSET_BY_SPRITE + carrier_sprite * 2)
    }

    pub fn multiplier_path_step(&self, path: i32, step: i32) -> Point {
        self.signed_pair(path + step * 2)
    }

    // -- Skill curves (indexed by `(skill - 100) / 10`) ----------------------

    pub fn think_delay(&self, int: i32) -> i32 {
        self.u8(addr::THINK_DELAY_BY_INT + (int - 100) / 10)
    }

    pub fn prediction_shift(&self, int: i32) -> i32 {
        self.u8(addr::PREDICTION_SHIFT_BY_INT + (int - 100) / 10)
    }

    pub fn jump_bonus(&self, spd: i32) -> i32 {
        self.u8(addr::JUMP_BONUS_BY_SPD + (spd - 100) / 10)
    }

    /// Defence penalty depending on the angle between tackler and victim.
    pub fn tackle_def_malus(&self, delta_dir: i32) -> i32 {
        self.u8(addr::TACKLE_DEF_MALUS_BY_ANGLE + delta_dir)
    }

    // -- Misc ----------------------------------------------------------------

    /// Raw skill selector for an equipment pickup (72..=79 maps to the 8 skills).
    pub fn equipment_skill(&self, equipment_index: i32) -> i32 {
        self.u8(addr::EQUIPMENT_SKILL + equipment_index)
    }

    /// A support position for off-ball AI from the tactics grid.
    pub fn support_position(&self, forward: bool, column: i32, row: i32) -> Point {
        let base = if forward {
            addr::SUPPORT_POSITIONS_FORWARD
        } else {
            addr::SUPPORT_POSITIONS
        };
        let ram = base + column * 4 + row * 36;
        Point::new(self.u16(ram), self.u16(ram + 2))
    }

    /// Status bar LED: `(screen x, sprite index)` for slot 0..6.
    pub fn status_led(&self, slot: i32) -> (i32, i32) {
        let ram = addr::STATUS_LEDS + slot * 4;
        (self.u16(ram), self.u16(ram + 2))
    }

    /// 10-character position name ("GOALKEEPER" etc.) for player slot 0..9.
    pub fn position_name(&self, player_index: usize) -> [u8; 10] {
        let base = addr::POSITION_NAMES + player_index as i32 * 10;
        std::array::from_fn(|i| self.u8(base + i as i32) as u8)
    }
}

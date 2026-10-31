//! The Atari ST disk viewed as the game's RAM image, with typed accessors
//! for every lookup table the match engine uses.

use super::addr;
use super::bytes::BigEndian;
use super::lookups::{entry, in_run, Lookups, DIRECTIONS, SUPPORT_COLUMNS};
use crate::game::geometry::{DirBits, Point};
use crate::game::roster::Teams;

/// The ST disk image, addressed by original RAM address, and the lookup
/// tables the match uses (read from it, or replaced by an asset pack's).
///
/// The game's data segment sits at a fixed disk offset, so a RAM address maps
/// to `ram - 0x400 + 0x4D600` on disk.
pub struct AtariRom {
    disk: Vec<u8>,
    lookups: Lookups,
    teams: Teams,
}

impl AtariRom {
    const RAM_BASE: i32 = 0x400;
    const DISK_BASE: i32 = 0x4D600;

    pub fn new(disk: Vec<u8>) -> Self {
        let mut rom = Self {
            disk,
            lookups: Lookups::default(),
            teams: Teams::default(),
        };
        rom.lookups = Lookups::read(&rom);
        rom.teams = Teams::read(&rom);
        rom
    }

    /// The league teams and player two's, in use.
    pub fn teams(&self) -> &Teams {
        &self.teams
    }

    /// Uses other teams (an asset pack's).
    pub fn set_teams(&mut self, teams: Teams) {
        self.teams = teams;
    }

    /// Uses other lookup tables (an asset pack's).
    pub fn set_lookups(&mut self, lookups: Lookups) {
        self.lookups = lookups;
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
        // The table is one run in memory: a direction off either end reads
        // the neighbouring speed's.
        match in_run(&self.lookups.velocity, DIRECTIONS, speed, dir) {
            Some([x, y]) => Point::new(x, y),
            None => {
                let ram = addr::VELOCITY_BY_SPEED_DIR + 32 * speed + dir * 4;
                Point::new(self.i16(ram), self.i16(ram + 2))
            }
        }
    }

    /// Direction 0..=7 for a joystick bit set, or -1 for "no direction".
    pub fn dir_from_bits(&self, bits: DirBits) -> i32 {
        let index = i32::from(bits.bits());
        entry(&self.lookups.direction_from_joystick, index)
            .unwrap_or_else(|| self.i8(addr::DIR_FROM_DIR_BITS + index))
    }

    pub fn mirror_dir_x(&self, dir: i32) -> i32 {
        entry(&self.lookups.mirror_x, dir).unwrap_or_else(|| self.u8(addr::MIRROR_X_DIR + dir))
    }

    pub fn mirror_dir_y(&self, dir: i32) -> i32 {
        entry(&self.lookups.mirror_y, dir).unwrap_or_else(|| self.u8(addr::MIRROR_Y_DIR + dir))
    }

    /// Direction the ball leaves in after a goalkeeper dive deflects it.
    pub fn goalie_deflect_dir(&self, dir: i32, initial_dir: i32) -> i32 {
        let side = usize::from(initial_dir != 0);
        entry(&self.lookups.goalie_deflect[side], dir)
            .unwrap_or_else(|| self.u8(addr::GOALIE_DEFLECT_DIR + dir + side as i32 * 8))
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
        entry(&self.lookups.sprite_origin, sprite)
            .map_or_else(|| self.signed_pair(addr::SPRITE_ORIGIN + sprite * 2), pair)
    }

    /// Where a carried ball sits relative to the carrier, per carrier sprite.
    pub fn ball_offset(&self, carrier_sprite: i32) -> Point {
        entry(&self.lookups.ball_offset, carrier_sprite).map_or_else(
            || self.signed_pair(addr::BALL_OFFSET_BY_SPRITE + carrier_sprite * 2),
            pair,
        )
    }

    pub fn multiplier_path_step(&self, path: i32, step: i32) -> Point {
        self.signed_pair(path + step * 2)
    }

    // -- Skill curves (indexed by `(skill - 100) / 10`) ----------------------

    fn skill_curve(&self, table: &[i32], base: i32, skill: i32) -> i32 {
        let index = (skill - 100) / 10;
        entry(table, index).unwrap_or_else(|| self.u8(base + index))
    }

    pub fn think_delay(&self, int: i32) -> i32 {
        self.skill_curve(&self.lookups.think_delay, addr::THINK_DELAY_BY_INT, int)
    }

    pub fn prediction_shift(&self, int: i32) -> i32 {
        let table = &self.lookups.prediction_shift;
        self.skill_curve(table, addr::PREDICTION_SHIFT_BY_INT, int)
    }

    pub fn jump_bonus(&self, spd: i32) -> i32 {
        self.skill_curve(&self.lookups.jump_bonus, addr::JUMP_BONUS_BY_SPD, spd)
    }

    /// Defence penalty depending on the angle between tackler and victim.
    pub fn tackle_def_malus(&self, delta_dir: i32) -> i32 {
        entry(&self.lookups.tackle_defence_penalty, delta_dir)
            .unwrap_or_else(|| self.u8(addr::TACKLE_DEF_MALUS_BY_ANGLE + delta_dir))
    }

    // -- Misc ----------------------------------------------------------------

    /// Raw skill selector for an equipment pickup (72..=79 maps to the 8 skills).
    pub fn equipment_skill(&self, equipment_index: i32) -> i32 {
        entry(&self.lookups.equipment_skill, equipment_index)
            .unwrap_or_else(|| self.u8(addr::EQUIPMENT_SKILL + equipment_index))
    }

    /// A support position for off-ball AI from the tactics grid.
    pub fn support_position(&self, forward: bool, column: i32, row: i32) -> Point {
        let (grid, base) = if forward {
            (
                &self.lookups.support_positions_forward,
                addr::SUPPORT_POSITIONS_FORWARD,
            )
        } else {
            (&self.lookups.support_positions, addr::SUPPORT_POSITIONS)
        };
        match in_run(grid, SUPPORT_COLUMNS, row, column) {
            Some([x, y]) => Point::new(x, y),
            None => {
                let ram = base + column * 4 + row * 36;
                Point::new(self.u16(ram), self.u16(ram + 2))
            }
        }
    }

    /// Status bar LED: `(screen x, sprite index)` for slot 0..6.
    pub fn status_led(&self, slot: i32) -> (i32, i32) {
        match entry(&self.lookups.status_leds, slot) {
            Some([x, sprite]) => (x, sprite),
            None => {
                let ram = addr::STATUS_LEDS + slot * 4;
                (self.u16(ram), self.u16(ram + 2))
            }
        }
    }

    /// 10-character position name ("GOALKEEPER" etc.) for player slot 0..9.
    pub fn position_name(&self, player_index: usize) -> [u8; 10] {
        let base = addr::POSITION_NAMES + player_index as i32 * 10;
        std::array::from_fn(|i| self.u8(base + i as i32) as u8)
    }
}

fn pair([x, y]: [i32; 2]) -> Point {
    Point::new(x, y)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, Platform};
    use crate::data::GameFiles;

    /// Every lookup, in its table or past either end, reads what the
    /// original memory holds there.
    #[test]
    fn lookups_read_what_the_memory_holds() {
        let config = Config::default();
        let Ok(files) = GameFiles::load(&config.data_dir, Platform::Atari) else {
            eprintln!("skipping: no originals");
            return;
        };
        let rom = AtariRom::new(files.atari_disk);
        let raw_pair = |ram: i32| Point::new(rom.i8(ram), rom.i8(ram + 1));
        for dir in -10..20 {
            for speed in -2..12 {
                let ram = addr::VELOCITY_BY_SPEED_DIR + 32 * speed + dir * 4;
                let raw = Point::new(rom.i16(ram), rom.i16(ram + 2));
                assert_eq!(rom.velocity(dir, speed), raw, "velocity {dir} {speed}");
            }
            assert_eq!(rom.mirror_dir_x(dir), rom.u8(addr::MIRROR_X_DIR + dir));
            assert_eq!(rom.mirror_dir_y(dir), rom.u8(addr::MIRROR_Y_DIR + dir));
            assert_eq!(
                rom.tackle_def_malus(dir),
                rom.u8(addr::TACKLE_DEF_MALUS_BY_ANGLE + dir)
            );
            assert_eq!(
                rom.equipment_skill(dir),
                rom.u8(addr::EQUIPMENT_SKILL + dir)
            );
            for initial_dir in [0, 4] {
                let side = if initial_dir != 0 { 8 } else { 0 };
                assert_eq!(
                    rom.goalie_deflect_dir(dir, initial_dir),
                    rom.u8(addr::GOALIE_DEFLECT_DIR + dir + side)
                );
            }
            let led = addr::STATUS_LEDS + dir * 4;
            assert_eq!(rom.status_led(dir), (rom.u16(led), rom.u16(led + 2)));
        }
        for bits in 0..16 {
            let bits = DirBits::from_bits(bits);
            let raw = rom.i8(addr::DIR_FROM_DIR_BITS + i32::from(bits.bits()));
            assert_eq!(rom.dir_from_bits(bits), raw);
        }
        for skill in 0..=255 {
            let index = (skill - 100) / 10;
            assert_eq!(
                rom.think_delay(skill),
                rom.u8(addr::THINK_DELAY_BY_INT + index)
            );
            assert_eq!(
                rom.prediction_shift(skill),
                rom.u8(addr::PREDICTION_SHIFT_BY_INT + index)
            );
            assert_eq!(
                rom.jump_bonus(skill),
                rom.u8(addr::JUMP_BONUS_BY_SPD + index)
            );
        }
        for sprite in -5..140 {
            assert_eq!(
                rom.ball_offset(sprite),
                raw_pair(addr::BALL_OFFSET_BY_SPRITE + sprite * 2)
            );
            assert_eq!(
                rom.sprite_origin(sprite),
                raw_pair(addr::SPRITE_ORIGIN + sprite * 2)
            );
        }
        for forward in [false, true] {
            let base = if forward {
                addr::SUPPORT_POSITIONS_FORWARD
            } else {
                addr::SUPPORT_POSITIONS
            };
            for row in -2..8 {
                for column in -2..11 {
                    let ram = base + column * 4 + row * 36;
                    let raw = Point::new(rom.u16(ram), rom.u16(ram + 2));
                    assert_eq!(rom.support_position(forward, column, row), raw);
                }
            }
        }
    }
}

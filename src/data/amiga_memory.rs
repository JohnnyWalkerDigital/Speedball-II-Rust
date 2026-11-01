//! The match's data as the Amiga holds it before any match is played
//! (`Intro - Amiga.dmp`), read at the ST's addresses ([`super::addr`]),
//! which is how the port names it.
//!
//! The data is the ST's, in blocks the Amiga keeps elsewhere. Its records
//! point to its own code, graphics and scripts, and its sprite banks
//! number some sprites differently: [`AmigaMemory`] gives these as the
//! port has them. Where the Amiga's values really differ, they are the
//! Amiga's; the tests list each one.

use std::collections::HashMap;
use std::ops::Range;

use super::addr;
use crate::game::entity::EntityId;
use crate::game::geometry::Point;
use crate::game::sprites::{AmigaSprite, Bank};

/// The blocks of the ST's addresses the Amiga has, and how far on it has
/// them.
const BLOCKS: [(Range<i32>, i32); 9] = [
    // The joystick directions.
    (0x04B0..0x04D2, -0x33E),
    // The skill curves and tackling.
    (0x04D2..0x0673, -0x338),
    // The status lights and the ball's offsets.
    (0x247C..SPRITE_ORIGINS.start, 0x1556),
    // The ramp paths, the support grid and the equipment's skills.
    (SPRITE_ORIGINS.end..0x3F99, 0x1646),
    // The attributes and the squads.
    (0x3FCE..0x4700, 0x1794),
    // The ball's records to the equipment's.
    (addr::BALL..GOAL_INDICATORS.start, 0x17BC),
    // The player indicators' records to the scripts before the domes'.
    (GOAL_INDICATORS.end..DOME_SCRIPT, 0x176C),
    // The domes' script, the animation lists, the mirrors, the
    // velocities, the teams and their names.
    (DOME_SCRIPT..0x682E, 0x177C),
    // The goalkeepers' deflections.
    (0xD5FA..0xD939, 0x16E0),
];

/// The sprite origins, which the Amiga has as word pairs.
const SPRITE_ORIGINS: Range<i32> = addr::SPRITE_ORIGIN..addr::SPRITE_ORIGIN + 240;
const AMIGA_SPRITE_ORIGINS: i32 = 0x40E2;

/// The Amiga has no goal indicators: these are the ST's records.
const GOAL_INDICATORS: Range<i32> = addr::GOAL_INDICATORS[0]..addr::GOAL_INDICATORS[1] + 40;
const GOAL_INDICATOR_RECORDS: [u8; 80] = [
    0x00, 0x01, 0x93, 0xEA, 0x00, 0x01, 0x30, 0x5C, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x10, 0x00, 0x02, 0x01, 0x10, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x22, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, //
    0x00, 0x01, 0x93, 0xEA, 0x00, 0x01, 0x30, 0x5C, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x10, 0x00, 0x02, 0x01, 0x70, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x22, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
];

/// The scripts, and the banks they draw from, from each address on.
const SCRIPTS: Range<i32> = addr::SCRIPT_BALL_THROWN_HIGH..addr::ANIMATIONS;
const SCRIPT_BANKS: [(i32, Bank); 6] = [
    (addr::SCRIPT_BALL_THROWN_HIGH, Bank::Entities),
    (0x5660, Bank::Launcher),
    (0x56B0, Bank::BigBall),
    (0x56C8, Bank::Entities),
    (0x56FA, Bank::Players),
    (DOME_SCRIPT, Bank::Launcher),
];
const DOME_SCRIPT: i32 = 0x5E1E;
/// Script words from here on are controls, not sprites.
const FIRST_CONTROL: u16 = 0xFF00;

/// The Amiga's own scripts for the electrobounces. The port lights them
/// itself, so they have none.
const ELECTROBOUNCE_SCRIPTS: Range<u32> = 0x758A..0x759A;

/// Where a record's sprite is, from the start of the record.
const SPRITE_FIELD: i32 = 32;
const GFX_FIELD: i32 = 0;

/// The Amiga's graphics banks, as its records point to them, and the
/// port's pointers for them. The port draws the launcher bank's sprites
/// as the players'.
const BANKS: [(u32, Bank, u32); 4] = [
    (0x3A2D6, Bank::Entities, addr::GFX_ENTITIES),
    (0x3CF56, Bank::Players, addr::GFX_PLAYERS),
    (0x4D356, Bank::BigBall, addr::GFX_BIG_BALL),
    (0x4DD56, Bank::Launcher, addr::GFX_PLAYERS),
];

/// The Amiga code the records point to, and the ST's.
const THINKS: [(u32, u32); 4] = [
    (0x10A8A, 0xF344),
    (0x113D4, 0xFC3C),
    (0x11436, 0xFC9C),
    (0x11558, 0xFD92),
];

/// The Amiga's memory before a match.
pub struct AmigaMemory {
    dump: Vec<u8>,
    /// The bank each record's sprite is in, by where the sprite is.
    sprite_banks: HashMap<i32, Bank>,
}

impl AmigaMemory {
    pub fn new(dump: &[u8]) -> Self {
        let mut memory = Self {
            dump: dump.to_vec(),
            sprite_banks: HashMap::new(),
        };
        memory.sprite_banks = EntityId::records()
            .into_iter()
            .filter_map(|(_, start)| {
                let bank = bank(memory.plain_long(start + GFX_FIELD))?;
                Some((start + SPRITE_FIELD, bank))
            })
            .collect();
        memory
    }

    /// Where the Amiga has what the ST has at `st`.
    fn amiga_address(st: i32) -> Option<i32> {
        BLOCKS
            .iter()
            .find(|(block, _)| block.contains(&st))
            .map(|(_, shift)| st + shift)
    }

    /// The ST's address for what the Amiga has at `amiga`.
    fn st_address(amiga: u32) -> Option<u32> {
        let amiga = i32::try_from(amiga).ok()?;
        BLOCKS
            .iter()
            .map(|(block, shift)| (block, amiga - shift))
            .find(|(block, st)| block.contains(st))
            .map(|(_, st)| st as u32)
    }

    fn raw_byte(&self, amiga: i32) -> u8 {
        usize::try_from(amiga)
            .ok()
            .and_then(|at| self.dump.get(at))
            .copied()
            .unwrap_or(0)
    }

    fn raw_word(&self, amiga: i32) -> u16 {
        u16::from_be_bytes([self.raw_byte(amiga), self.raw_byte(amiga + 1)])
    }

    /// The byte at `st`, or 0 where the Amiga has nothing for it.
    pub fn byte(&self, st: i32) -> u8 {
        if GOAL_INDICATORS.contains(&st) {
            return GOAL_INDICATOR_RECORDS[(st - GOAL_INDICATORS.start) as usize];
        }
        if SPRITE_ORIGINS.contains(&st) {
            let word = AMIGA_SPRITE_ORIGINS + (st - SPRITE_ORIGINS.start) * 2;
            return self.raw_byte(word + 1);
        }
        Self::amiga_address(st).map_or(0, |amiga| self.raw_byte(amiga))
    }

    fn plain_word(&self, st: i32) -> u16 {
        u16::from_be_bytes([self.byte(st), self.byte(st + 1)])
    }

    fn plain_long(&self, st: i32) -> u32 {
        (u32::from(self.plain_word(st)) << 16) | u32::from(self.plain_word(st + 2))
    }

    /// The word at `st`, with a record's or a script's sprite numbered as
    /// the port numbers it.
    pub fn word(&self, st: i32) -> u16 {
        let word = self.plain_word(st);
        let bank = match self.sprite_banks.get(&st) {
            Some(&bank) => bank,
            None if SCRIPTS.contains(&st) && word < FIRST_CONTROL => {
                SCRIPT_BANKS
                    .iter()
                    .rev()
                    .find(|&&(start, _)| start <= st)
                    .expect("the scripts start with a bank")
                    .1
            }
            None => return word,
        };
        let sprite = AmigaSprite {
            bank,
            index: i32::from(word),
        };
        sprite.port_index() as u16
    }

    /// The long at `st`: always a pointer, given as the port has it.
    pub fn long(&self, st: i32) -> u32 {
        let pointer = self.plain_long(st);
        if GOAL_INDICATORS.contains(&st) || pointer == 0 {
            return pointer;
        }
        if let Some(&(_, _, port)) = BANKS.iter().find(|&&(amiga, ..)| amiga == pointer) {
            return port;
        }
        if let Some(&(_, st)) = THINKS.iter().find(|&&(amiga, _)| amiga == pointer) {
            return st;
        }
        if ELECTROBOUNCE_SCRIPTS.contains(&pointer) {
            return 0;
        }
        Self::st_address(pointer).unwrap_or(pointer)
    }

    /// A sprite's origin, from the table of word pairs: past its end,
    /// whatever follows it.
    pub fn sprite_origin(&self, sprite: i32) -> Point {
        let at = AMIGA_SPRITE_ORIGINS + sprite * 4;
        let word = |at| i32::from(self.raw_word(at) as i16);
        Point::new(word(at), word(at + 2))
    }
}

/// The bank an Amiga record's graphics pointer is to.
fn bank(pointer: u32) -> Option<Bank> {
    BANKS
        .iter()
        .find(|&&(amiga, ..)| amiga == pointer)
        .map(|&(_, bank, _)| bank)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::data::animations::AnimationTable;
    use crate::data::files::{read_amiga_intro_dump, read_atari_disk};
    use crate::data::lookups::Lookups;
    use crate::data::Rom;
    use crate::game::roster::Teams;
    use serde::Serialize;
    use serde_json::Value;

    /// The match's data read from the ST's disk and from the Amiga's
    /// memory.
    fn both() -> Option<(Rom, Rom)> {
        let dir = Config::default().data_dir;
        let disk = read_atari_disk(&dir).ok()?;
        let dump = read_amiga_intro_dump(&dir).ok()?;
        Some((Rom::atari(disk), Rom::new(&dump)))
    }

    /// Where two values differ, each as `path: ST value, Amiga value`.
    fn differences(st: &impl Serialize, amiga: &impl Serialize) -> Vec<String> {
        fn walk(path: &str, st: &Value, amiga: &Value, out: &mut Vec<String>) {
            match (st, amiga) {
                (Value::Object(st), Value::Object(amiga)) => {
                    for (key, value) in st {
                        walk(&format!("{path}.{key}"), value, &amiga[key], out);
                    }
                }
                (Value::Array(st), Value::Array(amiga)) if st.len() == amiga.len() => {
                    for (i, (st, amiga)) in st.iter().zip(amiga).enumerate() {
                        // A record or a script by its name.
                        let name = st.get("id").or_else(|| st.get("names").map(|n| &n[0]));
                        let step = name.and_then(Value::as_str).map_or(i.to_string(), str::to_string);
                        walk(&format!("{path}.{step}"), st, amiga, out);
                    }
                }
                _ if st != amiga => out.push(format!("{path}: {st}, {amiga}")),
                _ => {}
            }
        }
        let mut out = Vec::new();
        let (st, amiga) = (serde_json::to_value(st), serde_json::to_value(amiga));
        walk("", &st.unwrap(), &amiga.unwrap(), &mut out);
        out
    }

    #[test]
    fn the_tables_and_teams_are_the_sts() {
        let Some((st, amiga)) = both() else {
            eprintln!("skipping: no originals");
            return;
        };
        assert_eq!(Lookups::read(&st), Lookups::read(&amiga));
        assert_eq!(Teams::read(&st), Teams::read(&amiga));
    }

    #[test]
    fn the_scripts_are_the_sts_but_for_the_domes() {
        let Some((st, amiga)) = both() else {
            return;
        };
        let differences = differences(&AnimationTable::read(&st), &AnimationTable::read(&amiga));
        // The Amiga draws the domes as part of the pitch, and as sprites
        // only when hit: its domes show the hit frame, then hide.
        let expected = [
            ".scripts.bounce_dome_0.frames.2: 135, 136",
            ".scripts.bounce_dome_0.frames.3: \"hold\", \"hide\"",
        ];
        assert_eq!(differences, expected);
    }

    #[test]
    fn the_starting_records_are_the_sts_but_for_the_listed_differences() {
        let Some((st, amiga)) = both() else {
            return;
        };
        let table = |rom: &Rom| rom.start().table(rom.animations());
        let differences = differences(&table(&st), &table(&amiga));
        let expected = [
            // The big ball's first sprite, before its script sets one.
            ".entities.big_ball.sprite: 137, 3",
            // The player indicators are taller, and drawn 2 lower.
            ".entities.player_indicator_0.origin.1: -24, -22",
            ".entities.player_indicator_0.size.1: 10, 16",
            ".entities.player_indicator_1.origin.1: -24, -22",
            ".entities.player_indicator_1.size.1: 10, 16",
            // The right-hand ramps' lights are a pixel further right.
            ".entities.score_multiplier_2.pos.0: 631, 632",
            ".entities.score_multiplier_3.pos.0: 631, 632",
            // The domes start on their hit frame.
            ".entities.bounce_dome_0.sprite: 135, 136",
            ".entities.bounce_dome_1.sprite: 135, 136",
            // The electrobounces are 32 by 32, drawn 4 pixels outwards.
            ".entities.electrobounce_0.gfx: 0, \"players\"",
            ".entities.electrobounce_0.origin.0: 0, -4",
            ".entities.electrobounce_0.size.0: 0, 32",
            ".entities.electrobounce_0.size.1: 0, 32",
            ".entities.electrobounce_1.gfx: 0, \"players\"",
            ".entities.electrobounce_1.origin.0: 0, 4",
            ".entities.electrobounce_1.size.0: 0, 32",
            ".entities.electrobounce_1.size.1: 0, 32",
        ];
        assert_eq!(differences, expected);
    }

    #[test]
    fn sprite_origins_past_the_table_read_the_memory_after_it() {
        let Ok(dump) = read_amiga_intro_dump(&Config::default().data_dir) else {
            return;
        };
        let amiga = Rom::new(&dump);
        for sprite in [-2, 120, 130] {
            let at = (AMIGA_SPRITE_ORIGINS + sprite * 4) as usize;
            let word = |at: usize| i32::from(i16::from_be_bytes([dump[at], dump[at + 1]]));
            assert_eq!(amiga.sprite_origin(sprite), Point::new(word(at), word(at + 2)));
        }
    }
}

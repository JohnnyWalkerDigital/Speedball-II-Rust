//! Where the match's data is in the Amiga's memory before any match is
//! played (`Intro - Amiga.dmp`), against where it is on the ST disk
//! ([`super::addr`]).
//!
//! The data is the ST's, moved: most of it by a fixed amount per block.
//! The Amiga's records point to its own code, graphics and scripts, and
//! number some sprites differently. A few values really differ; they are
//! listed in [`AMIGA_DIFFERENCES`].

use super::addr;

/// Amiga addresses of the ST's tables, by ST address.
pub const TABLES: &[(i32, usize)] = &[
    (addr::DIR_FROM_DIR_BITS, 0x017E),
    (addr::THINK_DELAY_BY_INT, 0x01FA),
    (addr::PREDICTION_SHIFT_BY_INT, 0x020A),
    (addr::JUMP_BONUS_BY_SPD, 0x022A),
    (addr::TACKLE_DEF_MALUS_BY_ANGLE, 0x024A),
    (addr::STATUS_LEDS, 0x3FE0),
    (addr::BALL_OFFSET_BY_SPRITE, 0x3FF8),
    (addr::MULTIPLIER_PATH_LEFT_UP, 0x42C2),
    (addr::SUPPORT_POSITIONS, 0x4452),
    (addr::SUPPORT_POSITIONS_FORWARD, 0x452A),
    (addr::EQUIPMENT_SKILL, 0x460B),
    (addr::MIRROR_X_DIR, 0x7802),
    (addr::MIRROR_Y_DIR, 0x780A),
    (addr::VELOCITY_BY_SPEED_DIR, 0x7812),
    (addr::GOALIE_DEFLECT_DIR, 0xEDF4),
    (addr::ATTRIBUTES_TEAM1, 0x583A),
    (addr::ATTRIBUTES_TEAM2, 0x58BE),
    (addr::ATTRIBUTES_TEMP, 0x5942),
    (addr::ATTRIBUTES_MARKET, 0x5B6C),
    (addr::PERSONS_TEAM1, 0x59AE),
    (addr::PERSONS_TEAM2, 0x5A94),
    (addr::PERSON_TEMP, 0x5A56),
    (addr::TEAM_PLAYER2, 0x7B10),
    (addr::TEAMS, 0x7B50),
];

/// The sprite origins: the ST's signed byte pairs as signed word pairs.
pub const SPRITE_ORIGIN: usize = 0x40E2;

/// The attribute tables, and the pointers to them, moved by this much.
pub const ATTRIBUTES_SHIFT: i32 = 0x1794;

/// The animation scripts and their lists. The Amiga has two more scripts
/// (the electrobounces') just before the domes' script, so scripts from
/// there on are 16 bytes further on.
pub const SCRIPTS_SHIFT: i32 = 0x176C;
pub const SCRIPTS_AFTER_ELECTROBOUNCES: i32 = 0x5E1E;
pub const SCRIPTS_AFTER_SHIFT: i32 = 0x177C;
pub const ELECTROBOUNCE_SCRIPTS: [usize; 2] = [0x758A, 0x7592];

/// Where the Amiga's scripts really differ from the ST's: the ST address
/// of the word, and the ST's and the Amiga's words. The domes' script
/// ends by hiding the dome rather than holding its frame, as the Amiga
/// draws the domes as part of the pitch but for the frame one is hit.
pub const SCRIPT_DIFFERENCES: &[(i32, u16, u16)] = &[(0x5E24, 0xFFFE, 0xFFFC)];

/// Where a script the ST has at `st` is on the Amiga.
pub fn script(st: i32) -> i32 {
    match st {
        0 => 0,
        st if st < SCRIPTS_AFTER_ELECTROBOUNCES => st + SCRIPTS_SHIFT,
        st => st + SCRIPTS_AFTER_SHIFT,
    }
}

/// The Amiga's graphics banks, as entity records point to them.
pub mod gfx {
    pub const ENTITIES: u32 = 0x3A2D6;
    pub const PLAYERS: u32 = 0x3CF56;
    pub const BIG_BALL: u32 = 0x4D356;
    /// The launcher, the domes and the electrobounces.
    pub const LAUNCHER: u32 = 0x4DD56;
}

/// The Amiga code entity records point to, by the ST's.
pub const THINKS: &[(u32, u32)] = &[
    (0, 0),
    // A player.
    (0xF344, 0x10A8A),
    (0xFC3C, 0x113D4),
    (0xFC9C, 0x11436),
    (0xFD92, 0x11558),
];

/// The entity records: ST address, Amiga address, Amiga graphics bank. The
/// Amiga has no goal-indicator records, so it has none for them.
pub const ENTITIES: &[(&str, i32, usize, u32)] = &[
    ("ball", addr::BALL, 0x5FCE, gfx::ENTITIES),
    ("launcher", addr::LAUNCHER, 0x5FF6, gfx::LAUNCHER),
    ("big ball", addr::BIG_BALL, 0x601E, gfx::BIG_BALL),
    ("coin 0", addr::COINS[0], 0x6046, gfx::ENTITIES),
    ("coin 1", addr::COINS[1], 0x606E, gfx::ENTITIES),
    ("coin 2", addr::COINS[2], 0x6096, gfx::ENTITIES),
    ("coin 3", addr::COINS[3], 0x60BE, gfx::ENTITIES),
    ("token 0", addr::TOKENS[0], 0x60E6, gfx::ENTITIES),
    ("token 1", addr::TOKENS[1], 0x610E, gfx::ENTITIES),
    ("equipment", addr::EQUIPMENT, 0x6136, gfx::ENTITIES),
    (
        "player indicator 0",
        addr::PLAYER_INDICATORS[0],
        0x615E,
        gfx::ENTITIES,
    ),
    (
        "player indicator 1",
        addr::PLAYER_INDICATORS[1],
        0x6186,
        gfx::ENTITIES,
    ),
    ("status light", addr::STATUS_LED, 0x61AE, gfx::ENTITIES),
    (
        "multiplier 0",
        addr::SCORE_MULTIPLIERS[0],
        0x61D6,
        gfx::ENTITIES,
    ),
    (
        "multiplier 1",
        addr::SCORE_MULTIPLIERS[1],
        0x61FE,
        gfx::ENTITIES,
    ),
    (
        "multiplier 2",
        addr::SCORE_MULTIPLIERS[2],
        0x6226,
        gfx::ENTITIES,
    ),
    (
        "multiplier 3",
        addr::SCORE_MULTIPLIERS[3],
        0x624E,
        gfx::ENTITIES,
    ),
    ("dome 0", addr::BOUNCE_DOMES[0], 0x6276, gfx::LAUNCHER),
    ("dome 1", addr::BOUNCE_DOMES[1], 0x62A0, gfx::LAUNCHER),
    (
        "electrobounce 0",
        addr::ELECTROBOUNCES[0],
        0x62CA,
        gfx::LAUNCHER,
    ),
    (
        "electrobounce 1",
        addr::ELECTROBOUNCES[1],
        0x62F4,
        gfx::LAUNCHER,
    ),
    ("medic 0", addr::MEDICS[0], 0x6B22, gfx::PLAYERS),
    ("medic 1", addr::MEDICS[1], 0x6B4A, gfx::PLAYERS),
    ("terrain centre", addr::TERRAIN_CENTER, 0x6B72, 0),
];

/// The players' records, team 1's then team 2's, moved by this much.
pub const PLAYERS_SHIFT: i32 = 0x176C;

/// Where the Amiga's starting records really differ from the ST's: the
/// entity, the field's offset in its record, and the ST's and the Amiga's
/// bytes. Sprite numbers here are in each platform's own numbering.
pub const AMIGA_DIFFERENCES: &[(&str, usize, &[u8], &[u8])] = &[
    ("launcher", SPRITE, &[0x00, 0x89], &[0x00, 0x03]),
    ("launcher", PAD, &[0x00], &[0x01]),
    ("big ball", SPRITE, &[0x00, 0x89], &[0x00, 0x03]),
    ("equipment", SPRITE, &[0x00, 0x50], &[0x00, 0x4F]),
    ("player indicator 0", HEIGHT, &[0x00, 0x0A], &[0x00, 0x10]),
    ("player indicator 0", ORIGIN_Y, &[0xFF, 0xE8], &[0xFF, 0xEA]),
    ("player indicator 1", HEIGHT, &[0x00, 0x0A], &[0x00, 0x10]),
    ("player indicator 1", ORIGIN_Y, &[0xFF, 0xE8], &[0xFF, 0xEA]),
    ("multiplier 2", X, &[0x02, 0x77], &[0x02, 0x78]),
    ("multiplier 3", X, &[0x02, 0x77], &[0x02, 0x78]),
    ("dome 0", SPRITE, &[0x00, 0x87], &[0x00, 0x02]),
    ("dome 0", PAD, &[0x00], &[0x01]),
    ("dome 1", SPRITE, &[0x00, 0x87], &[0x00, 0x02]),
    ("dome 1", PAD, &[0x00], &[0x01]),
    // The ST's electrobounces have empty records; the Amiga's are 32 by
    // 32, with scripts of their own.
    (
        "electrobounce 0",
        SCRIPT,
        &[0, 0, 0, 0],
        &[0x00, 0x00, 0x75, 0x8A],
    ),
    ("electrobounce 0", WIDTH, &[0x00, 0x00], &[0x00, 0x20]),
    ("electrobounce 0", HEIGHT, &[0x00, 0x00], &[0x00, 0x20]),
    ("electrobounce 0", PAD, &[0x00], &[0x01]),
    ("electrobounce 0", ORIGIN_X, &[0x00, 0x00], &[0xFF, 0xFC]),
    (
        "electrobounce 1",
        SCRIPT,
        &[0, 0, 0, 0],
        &[0x00, 0x00, 0x75, 0x92],
    ),
    ("electrobounce 1", WIDTH, &[0x00, 0x00], &[0x00, 0x20]),
    ("electrobounce 1", HEIGHT, &[0x00, 0x00], &[0x00, 0x20]),
    ("electrobounce 1", PAD, &[0x00], &[0x01]),
    ("electrobounce 1", ORIGIN_X, &[0x00, 0x00], &[0x00, 0x04]),
];

// An entity record's fields, by offset.
pub const GFX: usize = 0;
pub const THINK: usize = 8;
pub const SCRIPT: usize = 12;
pub const WIDTH: usize = 16;
pub const HEIGHT: usize = 18;
pub const X: usize = 20;
pub const SPRITE: usize = 32;
/// The byte after the flags, unused on the ST.
pub const PAD: usize = 35;
pub const ORIGIN_X: usize = 36;
pub const ORIGIN_Y: usize = 38;
pub const ENTITY_SIZE: usize = 40;
/// A player's record continues to here; it points to the player's
/// attributes at `PLAYER_ATTRIBUTES`.
pub const PLAYER_SIZE: usize = 0x72;
pub const PLAYER_ATTRIBUTES: usize = 92;

/// The ST's dome frame, which is the Amiga's third launcher-bank sprite.
const ST_DOME_FRAME: u16 = 0x87;

/// An ST sprite number as the Amiga numbers it in the animation scripts:
/// the entity bank leaves out a token, and the launcher bank counts from
/// its own start (with the dome's frame third).
pub fn script_sprite(st: u16) -> [u16; 2] {
    let launcher = match st {
        ST_DOME_FRAME => 2,
        st if st > ST_DOME_FRAME => st - 134,
        st => st,
    };
    [if (68..90).contains(&st) { st - 1 } else { st }, launcher]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, Platform};
    use crate::data::bytes::BigEndian;
    use crate::data::files::read_amiga_intro_dump;
    use crate::data::{AtariRom, GameFiles};

    /// The ST's memory and the Amiga's before a match.
    fn memories() -> Option<(AtariRom, Vec<u8>)> {
        let config = Config::default();
        let files = GameFiles::load(&config.data_dir, Platform::Atari).ok()?;
        let amiga = read_amiga_intro_dump(&config.data_dir).ok()?;
        Some((AtariRom::new(files.atari_disk), amiga))
    }

    fn st_bytes(rom: &AtariRom, at: i32, len: usize) -> Vec<u8> {
        (0..len as i32).map(|i| rom.u8(at + i) as u8).collect()
    }

    fn amiga_table(st: i32) -> usize {
        TABLES
            .iter()
            .find(|&&(s, _)| s == st)
            .map(|&(_, a)| a)
            .unwrap_or_else(|| panic!("no Amiga address for {st:#X}"))
    }

    /// The bytes of a text a record points to: its cursor, then up to NUL.
    fn text(bytes: impl Fn(usize) -> u8, at: usize) -> Vec<u8> {
        let mut out = vec![bytes(at), bytes(at + 1)];
        out.extend((at + 2..).map(&bytes).take_while(|&b| b != 0));
        out
    }

    #[test]
    fn tables_are_the_sts() {
        let Some((rom, amiga)) = memories() else {
            eprintln!("skipping: no originals");
            return;
        };
        let lengths = [
            (addr::DIR_FROM_DIR_BITS, 16),
            (addr::THINK_DELAY_BY_INT, 16),
            (addr::PREDICTION_SHIFT_BY_INT, 16),
            (addr::JUMP_BONUS_BY_SPD, 16),
            (addr::TACKLE_DEF_MALUS_BY_ANGLE, 8),
            (addr::STATUS_LEDS, 24),
            (addr::BALL_OFFSET_BY_SPRITE, 234),
            (addr::MULTIPLIER_PATH_LEFT_UP, 400),
            (addr::SUPPORT_POSITIONS, 216),
            (addr::SUPPORT_POSITIONS_FORWARD, 216),
            (addr::EQUIPMENT_SKILL, 8),
            (addr::MIRROR_X_DIR, 8),
            (addr::MIRROR_Y_DIR, 8),
            (addr::VELOCITY_BY_SPEED_DIR, 288),
            (addr::GOALIE_DEFLECT_DIR, 16),
            (addr::ATTRIBUTES_TEAM1, 132),
            (addr::ATTRIBUTES_TEAM2, 132),
            (addr::ATTRIBUTES_TEMP, 11),
            (addr::ATTRIBUTES_MARKET, 198),
        ];
        for (st, len) in lengths {
            let at = amiga_table(st);
            assert_eq!(
                st_bytes(&rom, st, len),
                amiga[at..at + len],
                "table at ST {st:#X}"
            );
        }
        for i in 0..120 {
            for (k, offset) in [0, 1].into_iter().enumerate() {
                let st = rom.i8(addr::SPRITE_ORIGIN + i * 2 + offset);
                let at = SPRITE_ORIGIN + (i as usize * 2 + k) * 2;
                assert_eq!(st, i32::from(amiga.i16_at(at)), "sprite origin {i}");
            }
        }
    }

    #[test]
    fn teams_and_squads_are_the_sts() {
        let Some((rom, amiga)) = memories() else {
            return;
        };
        let teams =
            std::iter::once(addr::TEAM_PLAYER2).chain((0..16).map(|i| addr::TEAMS + i * 0x40));
        for st in teams {
            let at = if st == addr::TEAM_PLAYER2 {
                amiga_table(st)
            } else {
                amiga_table(addr::TEAMS) + (st - addr::TEAMS) as usize
            };
            // The skill templates.
            assert_eq!(st_bytes(&rom, st + 24, 32), amiga[at + 24..at + 56]);
            // The full and short names.
            for name in [0, 4] {
                let st_text = text(|i| rom.u8(i as i32) as u8, rom.u32(st + name) as usize);
                let amiga_text = text(|i| amiga[i], amiga.u32_at(at + name as usize) as usize);
                assert_eq!(st_text, amiga_text, "team at ST {st:#X}");
            }
        }
        let people = [
            (addr::PERSONS_TEAM1, 12),
            (addr::PERSONS_TEAM2, 12),
            (addr::PERSON_TEMP, 1),
        ];
        for (base, count) in people {
            for i in 0..count {
                let st = base + i * 0xE;
                let at = amiga_table(base) + (i * 0xE) as usize;
                let attributes = rom.u32(st);
                assert_eq!(
                    amiga.u32_at(at) as i32,
                    attributes + ATTRIBUTES_SHIFT,
                    "person at ST {st:#X}"
                );
                assert_eq!(st_bytes(&rom, st + 4, 10), amiga[at + 4..at + 14]);
            }
        }
    }

    /// Compares an ST record with the Amiga's, field by field: pointers
    /// as translated, the listed differences as listed, the rest as they
    /// are.
    fn compare_record(rom: &AtariRom, amiga: &[u8], name: &str, st: i32, at: usize, gfx: u32) {
        let fields = [
            (GFX, 4),
            (THINK, 4),
            (SCRIPT, 4),
            (WIDTH, 2),
            (HEIGHT, 2),
            (X, 2),
            (22, 2),
            (24, 4),
            (28, 4),
            (SPRITE, 2),
            (34, 1),
            (PAD, 1),
            (ORIGIN_X, 2),
            (ORIGIN_Y, 2),
        ];
        for (offset, len) in fields {
            let st_field = st_bytes(rom, st + offset as i32, len);
            let amiga_field = &amiga[at + offset..at + offset + len];
            let known = AMIGA_DIFFERENCES
                .iter()
                .find(|&&(entity, field, ..)| entity == name && field == offset);
            let what = format!("{name}, field {offset}");
            if let Some(&(_, _, st_value, amiga_value)) = known {
                assert_eq!(st_field, st_value, "{what} on the ST");
                assert_eq!(amiga_field, amiga_value, "{what} on the Amiga");
                continue;
            }
            match offset {
                GFX => assert_eq!(amiga_field.u32_at(0), gfx, "{what}"),
                THINK => {
                    let st_long = st_field.u32_at(0);
                    let think = THINKS.iter().find(|&&(s, _)| s == st_long);
                    let think = think.unwrap_or_else(|| panic!("{what}: ST code {st_long:#X}"));
                    assert_eq!(amiga_field.u32_at(0), think.1, "{what}");
                }
                SCRIPT => {
                    let expected = script(st_field.u32_at(0) as i32) as u32;
                    assert_eq!(amiga_field.u32_at(0), expected, "{what}");
                }
                _ => assert_eq!(st_field, amiga_field, "{what}"),
            }
        }
    }

    #[test]
    fn starting_records_are_the_sts_but_for_the_listed_differences() {
        let Some((rom, amiga)) = memories() else {
            return;
        };
        for &(name, st, at, gfx) in ENTITIES {
            compare_record(&rom, &amiga, name, st, at, gfx);
        }
        let players = (0..9)
            .map(|i| addr::PLAYERS_TEAM1 + i * PLAYER_SIZE as i32)
            .chain((0..9).map(|i| addr::PLAYERS_TEAM2 + i * PLAYER_SIZE as i32));
        for (i, st) in players.enumerate() {
            let at = (st + PLAYERS_SHIFT) as usize;
            let name = format!("player {i}");
            compare_record(&rom, &amiga, &name, st, at, gfx::PLAYERS);
            let tail = (ENTITY_SIZE..PLAYER_SIZE)
                .filter(|o| !(PLAYER_ATTRIBUTES..PLAYER_ATTRIBUTES + 4).contains(o));
            for offset in tail {
                assert_eq!(
                    rom.u8(st + offset as i32) as u8,
                    amiga[at + offset],
                    "{name}, byte {offset}"
                );
            }
            let attributes = rom.u32(st + PLAYER_ATTRIBUTES as i32);
            assert_eq!(
                amiga.u32_at(at + PLAYER_ATTRIBUTES) as i32,
                attributes + ATTRIBUTES_SHIFT,
                "{name}'s attributes"
            );
        }
    }

    #[test]
    fn scripts_are_the_sts_in_the_amigas_sprite_numbers() {
        let Some((rom, amiga)) = memories() else {
            return;
        };
        let mut renumbered = 0;
        for st in (addr::SCRIPT_BALL_THROWN_HIGH..addr::ANIMATIONS).step_by(2) {
            let word = rom.u16(st) as u16;
            let amiga_word = amiga.u16_at(script(st) as usize);
            if let Some(&(_, st_word, amiga_value)) =
                SCRIPT_DIFFERENCES.iter().find(|&&(at, ..)| at == st)
            {
                assert_eq!(
                    (word, amiga_word),
                    (st_word, amiga_value),
                    "script word at ST {st:#X}"
                );
            } else if word != amiga_word {
                assert!(
                    script_sprite(word).contains(&amiga_word),
                    "script word at ST {st:#X}: {word} on the ST, {amiga_word} on the Amiga"
                );
                renumbered += 1;
            }
        }
        assert_eq!(renumbered, 66);
        // The per-direction lists, after the scripts.
        let lists = script(addr::ANIMATIONS) as usize;
        for i in 0..19 * 8 {
            let st = rom.u32(addr::ANIMATIONS + i * 4);
            assert_eq!(
                amiga.u32_at(lists + i as usize * 4) as i32,
                script(st),
                "animation list entry {i}"
            );
        }
        // The electrobounces' own scripts sit just before the domes'.
        let domes = script(SCRIPTS_AFTER_ELECTROBOUNCES) as usize;
        assert_eq!(ELECTROBOUNCE_SCRIPTS[1] + 8, domes);
    }
}

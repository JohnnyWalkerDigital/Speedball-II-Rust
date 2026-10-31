//! The tables behind the manager screens, read from Amiga memory: the
//! keypads, the gym's upgrades and armour, labels and the squad table.

use super::super::ram::MenuRam;
use crate::data::bytes::BigEndian;
use crate::game::squad::{Upgrade, SQUAD_SIZE};

mod addr {
    pub const MANAGER_KEYPAD_TABLE: usize = 0x3D7C;
    pub const TRANSFER_KEYPAD_TABLE: usize = 0x3E24;
    pub const GYM_KEYPAD_TABLE: usize = 0x3ECC;
    pub const UPGRADES_TABLE: usize = 0x3B4E;
    pub const BUY_ALL_UPGRADES_TABLE: usize = 0x3B6E;
    /// The nineteen armour pieces on the gym's model, unlit.
    pub const ARMOUR_BRAIN_BOOST: usize = 0x3B8E;
    pub const ARMOUR_PIECES: usize = 19;
    pub const UPGRADE_NAME_NOT_ENOUGH_CASH: usize = 0x3B08;
    pub const UPGRADE_MAXIMUM_BOOST_ALREADY: usize = 0x3B36;
    /// The eight skill labels beside the stat bars.
    pub const ATTRIBUTES: usize = 0x2916;
    pub const TEAM_INFO_TABLE: usize = 0x5A64;
    pub const PLAYER_TABLE_X: usize = 0x3C94;
    pub const PLAYER_GRID_LEFT: usize = 0x3C9A;
    pub const PLAYER_GRID_RIGHT: usize = 0x3CB2;
    pub const PLAYER_GRID_UP: usize = 0x3CCA;
    pub const PLAYER_GRID_DOWN: usize = 0x3CE2;
    /// "GOAL", "LDEF", ... four letters per squad place.
    pub const POSITION_STRINGS: usize = 0x3CFC + 0x10;
    pub const STR_DEF_MID_FWD: usize = 0x5DBE;
    pub const STR_NO_PLAYERS_ON_THE_MARKET: usize = 0x28B4;
    pub const STR_A_G: usize = 0x280D;
    pub const STR_H_O: usize = 0x2817;
    pub const STR_P_R: usize = 0x2822;
    pub const STR_00000: usize = 0x2828;
    pub const MANAGER_TRANSFER_TILE_MAP: usize = 0x5DE20;
    pub const GYM_TILE_MAP: usize = 0x5DD1C;
    /// 20x13 tiles.
    pub const TILE_MAP_LENGTH: usize = 260;
}

/// What a key does (its function pointer in the original).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyAction {
    /// `keypad_fn_noop`: a blank key.
    Noop,
    Exit,
    Previous,
    Next,
    Transfer,
    Save,
    Table,
    Gym,
    Fixture,
    Statistics,
    Substitute,
    TransferPrevious,
    TransferNext,
    TransferBuy,
    GymPlayer,
    GymGroup,
    GymTeam,
    BuyAll,
    Buy,
}

impl KeyAction {
    fn from_function(address: u32) -> Self {
        match address {
            0xAC36 => KeyAction::Noop,
            0xAE2A => KeyAction::Exit,
            0xAD28 => KeyAction::Previous,
            0xAD6A => KeyAction::Next,
            0xADEA => KeyAction::Transfer,
            0x966A => KeyAction::Save,
            0xADB2 => KeyAction::Table,
            0xADDA => KeyAction::Gym,
            0xAE02 | 0xAE16 | 0xAE20 => KeyAction::Fixture,
            0xAE0C => KeyAction::Statistics,
            0xB1BE => KeyAction::Substitute,
            0xAC52 => KeyAction::TransferPrevious,
            0xACB6 => KeyAction::TransferNext,
            0xB326 => KeyAction::TransferBuy,
            0xAE2C => KeyAction::GymPlayer,
            0xAE78 => KeyAction::GymGroup,
            0xAEC6 => KeyAction::GymTeam,
            0xB160 => KeyAction::BuyAll,
            0xAF22 => KeyAction::Buy,
            other => panic!("unknown keypad function {other:#x}"),
        }
    }
}

/// A keypad button: its centre, its button sprites and what it does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Key {
    pub x: i32,
    pub y: i32,
    pub sprite: usize,
    pub pressed_sprite: usize,
    pub action: KeyAction,
}

/// Three rows of four keys; the bottom row has only its two ends.
pub type Keypad = [[Option<Key>; 4]; 3];

/// A line of small text at a pixel position.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextLine {
    pub x: i32,
    pub y: i32,
    pub text: Vec<u8>,
}

/// An armour piece on the gym's model: centre and sprite.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Armour {
    pub x: i32,
    pub y: i32,
    pub sprite: usize,
}

/// One of the gym's eight upgrade buttons.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UpgradeButton {
    pub x: i32,
    pub y: i32,
    /// The light sprite, unlit; the next sprite is lit.
    pub light: usize,
    pub upgrade: Upgrade,
    /// Two lines of name, then the price line ("000 CRD").
    pub description: [TextLine; 3],
    /// The armour pieces it lights up on the model.
    pub armours: Vec<Armour>,
}

/// Where the cursor goes from each place in the 4x3 squad grid (-1: it
/// stays, or leaves the grid).
pub type GridMoves = [[i8; 3]; 4];

pub struct ManagerTables {
    pub manager_keypad: Keypad,
    pub transfer_keypad: Keypad,
    pub gym_keypad: Keypad,
    /// In four rows of two.
    pub upgrades: Vec<UpgradeButton>,
    /// The order "BUY ALL" goes through them, as indices into `upgrades`.
    pub buy_all_order: Vec<usize>,
    pub armour_pieces: Vec<Armour>,
    pub not_enough_cash: [TextLine; 3],
    pub maximum_boost: [TextLine; 3],
    /// The skill labels, with the row each stat bar is on.
    pub attributes: Vec<(i32, Vec<u8>)>,
    /// Each squad place's spot in the squad table.
    pub table_places: [(i32, i32); SQUAD_SIZE],
    pub table_x: i32,
    pub grid_left: GridMoves,
    pub grid_right: GridMoves,
    pub grid_up: GridMoves,
    pub grid_down: GridMoves,
    pub position_names: [[u8; 4]; SQUAD_SIZE],
    /// "DEF MID FWD ".
    pub def_mid_fwd: Vec<u8>,
    pub no_players_on_the_market: Vec<u8>,
    /// The screen titles, drawn with the title letters.
    pub title_manager: Vec<u8>,
    pub title_transfer: Vec<u8>,
    pub title_gym: Vec<u8>,
    pub cash_template: Vec<u8>,
    /// The 20x13 maps of background tiles behind the manager and transfer
    /// screens, and the gym.
    pub manager_map: Vec<u8>,
    pub gym_map: Vec<u8>,
}

impl ManagerTables {
    pub fn read(ram: &MenuRam) -> Self {
        let bytes = ram.bytes();
        let word = |at: usize| bytes.u16_at(at) as i16 as i32;
        let pointer = |at: usize| bytes.u32_at(at) as usize;
        let keypad = |table: usize| -> Keypad {
            std::array::from_fn(|row| {
                std::array::from_fn(|column| {
                    let key = pointer(table + row * 16 + column * 4);
                    (key != 0).then(|| Key {
                        x: word(key),
                        y: word(key + 2),
                        sprite: word(key + 4) as usize,
                        pressed_sprite: word(key + 6) as usize,
                        action: KeyAction::from_function(bytes.u32_at(key + 8)),
                    })
                })
            })
        };
        let armour = |at: usize| Armour {
            x: word(at),
            y: word(at + 2),
            sprite: word(at + 4) as usize,
        };
        let description = |record: usize| -> [TextLine; 3] {
            std::array::from_fn(|line| {
                let at = record + line * 8;
                TextLine {
                    x: word(at),
                    y: word(at + 2),
                    text: ram.c_string(pointer(at + 4)).into_bytes(),
                }
            })
        };
        let upgrade_records: Vec<usize> = (0..8)
            .map(|i| pointer(addr::UPGRADES_TABLE + i * 4))
            .collect();
        let upgrades = ram.upgrades();
        let buttons = upgrade_records
            .iter()
            .zip(upgrades)
            .map(|(&record, upgrade)| {
                let list = pointer(record + 10);
                let armours = (0..7)
                    .map(|k| pointer(list + k * 4))
                    .take_while(|&piece| piece != 0)
                    .map(armour)
                    .collect();
                UpgradeButton {
                    x: word(record),
                    y: word(record + 2),
                    light: word(record + 4) as usize,
                    upgrade,
                    description: description(pointer(record + 14)),
                    armours,
                }
            })
            .collect();
        let buy_all_order = (0..8)
            .map(|i| {
                let record = pointer(addr::BUY_ALL_UPGRADES_TABLE + i * 4);
                upgrade_records
                    .iter()
                    .position(|&r| r == record)
                    .expect("an upgrade")
            })
            .collect();
        let grid = |table: usize| -> GridMoves {
            std::array::from_fn(|row| {
                std::array::from_fn(|column| word(table + row * 6 + column * 2) as i8)
            })
        };
        let string_bytes = |at: usize| ram.string(at);
        Self {
            manager_keypad: keypad(addr::MANAGER_KEYPAD_TABLE),
            transfer_keypad: keypad(addr::TRANSFER_KEYPAD_TABLE),
            gym_keypad: keypad(addr::GYM_KEYPAD_TABLE),
            upgrades: buttons,
            buy_all_order,
            armour_pieces: (0..addr::ARMOUR_PIECES)
                .map(|i| armour(addr::ARMOUR_BRAIN_BOOST + i * 6))
                .collect(),
            not_enough_cash: description(addr::UPGRADE_NAME_NOT_ENOUGH_CASH),
            maximum_boost: description(addr::UPGRADE_MAXIMUM_BOOST_ALREADY),
            attributes: (0..8)
                .map(|i| {
                    let record = pointer(addr::ATTRIBUTES + i * 4);
                    (word(record), string_bytes(pointer(record + 4)))
                })
                .collect(),
            table_places: std::array::from_fn(|slot| {
                let person = pointer(addr::TEAM_INFO_TABLE + slot * 4);
                (word(person + 0xA), word(person + 0xC))
            }),
            table_x: word(addr::PLAYER_TABLE_X),
            grid_left: grid(addr::PLAYER_GRID_LEFT),
            grid_right: grid(addr::PLAYER_GRID_RIGHT),
            grid_up: grid(addr::PLAYER_GRID_UP),
            grid_down: grid(addr::PLAYER_GRID_DOWN),
            position_names: std::array::from_fn(|slot| {
                std::array::from_fn(|i| bytes[addr::POSITION_STRINGS + slot * 4 + i])
            }),
            def_mid_fwd: ram.slice(addr::STR_DEF_MID_FWD, 12).to_vec(),
            no_players_on_the_market: string_bytes(addr::STR_NO_PLAYERS_ON_THE_MARKET),
            title_manager: string_bytes(addr::STR_A_G),
            title_transfer: string_bytes(addr::STR_H_O),
            title_gym: string_bytes(addr::STR_P_R),
            cash_template: string_bytes(addr::STR_00000),
            manager_map: ram
                .slice(addr::MANAGER_TRANSFER_TILE_MAP, addr::TILE_MAP_LENGTH)
                .to_vec(),
            gym_map: ram
                .slice(addr::GYM_TILE_MAP, addr::TILE_MAP_LENGTH)
                .to_vec(),
        }
    }
}

//! The menus' strings, tables and fonts, read from Amiga memory at their
//! original addresses (see [`crate::data::amiga::menu_ram`]).

use crate::data::bytes::BigEndian;
use crate::game::career::{TeamId, TeamRecord, LEAGUE_TEAMS};

/// Amiga RAM addresses the menu code uses, named as in the disassembly.
pub mod addr {
    // Titles: a pair of string pointers, drawn at the top and bottom.
    pub const TITLE_SPEEDBALL_2_PUSH_START: usize = 0x2844;
    pub const TITLE_SELECT_GAME: usize = 0x2874;
    pub const TITLE_TWO_PLAYER: usize = 0x2894;

    // Menus: title, items, arrow positions and item count.
    pub const STR_MENU_GAME_SELECT: usize = 0x34A8;
    pub const MENU_ARROWS_GAME_TYPE: usize = 0x34E0;
    pub const MENU_SELECT_MATCH: usize = 0x3500;
    pub const MENU_SELECT_ONE_THREE_FIVE: usize = 0x359A;

    pub const STR_MATCH: usize = 0x36A7;
    pub const STR_PLAYER_WINS: usize = 0x36B1;
    pub const STR_SECOND_LEG: usize = 0x3733;
    pub const STR_SCORE: usize = 0x3772;
    pub const STR_TOTAL: usize = 0x3785;
    pub const STR_SCORE_WIN: usize = 0x3798;
    pub const STR_BLUE_WIN: usize = 0x37B7;
    pub const STR_RED_WIN: usize = 0x37C8;
    pub const STR_MATCH_DRAWN: usize = 0x37D8;
    pub const STR_GAME_OVER: usize = 0x37E6;

    /// The 16 league teams, 0x40 bytes each; a team starts with pointers
    /// to its full and short names, then its record.
    pub const TEAMS: usize = 0x7B50;
    pub const TEAM_PLAYER_2: usize = 0x7B10;
    pub const TEAM_SIZE: usize = 0x40;

    // Fonts, all in overlay 1.
    /// 16x16 letters for menus and messages (`sprites_menu_font`).
    pub const SPRITES_MENU_FONT: usize = 0x44B5C;
    pub const MENU_FONT_LENGTH: usize = 0x26 * 160;
    /// 8x8 letters for the title at the top and bottom of the backdrop.
    pub const SPRITES_FONTS_TITLES_TOP: usize = 0x4631C;
    pub const TITLES_TOP_LENGTH: usize = 1120;
    pub const SPRITES_FONTS_TITLES_BOTTOM: usize = 0x4677C;
    pub const TITLES_BOTTOM_LENGTH: usize = 800;
    /// 8x8 letters for the victory and defeat pictures.
    pub const SPRITES_FONTS_WHITE: usize = 0x475DC;
    pub const WHITE_LENGTH: usize = 1440;
}

/// A screen title: the text at the top and bottom of the backdrop.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Title {
    pub top: Vec<u8>,
    pub bottom: Vec<u8>,
}

/// A list menu (`shared_menu_function`'s menu structure).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MenuSpec {
    pub title: Title,
    pub items: Vec<u8>,
    pub arrows: Vec<Arrows>,
}

/// Where the two arrows either side of a menu item go, in text cells.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Arrows {
    pub left: (u8, u8),
    pub right: (u8, u8),
}

pub struct MenuRam {
    bytes: Vec<u8>,
}

impl MenuRam {
    pub fn new(bytes: Vec<u8>) -> Self {
        Self { bytes }
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn slice(&self, address: usize, length: usize) -> &[u8] {
        &self.bytes[address..address + length]
    }

    /// A string in the game's text format: cursor codes and letters up to
    /// the NUL. Strings that start with a cursor position keep it.
    pub fn string(&self, address: usize) -> Vec<u8> {
        // The first two bytes are always the cursor, even if one is 0.
        let end = self.bytes[address + 2..]
            .iter()
            .position(|&b| b == 0)
            .map_or(self.bytes.len(), |n| address + 2 + n);
        self.bytes[address..end].to_vec()
    }

    pub fn title(&self, address: usize) -> Title {
        Title {
            top: self.string(self.pointer(address)),
            bottom: self.string(self.pointer(address + 4)),
        }
    }

    pub fn menu(&self, address: usize) -> MenuSpec {
        let count = usize::from(self.bytes.u16_at(address + 12));
        MenuSpec {
            title: self.title(self.pointer(address)),
            items: self.string(self.pointer(address + 4)),
            arrows: self.arrows(self.pointer(address + 8), count),
        }
    }

    /// `count` arrow pairs, four words each.
    pub fn arrows(&self, address: usize, count: usize) -> Vec<Arrows> {
        let word = |at: usize| self.bytes.u16_at(at) as u8;
        (0..count)
            .map(|i| {
                let at = address + i * 8;
                Arrows {
                    left: (word(at), word(at + 2)),
                    right: (word(at + 4), word(at + 6)),
                }
            })
            .collect()
    }

    /// A team's full name, cursor bytes included.
    pub fn team_name(&self, team: TeamId) -> Vec<u8> {
        self.string(self.pointer(Self::team_address(team)))
    }

    fn team_address(team: TeamId) -> usize {
        match team {
            TeamId::League(index) => addr::TEAMS + index * addr::TEAM_SIZE,
            TeamId::PlayerTwo => addr::TEAM_PLAYER_2,
        }
    }

    /// The results stored with every team, league teams first.
    pub fn team_records(&self) -> [TeamRecord; LEAGUE_TEAMS + 1] {
        std::array::from_fn(|i| {
            let team = Self::team_address(match i {
                LEAGUE_TEAMS => TeamId::PlayerTwo,
                i => TeamId::League(i),
            });
            let word = |offset: usize| self.bytes.u16_at(team + offset);
            TeamRecord {
                played: word(0x8),
                won: word(0xA),
                drawn: word(0xC),
                lost: word(0xE),
                points_for: word(0x10),
                points_against: word(0x12),
                league_points: word(0x14),
                league_placing: self.bytes[team + 0x16],
                old_league_placing: self.bytes[team + 0x17],
            }
        })
    }

    fn pointer(&self, address: usize) -> usize {
        self.bytes.u32_at(address) as usize
    }
}

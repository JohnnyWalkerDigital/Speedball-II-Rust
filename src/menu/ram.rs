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
    pub const TITLE_NEXT_FIXTURE: usize = 0x284C;
    pub const TITLE_DIVISION_1: usize = 0x2854;
    pub const TITLE_DIVISION_2: usize = 0x285C;
    pub const TITLE_RESULTS: usize = 0x2864;

    // Menus: title, items, arrow positions and item count.
    pub const STR_MENU_GAME_SELECT: usize = 0x34A8;
    pub const MENU_ARROWS_GAME_TYPE: usize = 0x34E0;
    pub const MENU_SELECT_MATCH: usize = 0x3500;
    pub const MENU_SELECT_ONE_THREE_FIVE: usize = 0x359A;
    /// "NEW GAME", "TEAM MANAGER", "LOAD GAME".
    pub const MENU_SELECT_GAME: usize = 0x3550;
    /// "NEW GAME", "LOAD GAME".
    pub const MENU_SELECT_NEW_LOAD: usize = 0x3618;

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
    pub const STR_TOTAL_SCORE_WIN: usize = 0x374D;
    pub const STR_NEW_SEASON: usize = 0x3740;
    pub const STR_RELEGATE: usize = 0x371C;
    pub const STR_PLAYOFF: usize = 0x3729;
    /// "ROUND ONE" to "GRAND FINAL", one pointer per cup round.
    pub const STRINGS_CUP_ROUNDS: usize = 0x3700;

    // Panel screens: a count, then string pointers (`draw_string_list`).
    pub const STRINGS_LEAGUE_PLACINGS: usize = 0x3356;
    pub const STRINGS_RESULTS: usize = 0x33A8;
    pub const STRINGS_NEXT_FIXTURE: usize = 0x32BE;
    /// A table row, filled in by `construct_league_placing_string`.
    pub const STR_LEAGUE_TEAM_DETAILS: usize = 0x2E2D;
    pub const STR_LEAGUE_MATCH_RESULT: usize = 0x3205;
    pub const STR_PLAY: usize = 0x2976;
    // The next-fixture lines with numbers in them.
    pub const STR_PWDLP_NUMS: usize = 0x2A46;
    pub const STR_LEAGUE_PLACING: usize = 0x2ADF;
    pub const STR_POINTS_FOR: usize = 0x2B12;
    pub const STR_POINTS_AGAINST: usize = 0x2B45;
    pub const STR_DEFENCE: usize = 0x2B78;
    pub const STR_MIDFIELD: usize = 0x2BAB;
    pub const STR_ATTACK: usize = 0x2BDE;
    pub const STR_SUBSTITUTES: usize = 0x2C11;

    /// Four tables of 16x16 results for matches the player is not in.
    pub const SCORES_TABLE: usize = 0x5D51C;
    pub const SCORES_TABLE_LENGTH: usize = 4 * 16 * 16 * 2;

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
    /// 8x8 letters, digits and table borders (`sprites_font_orange`).
    pub const SPRITES_FONT_ORANGE: usize = 0x44364;
    pub const ORANGE_LENGTH: usize = 2040;
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

    /// A list of strings: a word count, then pointers.
    pub fn string_list(&self, address: usize) -> Vec<Vec<u8>> {
        self.pointer_list(address)
            .into_iter()
            .map(|p| self.string(p))
            .collect()
    }

    /// The pointers in a string list.
    pub fn pointer_list(&self, address: usize) -> Vec<usize> {
        let count = usize::from(self.bytes.u16_at(address));
        (0..count)
            .map(|i| self.pointer(address + 2 + i * 4))
            .collect()
    }

    /// The `index`th of a table of string pointers.
    pub fn string_at(&self, table: usize, index: usize) -> Vec<u8> {
        self.string(self.pointer(table + index * 4))
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

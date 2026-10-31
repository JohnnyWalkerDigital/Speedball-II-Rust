//! The menus' strings, tables and fonts, read from Amiga memory at their
//! original addresses (see [`crate::data::amiga::menu_ram`]).

use super::strings::{Arrows, MenuSpec, Title};
use crate::data::bytes::BigEndian;
use crate::game::career::{TeamId, TeamRecord, LEAGUE_TEAMS};
use crate::game::squad::{MarketPlayer, Squad, SquadPlayer, Upgrade, MARKET_SIZE, SQUAD_SIZE};

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
    /// "PLAYER 0", before each player's turn in the gym.
    pub const STR_PLAYER_0: usize = 0x36C1;
    pub const STR_SECOND_LEG: usize = 0x3733;
    pub const STR_SCORE: usize = 0x3772;
    pub const STR_TOTAL: usize = 0x3785;
    pub const STR_SCORE_WIN: usize = 0x3798;
    pub const STR_BLUE_WIN: usize = 0x37B7;
    pub const STR_RED_WIN: usize = 0x37C8;
    pub const STR_MATCH_DRAWN: usize = 0x37D8;
    pub const STR_GAME_OVER: usize = 0x37E6;
    pub const STR_REPLAYS_OVER: usize = 0x37F2;
    pub const TITLE_REPLAY_GOALS: usize = 0x2834;
    pub const STR_INSERT_GOAL_SAVE_DISK: usize = 0x366C;
    /// The panel message for a replay from disk, "GOAL 0" with the slot's
    /// digit at `STR_GOAL_DIGIT`.
    pub const STR_GOAL: usize = 0x2657;
    pub const STR_GOAL_DIGIT: usize = 9;
    /// "DISK ERROR", "FIRE RETRY", "ESC CANCEL" on the match panel.
    pub const STR_DISK_ERROR: usize = 0x266B;
    /// `maybe_serial`: a long the loader leaves at boot, saved with each
    /// replay.
    pub const MAYBE_SERIAL: usize = 0x80;
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
    pub const TITLE_STATISTICS: usize = 0x2884;
    pub const TITLE_SAVE_GAME: usize = 0x289C;
    pub const TITLE_LOAD_GAME: usize = 0x28A4;
    /// "PLEASE INSERT GAME SAVE DISK".
    pub const STR_INSERT_GAME_SAVE_DISK: usize = 0x364C;
    /// "DISK ERROR": "RETRY", "CANCEL".
    pub const MENU_SELECT_RETRY_CANCEL: usize = 0x35EA;
    /// Saved names are offsets from here (the first market player's).
    pub const STR_JAMS: usize = 0x5DCA;
    pub const TITLE_FIXTURE_LIST: usize = 0x288C;
    pub const STRINGS_STATISTICS: usize = 0x33FE;
    pub const STRINGS_FIXTURE_LIST: usize = 0x3454;
    pub const STR_SHOTS_AT_GOAL: usize = 0x2F68;
    pub const STR_GOALS_SCORED: usize = 0x2F94;
    pub const STR_GOALS_SAVED: usize = 0x2FC0;
    pub const STR_TIME_IN_POSSESSION: usize = 0x2FEC;
    pub const STR_TIME_IN_OPPONENTS_HALF: usize = 0x3018;
    pub const STR_SUCCESSFUL_TACKLES: usize = 0x3044;
    pub const STR_SUBSTITUTIONS: usize = 0x3070;
    pub const STR_BONUS_POINTS: usize = 0x309C;
    // The next-fixture lines with numbers in them.
    pub const STR_PWDLP_NUMS: usize = 0x2A46;
    pub const STR_LEAGUE_PLACING: usize = 0x2ADF;
    pub const STR_POINTS_FOR: usize = 0x2B12;
    pub const STR_POINTS_AGAINST: usize = 0x2B45;
    pub const STR_DEFENCE: usize = 0x2B78;
    pub const STR_MIDFIELD: usize = 0x2BAB;
    pub const STR_ATTACK: usize = 0x2BDE;
    pub const STR_SUBSTITUTES: usize = 0x2C11;

    // The manager's data.
    /// Twelve squad places: stats pointer, name pointer, face, and the
    /// place's position in the squad table.
    pub const TEAM_INFO_TABLE: usize = 0x5A64;
    pub const TEAM_INFO_TABLE_OTHER: usize = 0x5B3C;
    /// The squad's names and faces, restored at the main menu.
    pub const BASE_TEAM_SPRITE_OFFSETS: usize = 0x3F74;
    pub const BASE_TEAM_NAMES: usize = 0x3FB0;
    pub const PLAYERS_MARKET: usize = 0x5D76;
    /// The gym's eight upgrades, in four rows of two.
    pub const UPGRADES_TABLE: usize = 0x3B4E;

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
    // The manager screens' graphics.
    pub const PALETTE_MANAGEMENT_A: usize = 0x132;
    pub const SPRITES_MGMT_BACKGROUND: usize = 0x3D744;
    pub const SPRITES_PLAYER_FACES: usize = 0x531FC;
    pub const SPRITES_GROUP_LOGO: usize = 0x5B8FC;
    pub const SPRITES_MGMT_LIGHTS: usize = 0x47B7C;
    pub const SPRITES_MGMT_BUTTONS: usize = 0x4857C;
    pub const SPRITES_MGMT_ARMOUR: usize = 0x4D57C;
    pub const SPRITES_FONTS_SMALL_GREEN: usize = 0x4703C;
    pub const SPRITES_FONTS_MGR_XFER_GYM: usize = 0x46BDC;
    /// Eight glyphs of its own; its digits are the title letters' last ten.
    pub const SPRITES_FONTS_CASH: usize = 0x46A9C;

    /// 8x8 letters, digits and table borders (`sprites_font_orange`).
    pub const SPRITES_FONT_ORANGE: usize = 0x44364;
    pub const ORANGE_LENGTH: usize = 2040;
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

    /// A NUL-terminated string with no cursor bytes.
    pub fn c_string(&self, address: usize) -> String {
        let end = self.bytes[address..]
            .iter()
            .position(|&b| b == 0)
            .map_or(self.bytes.len(), |n| address + n);
        String::from_utf8_lossy(&self.bytes[address..end]).into_owned()
    }

    /// A squad (`team_info_table` or `team_info_table_other`).
    pub fn squad(&self, table: usize) -> Squad {
        std::array::from_fn(|i| self.person(self.pointer(table + i * 4)))
    }

    /// The squad names and faces `init_team_players_names_and_sprites`
    /// restores.
    pub fn base_identities(&self) -> [(String, u16); SQUAD_SIZE] {
        std::array::from_fn(|i| {
            let name = self.c_string(self.pointer(addr::BASE_TEAM_NAMES + i * 4));
            (
                name,
                self.bytes.u16_at(addr::BASE_TEAM_SPRITE_OFFSETS + i * 2),
            )
        })
    }

    /// The players on the transfer market, as stored.
    pub fn market(&self) -> Vec<MarketPlayer> {
        (0..MARKET_SIZE)
            .map(|i| {
                let record = self.pointer(addr::PLAYERS_MARKET + i * 4);
                MarketPlayer {
                    player: self.person(record),
                    change_time: self.bytes.u16_at(record + 0xE) as i16,
                    buyable: self.bytes.u16_at(record + 0x10) != 0,
                }
            })
            .collect()
    }

    /// The gym's upgrades: the skill each improves and its price.
    pub fn upgrades(&self) -> [Upgrade; 8] {
        std::array::from_fn(|i| {
            let record = self.pointer(addr::UPGRADES_TABLE + i * 4);
            Upgrade {
                skill: usize::from(self.bytes.u16_at(record + 6) - 0x48),
                price: self.bytes.u16_at(record + 8),
            }
        })
    }

    /// A person record: stats pointer, name pointer, face.
    fn person(&self, record: usize) -> SquadPlayer {
        let stats = self.pointer(record);
        SquadPlayer {
            name: self.c_string(self.pointer(record + 4)),
            face: self.bytes.u16_at(record + 8),
            position: self.bytes[stats + 2],
            skills: std::array::from_fn(|skill| self.bytes[stats + 3 + skill]),
        }
    }

    /// The 32-bit pointer stored at `address`.
    pub fn pointer_at(&self, address: usize) -> usize {
        self.pointer(address)
    }

    pub fn long(&self, address: usize) -> u32 {
        self.bytes.u32_at(address)
    }

    fn pointer(&self, address: usize) -> usize {
        self.bytes.u32_at(address) as usize
    }
}

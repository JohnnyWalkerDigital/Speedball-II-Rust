//! Atari ST RAM addresses of the game's data.
//!
//! The engine reads its initial state and lookup tables straight out of the
//! ST disk image, so these addresses are part of the data format. Names
//! describe how the code uses each table.

// ---------------------------------------------------------------------------
// Entity records (40 bytes each, 42 apart for the pitch furniture).
// ---------------------------------------------------------------------------

pub const BALL: i32 = 0x4812;
/// The launcher in the centre of the pitch.
pub const LAUNCHER: i32 = 0x483A;
/// The ball flying up out of the launcher.
pub const BIG_BALL: i32 = 0x4862;
pub const COINS: [i32; 4] = [0x488A, 0x48B2, 0x48DA, 0x4902];
pub const TOKENS: [i32; 2] = [0x492A, 0x4952];
pub const EQUIPMENT: i32 = 0x497A;
pub const GOAL_INDICATORS: [i32; 2] = [0x49A2, 0x49CA];
pub const PLAYER_INDICATORS: [i32; 2] = [0x49F2, 0x4A1A];
pub const STATUS_LED: i32 = 0x4A42;
pub const SCORE_MULTIPLIERS: [i32; 4] = [0x4A6A, 0x4A92, 0x4ABA, 0x4AE2];
pub const BOUNCE_DOMES: [i32; 2] = [0x4B0A, 0x4B34];
pub const ELECTROBOUNCES: [i32; 2] = [0x4B5E, 0x4B88];
pub const PLAYERS_TEAM1: i32 = 0x4BB2;
pub const PLAYERS_TEAM2: i32 = 0x4FB4;
pub const MEDICS: [i32; 2] = [0x53B6, 0x53DE];
pub const TERRAIN_CENTER: i32 = 0x5406;

// ---------------------------------------------------------------------------
// Roster data.
// ---------------------------------------------------------------------------

pub const ATTRIBUTES_TEAM1: i32 = 0x40A6;
pub const ATTRIBUTES_TEAM2: i32 = 0x412A;
pub const ATTRIBUTES_TEMP: i32 = 0x41AE;
pub const ATTRIBUTES_TEMP_END: i32 = 0x41B9;
pub const ATTRIBUTES_MARKET: i32 = 0x43D8;
pub const ATTRIBUTES_MARKET_END: i32 = 0x449E;
pub const PERSONS_TEAM1: i32 = 0x421A;
pub const PERSONS_TEAM2: i32 = 0x4300;
pub const PERSON_TEMP: i32 = 0x42C2;
pub const TEAMS: i32 = 0x63D4;
pub const TEAM_PLAYER2: i32 = 0x6394;

pub const PALETTE_MATCH: i32 = 0x44E;

// ---------------------------------------------------------------------------
// Lookup tables.
// ---------------------------------------------------------------------------

pub const DIR_FROM_DIR_BITS: i32 = 0x4BC;
pub const THINK_DELAY_BY_INT: i32 = 0x532;
pub const PREDICTION_SHIFT_BY_INT: i32 = 0x542;
pub const JUMP_BONUS_BY_SPD: i32 = 0x562;
pub const TACKLE_DEF_MALUS_BY_ANGLE: i32 = 0x582;
pub const STATUS_LEDS: i32 = 0x2A8A;
pub const BALL_OFFSET_BY_SPRITE: i32 = 0x2AA2;
pub const SPRITE_ORIGIN: i32 = 0x2B8C;
pub const SUPPORT_POSITIONS: i32 = 0x2E0C;
pub const SUPPORT_POSITIONS_FORWARD: i32 = 0x2EE4;
pub const EQUIPMENT_SKILL: i32 = 0x2FC5;
pub const MIRROR_X_DIR: i32 = 0x6086;
pub const MIRROR_Y_DIR: i32 = 0x608E;
pub const VELOCITY_BY_SPEED_DIR: i32 = 0x6096;
pub const GOALIE_DEFLECT_DIR: i32 = 0xD714;

/// Ball paths through the score-multiplier ramps, as signed (dx, dy) bytes.
pub const MULTIPLIER_PATH_LEFT_UP: i32 = 0x2C7C;
pub const MULTIPLIER_PATH_LEFT_DOWN: i32 = 0x2CE0;
pub const MULTIPLIER_PATH_RIGHT_DOWN: i32 = 0x2D44;
pub const MULTIPLIER_PATH_RIGHT_UP: i32 = 0x2DA8;

// ---------------------------------------------------------------------------
// Animation scripts (see `animations.rs`).
// ---------------------------------------------------------------------------

/// The per-direction animations: 19 lists of 8 script pointers, one after
/// another.
pub const ANIMATIONS: i32 = 0x5E26;

/// Single scripts (not per-direction lists).
pub const SCRIPT_BALL_THROWN_HIGH: i32 = 0x54C6;
pub const SCRIPT_BALL_THROWN: i32 = 0x5528;
pub const SCRIPT_BALL_HELD_JUMPING: i32 = 0x554C;
pub const SCRIPT_BALL_HELD: i32 = 0x5588;
pub const SCRIPT_BALL_IN_MULTIPLIER: i32 = 0x55BE;
pub const SCRIPT_BALL_LAUNCH: i32 = 0x561E;
pub const SCRIPT_PLAYER_ON_GROUND: i32 = 0x5B72;
pub const SCRIPT_PLAYER_CARRIED: i32 = 0x5B76;
pub const SCRIPT_MEDIC_CARRY_LEFT: i32 = 0x5DA6;
pub const SCRIPT_MEDIC_CARRY_RIGHT: i32 = 0x5DAA;
pub const SCRIPT_MEDIC_LIFT_A: i32 = 0x5DAE;
pub const SCRIPT_MEDIC_LIFT_B: i32 = 0x5DB6;

// ---------------------------------------------------------------------------
// Graphics bank pointers stored in entity records.
// ---------------------------------------------------------------------------

pub const GFX_ENTITIES: u32 = 0x193EA;
pub const GFX_PLAYERS: u32 = 0x1C0EA;
pub const GFX_BIG_BALL: u32 = 0x2C4EA;
pub const GFX_PITCH: u32 = 0x537EA;

//! Sounds the match asks for. The match only records them; the sound
//! driver plays them.
//!
//! Each constant is a sound the original Amiga game plays: a sound ID (an
//! index into the driver's sound table) and the priority it is played at.
//! Some IDs are shared by several calls (crowd reactions mostly), so the
//! constants are named for the call, not the noise.

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Sound {
    pub id: u8,
    pub priority: u8,
}

impl Sound {
    const fn new(id: u8, priority: u8) -> Self {
        Self { id, priority }
    }

    // -- Match flow ----------------------------------------------------------

    /// Stops channels 0-2: on a goal, when a substitute comes on, and at the
    /// start and end of a match.
    pub const SILENCE: Sound = Sound::new(0, 0);
    /// Stops channel 0: on a new panel message, when an action ends, and on
    /// an injury.
    pub const SILENCE_CHANNEL_0: Sound = Sound::new(43, 0);
    /// "Get ready" with crowd, at kick-off.
    pub const GET_READY_KICK_OFF: Sound = Sound::new(12, 1);
    /// "Get ready": panel back to "VS" after a goal, half time, substitute
    /// on.
    pub const GET_READY: Sound = Sound::new(46, 0);
    /// Everyone in position; the launcher starts.
    pub const LAUNCHER_START: Sound = Sound::new(9, 1);
    /// The launcher fires the ball.
    pub const BALL_LAUNCH: Sound = Sound::new(40, 8);
    /// Once a second while a power-up is active.
    pub const POWERUP_TICK: Sound = Sound::new(8, 2);
    /// Final whistle.
    pub const MATCH_OVER: Sound = Sound::new(2, 9);
    /// Teams lined up after the final whistle.
    pub const END_OF_MATCH: Sound = Sound::new(39, 8);

    // -- Scoring and crowd ----------------------------------------------------

    /// Goal.
    pub const GOAL: Sound = Sound::new(32, 8);
    /// Crowd after a goal by the team that started attacking up.
    pub const GOAL_CROWD_UP: Sound = Sound::new(1, 8);
    /// Crowd after a goal by the team that started attacking down; also the
    /// cheer when the last thrower's team started attacking up, and after
    /// injuries.
    pub const CROWD_CHEER: Sound = Sound::new(42, 8);
    /// Interception or steal by the team that started attacking up.
    pub const INTERCEPTION_UP: Sound = Sound::new(41, 8);
    /// Interception or steal by the team that started attacking down.
    pub const INTERCEPTION_DOWN: Sound = Sound::new(40, 8);
    /// A row of five stars completed.
    pub const STAR_ROW: Sound = Sound::new(25, 8);

    // -- Pitch furniture ------------------------------------------------------

    /// A star lit or knocked out.
    pub const STAR: Sound = Sound::new(17, 8);
    /// Ball bounces off a score-multiplier ramp.
    pub const MULTIPLIER_BOUNCE: Sound = Sound::new(53, 5);
    /// A multiplier goes up or down.
    pub const MULTIPLIER_CHANGE: Sound = Sound::new(24, 8);
    /// Ball through a warp tunnel.
    pub const WARP: Sound = Sound::new(19, 7);
    /// Ball off a bounce dome.
    pub const DOME: Sound = Sound::new(16, 1);
    /// Ball through an electrobounce.
    pub const ELECTROBOUNCE: Sound = Sound::new(11, 7);
    /// Ball off a side wall, low and high.
    pub const SIDE_WALL_LOW: Sound = Sound::new(4, 5);
    pub const SIDE_WALL_HIGH: Sound = Sound::new(50, 5);
    /// Ball off an end wall, low and high.
    pub const END_WALL_LOW: Sound = Sound::new(51, 5);
    pub const END_WALL_HIGH: Sound = Sound::new(52, 5);

    // -- Players ------------------------------------------------------------------

    /// A moving ball caught.
    pub const CATCH: Sound = Sound::new(14, 1);
    /// Throw, by the computer or a human; also a goalkeeper punching the
    /// ball away.
    pub const THROW: Sound = Sound::new(3, 1);
    /// Jump for a high ball.
    pub const JUMP: Sound = Sound::new(29, 1);
    /// Landing from a jump.
    pub const LAND: Sound = Sound::new(36, 1);
    /// Sliding tackle or goalkeeper dive.
    pub const SLIDE: Sound = Sound::new(5, 1);
    /// A tackle reaches an opponent.
    pub const TACKLE: Sound = Sound::new(6, 1);
    /// Injury.
    pub const INJURY: Sound = Sound::new(15, 9);
    /// "Ice cream! Ice cream!" when the medics lift a player, on a coin flip
    /// (also during goal replays).
    pub const ICE_CREAM: Sound = Sound::new(10, 1);

    /// Grunts when a player is knocked down, picked by the victim's random
    /// byte.
    pub fn knocked_down(random: i32) -> Sound {
        const TACKLE_NOISES: [u8; 4] = [7, 47, 48, 49];
        Sound::new(TACKLE_NOISES[(random & 3) as usize], 2)
    }

    // -- Pickups -------------------------------------------------------------------

    /// Coins or armour appear.
    pub const ITEM_APPEARS: Sound = Sound::new(37, 6);
    /// A power-up token appears.
    pub const TOKEN_APPEARS: Sound = Sound::new(37, 3);
    pub const COIN: Sound = Sound::new(21, 5);
    pub const TOKEN: Sound = Sound::new(28, 5);
    pub const ARMOUR: Sound = Sound::new(33, 5);
    /// Power-up effects: grab ball, teleport, full energy, zap.
    pub const GRAB_BALL: Sound = Sound::new(28, 6);
    pub const TELEPORT: Sound = Sound::new(27, 6);
    pub const FULL_ENERGY: Sound = Sound::new(31, 6);
    pub const ZAP: Sound = Sound::new(18, 6);
}

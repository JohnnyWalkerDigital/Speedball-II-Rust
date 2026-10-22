//! Sounds the match asks for. The match only records them; the sound
//! driver plays them.
//!
//! Each constant is a `sound_play` call from the original Amiga code: a
//! sound ID (an index into the driver's `sound_table`) and the priority the
//! call site passes. The C# port dropped all sound; these were recovered from
//! the Amiga disassembly, and the original routine is noted for each. Some
//! IDs are shared by several calls (crowd reactions mostly), so the
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

    /// Stops channels 0-2 (`handle_score_goal`, `step_injury_3`,
    /// `play_match`).
    pub const SILENCE: Sound = Sound::new(0, 0);
    /// Stops channel 0 (`build_text_overlay` on a new panel message,
    /// `complete_action_fn`, `start_injury`).
    pub const SILENCE_CHANNEL_0: Sound = Sound::new(43, 0);
    /// "Get ready" with crowd, at kick-off (`play_match`).
    pub const GET_READY_KICK_OFF: Sound = Sound::new(12, 1);
    /// "Get ready": panel back to "VS" after a goal
    /// (`step_prepare_ball_launch`), half time (`update_match_time`),
    /// substitute on (`step_injury_3`).
    pub const GET_READY: Sound = Sound::new(46, 0);
    /// Everyone in position; the launcher starts (`step_prepare_ball_launch`).
    pub const LAUNCHER_START: Sound = Sound::new(9, 1);
    /// The launcher fires the ball (`step_ball_launch`).
    pub const BALL_LAUNCH: Sound = Sound::new(40, 8);
    /// Once a second while a power-up is active (`display_time`).
    pub const POWERUP_TICK: Sound = Sound::new(8, 2);
    /// Final whistle (`update_match_time`).
    pub const MATCH_OVER: Sound = Sound::new(2, 9);
    /// Teams lined up after the final whistle (`play_match`).
    pub const END_OF_MATCH: Sound = Sound::new(39, 8);

    // -- Scoring and crowd ----------------------------------------------------

    /// Goal (`handle_score_goal`).
    pub const GOAL: Sound = Sound::new(32, 8);
    /// Crowd after a goal by the team that started attacking up
    /// (`goal_sound_function`).
    pub const GOAL_CROWD_UP: Sound = Sound::new(1, 8);
    /// Crowd after a goal by the team that started attacking down
    /// (`goal_sound_function`); also the cheer in `play_sound_downwards`
    /// and after injuries (`start_injury`).
    pub const CROWD_CHEER: Sound = Sound::new(42, 8);
    /// Interception or steal by the team that started attacking up
    /// (`get_ball`, `do_tackle`).
    pub const INTERCEPTION_UP: Sound = Sound::new(41, 8);
    /// Interception or steal by the team that started attacking down.
    pub const INTERCEPTION_DOWN: Sound = Sound::new(40, 8);
    /// A row of five stars completed (`check_all_stars_lit`).
    pub const STAR_ROW: Sound = Sound::new(25, 8);

    // -- Pitch furniture ------------------------------------------------------

    /// A star lit or knocked out (`stars_light_p1/p2`, `step_stars`).
    pub const STAR: Sound = Sound::new(17, 8);
    /// Ball bounces off a score-multiplier ramp (`step_start_multiplier`).
    pub const MULTIPLIER_BOUNCE: Sound = Sound::new(53, 5);
    /// A multiplier goes up or down (`change_score_multiplier`).
    pub const MULTIPLIER_CHANGE: Sound = Sound::new(24, 8);
    /// Ball through a warp tunnel (`step_warp`).
    pub const WARP: Sound = Sound::new(19, 7);
    /// Ball off a bounce dome (`step_bumper`).
    pub const DOME: Sound = Sound::new(16, 1);
    /// Ball through an electrobounce (`step_zapper`).
    pub const ELECTROBOUNCE: Sound = Sound::new(11, 7);
    /// Ball off a side wall, low and high (`constrain_sprite`).
    pub const SIDE_WALL_LOW: Sound = Sound::new(4, 5);
    pub const SIDE_WALL_HIGH: Sound = Sound::new(50, 5);
    /// Ball off an end wall, low and high (`constrain_sprite`).
    pub const END_WALL_LOW: Sound = Sound::new(51, 5);
    pub const END_WALL_HIGH: Sound = Sound::new(52, 5);

    // -- Players ------------------------------------------------------------------

    /// A moving ball caught (`get_ball`).
    pub const CATCH: Sound = Sound::new(14, 1);
    /// Throw (`do_throw_common_ai`, `handle_user_input`); also a goalkeeper
    /// punching the ball away (`goalie_deflect_ball`).
    pub const THROW: Sound = Sound::new(3, 1);
    /// Jump for a high ball (`slide_or_jump_at_target`, `handle_user_input`).
    pub const JUMP: Sound = Sound::new(29, 1);
    /// Landing from a jump (`jumping_action_fn`).
    pub const LAND: Sound = Sound::new(36, 1);
    /// Sliding tackle or goalkeeper dive (`do_slide`).
    pub const SLIDE: Sound = Sound::new(5, 1);
    /// A tackle reaches an opponent (`do_tackle`).
    pub const TACKLE: Sound = Sound::new(6, 1);
    /// Injury (`start_injury`).
    pub const INJURY: Sound = Sound::new(15, 9);
    /// "Ice cream! Ice cream!" when the medics lift a player, on a coin flip
    /// (`step_injury_2`; also `build_text_overlay` during goal replays).
    pub const ICE_CREAM: Sound = Sound::new(10, 1);

    /// Grunts when a player is knocked down (`tackle_noises` at `$01F6`),
    /// picked by the victim's random byte (`do_tackle`, `get_ball`).
    pub fn knocked_down(random: i32) -> Sound {
        const TACKLE_NOISES: [u8; 4] = [7, 47, 48, 49];
        Sound::new(TACKLE_NOISES[(random & 3) as usize], 2)
    }

    // -- Pickups -------------------------------------------------------------------

    /// Coins or armour appear (`spawn_cash`, `spawn_armour`).
    pub const ITEM_APPEARS: Sound = Sound::new(37, 6);
    /// A power-up token appears (`spawn_powerup`).
    pub const TOKEN_APPEARS: Sound = Sound::new(37, 3);
    pub const COIN: Sound = Sound::new(21, 5);
    pub const TOKEN: Sound = Sound::new(28, 5);
    pub const ARMOUR: Sound = Sound::new(33, 5);
    /// Power-up effects (`powerup_grab`, `_teleport`, `_energy`, `_zap`).
    pub const GRAB_BALL: Sound = Sound::new(28, 6);
    pub const TELEPORT: Sound = Sound::new(27, 6);
    pub const FULL_ENERGY: Sound = Sound::new(31, 6);
    pub const ZAP: Sound = Sound::new(18, 6);
}

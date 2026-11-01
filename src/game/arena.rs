//! Pitch furniture: wall stars, score-multiplier ramps, bounce domes and
//! electrobounces.

use super::entity::{Entity, EntityId, Think};
use super::events::Sound;
use super::geometry::{PerTeam, Point, Side};
use super::state::Match;
use crate::data::animations::Script;
use crate::data::lookups::MultiplierPath;
use crate::data::Rom;

pub struct Arena {
    pub stars: Stars,
    pub multipliers: ScoreMultipliers,
    pub domes: [Entity; 2],
    pub electrobounces: Electrobounces,
}

impl Arena {
    pub fn new(rom: &Rom) -> Self {
        Self {
            stars: Stars::default(),
            multipliers: ScoreMultipliers::new(rom),
            domes: [0, 1].map(|i| rom.start().entity(EntityId::BounceDome(i))),
            electrobounces: Electrobounces::new(rom),
        }
    }
}

// ---------------------------------------------------------------------------
// Stars
// ---------------------------------------------------------------------------

/// The five stars on each side wall. Each team lights the stars on the
/// wall it attacks; a full row is worth 10 points.
#[derive(Clone, Debug)]
pub struct Stars {
    lit: PerTeam<u8>,
    /// Tile state per star: 6 off, 7 lit by team 1, 8 lit by team 2. The sign
    /// says which wall the row is on (positive: left).
    tiles: PerTeam<[i32; 5]>,
}

impl Default for Stars {
    fn default() -> Self {
        Self {
            lit: PerTeam::default(),
            tiles: PerTeam::new([6; 5], [-6; 5]),
        }
    }
}

impl Stars {
    const FULL_ROW: u8 = 0b11111;

    /// Tile variant per star for the left and right walls: `None` when off,
    /// else 0 (team 1 colour) or 1 (team 2 colour).
    pub fn wall_tiles(&self) -> [[Option<i32>; 5]; 2] {
        let (left, right) = if self.team1_on_left() {
            (&self.tiles.team1, &self.tiles.team2)
        } else {
            (&self.tiles.team2, &self.tiles.team1)
        };
        let variant = |t: &i32| Some(t.abs() - 7).filter(|&v| v >= 0);
        [left.each_ref().map(variant), right.each_ref().map(variant)]
    }

    fn team1_on_left(&self) -> bool {
        self.tiles.team1[0] >= 0
    }

    fn is_lit(&self, side: Side, star: usize) -> bool {
        self.lit.get(side) & (1 << star) != 0
    }

    /// Lights a star; returns whether it was off.
    fn light(&mut self, side: Side, star: usize) -> bool {
        if self.is_lit(side, star) {
            return false;
        }
        *self.lit.get_mut(side) |= 1 << star;
        bump_magnitude(&mut self.tiles.get_mut(side)[star], Self::colour_step(side));
        true
    }

    fn put_out(&mut self, side: Side, star: usize) {
        if !self.is_lit(side, star) {
            return;
        }
        *self.lit.get_mut(side) &= !(1 << star) & Self::FULL_ROW;
        bump_magnitude(
            &mut self.tiles.get_mut(side)[star],
            -Self::colour_step(side),
        );
    }

    fn colour_step(side: Side) -> i32 {
        match side {
            Side::Team1 => 1,
            Side::Team2 => 2,
        }
    }

    pub fn switch_sides(&mut self) {
        for star in 0..5 {
            self.put_out(Side::Team1, star);
            self.put_out(Side::Team2, star);
        }
        std::mem::swap(&mut self.tiles.team1, &mut self.tiles.team2);
    }
}

/// Moves a signed tile value away from (or towards) zero.
fn bump_magnitude(value: &mut i32, delta: i32) {
    if *value >= 0 {
        *value += delta;
    } else {
        *value -= delta;
    }
}

// ---------------------------------------------------------------------------
// Score multipliers
// ---------------------------------------------------------------------------

/// The ramps on each side wall. Sending the ball through one first cancels
/// the opponent's multiplier, then raises your own (x1.5, then x2).
pub struct ScoreMultipliers {
    level: PerTeam<i32>,
    /// The ball's way through the ramp it is in.
    path: MultiplierPath,
    step: i32,
    origin: Point,
    /// Indicator lights: team 1 pair at 0 and 2, team 2 pair at 1 and 3.
    pub lights: [Entity; 4],
}

impl ScoreMultipliers {
    const MAX_LEVEL: i32 = 2;
    const LIGHTS_UP_AT_STEP: i32 = 22;

    fn new(rom: &Rom) -> Self {
        let sprites = [41, 39, 37, 35];
        let lights = std::array::from_fn(|i| {
            let mut light = rom.start().entity(EntityId::ScoreMultiplier(i as u8));
            light.sprite = sprites[i];
            light.flags.set_invisible(true);
            light
        });
        Self {
            level: PerTeam::default(),
            path: MultiplierPath::LeftUp,
            step: 0,
            origin: Point::ZERO,
            lights,
        }
    }

    pub fn level(&self, side: Side) -> i32 {
        *self.level.get(side)
    }

    /// Updates the levels after a ramp run by `thrower_side`. Returns
    /// `false` if nothing changed (the thrower's level was already maximal).
    fn register_run(&mut self, thrower_side: Side) -> bool {
        let opponent = thrower_side.opponent();
        let (side, change) = if *self.level.get(opponent) != 0 {
            *self.level.get_mut(opponent) -= 1;
            (
                opponent,
                LightChange::Lower {
                    now_off: self.level(opponent) == 0,
                },
            )
        } else {
            if self.level(thrower_side) == Self::MAX_LEVEL {
                return false;
            }
            *self.level.get_mut(thrower_side) += 1;
            (
                thrower_side,
                LightChange::Raise {
                    first: self.level(thrower_side) == 1,
                },
            )
        };
        let first_light = match side {
            Side::Team1 => 0,
            Side::Team2 => 1,
        };
        for light in [first_light, first_light + 2] {
            let light = &mut self.lights[light];
            match change {
                LightChange::Raise { first: true } => light.flags.set_invisible(false),
                LightChange::Raise { first: false } => light.sprite += 1,
                LightChange::Lower { now_off: true } => light.flags.set_invisible(true),
                LightChange::Lower { now_off: false } => light.sprite -= 1,
            }
        }
        true
    }
}

#[derive(Clone, Copy)]
enum LightChange {
    Raise { first: bool },
    Lower { now_off: bool },
}

// ---------------------------------------------------------------------------
// Electrobounces
// ---------------------------------------------------------------------------

/// The two pads that charge the ball so it knocks opponents over.
pub struct Electrobounces {
    light_timer: i32,
    pub pads: [Entity; 2],
}

impl Electrobounces {
    fn new(rom: &Rom) -> Self {
        let pads = std::array::from_fn(|i| {
            let mut pad = rom.start().entity(EntityId::Electrobounce(i as u8));
            pad.flags.set_invisible(true);
            pad
        });
        Self {
            light_timer: 0,
            pads,
        }
    }
}

// ---------------------------------------------------------------------------
// Match interaction
// ---------------------------------------------------------------------------

impl Match {
    fn ball_is_free_or_being_thrown(&self) -> bool {
        self.holder
            .map_or(true, |holder| self.entity(holder).think == Think::Throw)
    }

    pub(crate) fn arena_check_star_hit(&mut self) {
        if !self.ball_is_free_or_being_thrown() {
            return;
        }
        let ball = self.props.ball.pos;
        let team1_on_left = self.arena.stars.team1_on_left();
        let (star, wall_of_team1) = if ball.x <= 32 {
            if !(384..544).contains(&ball.y) {
                return;
            }
            ((ball.y - 384) / 32, team1_on_left)
        } else if ball.x >= 608 {
            if !(608..768).contains(&ball.y) {
                return;
            }
            ((ball.y - 608) / 32, !team1_on_left)
        } else {
            return;
        };
        let star = star as usize;
        let thrower_side = self.side_of(self.thrower);
        let wall_owner = Side::from_is_team1(wall_of_team1);
        if wall_owner == thrower_side {
            if self.arena.stars.light(thrower_side, star) {
                self.add_score(thrower_side, 2);
                self.play_sound(Sound::STAR);
            }
        } else if self.arena.stars.is_lit(wall_owner, star) {
            // Hitting the opponent's lit star puts it out and costs them.
            self.arena.stars.put_out(wall_owner, star);
            self.score.deduct(wall_owner, 2);
            self.play_sound(Sound::STAR);
        }
    }

    pub(crate) fn arena_check_full_star_rows(&mut self) {
        for side in [Side::Team1, Side::Team2] {
            if *self.arena.stars.lit.get(side) == Stars::FULL_ROW {
                for star in 0..5 {
                    self.arena.stars.put_out(side, star);
                }
                self.add_score(side, 10);
                self.play_sound(Sound::STAR_ROW);
                self.cheer_if_thrower_attacked_up();
            }
        }
    }

    pub(crate) fn arena_check_multiplier_entry(&mut self) {
        let ball = &self.props.ball;
        if self.holder.is_some() || self.ball_in_multiplier || ball.is_high() {
            return;
        }
        if ball.pos.x < 24 || ball.pos.x > 616 {
            return;
        }
        let heading_straight = ball.dir == 0 || ball.dir == 4;
        let (left, top, bottom, entrance_ok, path, origin_x) = if ball.pos.x < 64 {
            let path = if ball.dir == 4 {
                MultiplierPath::LeftDown
            } else {
                MultiplierPath::LeftUp
            };
            (true, 576, 640, ball.pos.x >= 48, path, 0)
        } else if ball.pos.x > 576 {
            let path = if ball.dir == 4 {
                MultiplierPath::RightDown
            } else {
                MultiplierPath::RightUp
            };
            (false, 512, 576, ball.pos.x <= 592, path, 640)
        } else {
            return;
        };
        if ball.pos.y > bottom || ball.pos.y < top {
            return;
        }
        if entrance_ok && heading_straight {
            let multipliers = &mut self.arena.multipliers;
            multipliers.path = path;
            multipliers.step = 0;
            multipliers.origin = Point::new(origin_x, top);
            self.ball_in_multiplier = true;
            self.props.ball.script = self.rom.script(Script::BallInMultiplier);
            self.props.ball.script_index = 0;
        } else {
            self.bounce_off_multiplier(left, top, bottom);
        }
    }

    fn bounce_off_multiplier(&mut self, left: bool, top: i32, bottom: i32) {
        let rom = &self.rom;
        let ball = &mut self.props.ball;
        let previous = Point::new(
            ball.pos.x - 2 * ball.velocity.x,
            ball.pos.y - 2 * ball.velocity.y,
        );
        if (left && previous.x >= 64) || (!left && previous.x <= 576) {
            ball.velocity.x = -ball.velocity.x;
            ball.dir = rom.mirror_dir_x(ball.dir);
        }
        if previous.y < top || previous.y > bottom {
            ball.velocity.y = -ball.velocity.y;
            ball.dir = rom.mirror_dir_y(ball.dir);
        }
        self.play_sound(Sound::MULTIPLIER_BOUNCE);
    }

    pub(crate) fn arena_follow_multiplier_path(&mut self) {
        if !self.ball_in_multiplier {
            return;
        }
        let multipliers = &self.arena.multipliers;
        let offset = self
            .rom
            .multiplier_path_step(multipliers.path, multipliers.step);
        if offset.is_zero() {
            // End of the ramp: the ball shoots back out.
            self.set_ball_speed_from_throw(self.thrower_skill_thr());
            self.ball_in_multiplier = false;
            let ball = &mut self.props.ball;
            ball.script = self.rom.script(Script::BallHeld);
            ball.script_index = 0;
            ball.velocity.y = if ball.velocity.y >= 0 { 8 } else { -8 };
            return;
        }
        let multipliers = &mut self.arena.multipliers;
        multipliers.step += 1;
        self.props.ball.pos = multipliers.origin + offset;
        if multipliers.step == ScoreMultipliers::LIGHTS_UP_AT_STEP {
            let side = self.side_of(self.thrower);
            if self.arena.multipliers.register_run(side) {
                self.play_sound(Sound::MULTIPLIER_CHANGE);
                self.cheer_if_thrower_attacked_up();
            }
        }
    }

    pub(crate) fn arena_check_dome_hits(&mut self) {
        if self.holder.is_some() || self.props.ball.is_high() {
            return;
        }
        for dome in 0..2 {
            self.check_dome_hit(dome);
        }
    }

    fn check_dome_hit(&mut self, dome: usize) {
        let ball = self.props.ball.pos;
        let entity = &mut self.arena.domes[dome];
        let delta = ball - entity.pos;
        if delta.y.abs() > 16 || delta.x.abs() > 16 || entity.distance_to(ball) > 16 {
            return;
        }
        // The dome shows again, and plays its hit from the start.
        entity.flags.set_invisible(false);
        entity.script_index = 0;
        let bits = entity.dir_bits_towards(ball);
        if !bits.has_direction() {
            return;
        }
        self.deflect_ball(self.rom.dir_from_bits(bits));
        let side = self.side_of(self.thrower);
        self.add_score(side, 2);
        self.play_sound(Sound::DOME);
    }

    pub(crate) fn arena_check_electrobounce_hits(&mut self) {
        let electrobounces = &mut self.arena.electrobounces;
        if electrobounces.light_timer != 0 {
            electrobounces.light_timer -= 1;
            if electrobounces.light_timer == 0 {
                for pad in &mut electrobounces.pads {
                    pad.flags.set_invisible(true);
                }
            }
        }
        if !self.ball_is_free_or_being_thrown() || self.props.ball.is_high() {
            return;
        }
        for pad in 0..2 {
            self.check_electrobounce_hit(pad);
        }
    }

    fn check_electrobounce_hit(&mut self, pad: usize) {
        let ball = self.props.ball.pos;
        let electrobounces = &mut self.arena.electrobounces;
        let entity = &mut electrobounces.pads[pad];
        if entity.axis_distance(ball, 15) > 15 {
            return;
        }
        entity.flags.set_invisible(false);
        electrobounces.light_timer = 2;
        let bits = electrobounces.pads[pad].dir_bits_towards(ball);
        if !bits.has_direction() {
            return;
        }
        let ball = &mut self.props.ball;
        ball.pos.x = if ball.pos.x <= 320 { 32 } else { 608 };
        ball.set_charged(true);
        self.deflect_ball(self.rom.dir_from_bits(bits));
        self.play_sound(Sound::ELECTROBOUNCE);
        self.cheer_if_thrower_attacked_up();
    }

    /// Sends the free ball off in `dir` at full speed.
    fn deflect_ball(&mut self, dir: i32) {
        let ball = &mut self.props.ball;
        ball.dir = dir;
        ball.velocity = self.rom.velocity(dir, 8);
        ball.script_index = 0;
        self.set_ball_speed_from_throw(self.thrower_skill_thr());
    }
}
